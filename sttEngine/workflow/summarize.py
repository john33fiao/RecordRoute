# summarize.py
import argparse
import json
import logging
import platform
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional, Tuple
import time
import signal
from concurrent.futures import ThreadPoolExecutor, TimeoutError as FutureTimeoutError

try:
    import ollama
except ImportError:
    print("오류: ollama 패키지가 설치되지 않았습니다. 'pip install ollama'로 설치하세요.")
    sys.exit(1)

# 설정 모듈 임포트
sys.path.append(str(Path(__file__).parent.parent))
from config import get_model_for_task, get_default_model, get_config_value

# 설정 상수 - .env 파일에서 로드
try:
    DEFAULT_MODEL = get_model_for_task("SUMMARY", get_default_model("SUMMARY"))
except:
    # 환경변수 설정이 없을 때 기존 로직 사용
    if platform.system() == "Windows":
        DEFAULT_MODEL = "gemma3:4b"
    else:
        DEFAULT_MODEL = "gpt-oss:20b"

DEFAULT_CHUNK_SIZE = get_config_value("DEFAULT_CHUNK_SIZE", 12000, int)  # 바이트 단위로 증가
DEFAULT_TEMPERATURE = get_config_value("DEFAULT_TEMPERATURE_SUMMARY", 0.2, float)
DEFAULT_NUM_CTX = get_config_value("DEFAULT_NUM_CTX", 8192, int)
MAX_RETRIES = get_config_value("MAX_RETRIES", 3, int)
RETRY_DELAY = get_config_value("RETRY_DELAY", 2, int)
OLLAMA_TIMEOUT = get_config_value("OLLAMA_TIMEOUT", 300, int)  # 5분 타임아웃

# 프롬프트 템플릿
BASE_PROMPT = """당신은 전문 요약가입니다. 다음 텍스트를 간결하고 구조화된 한국어 요약으로 작성합니다.

지침:
- 불렛 포인트를 사용합니다.
- 사실에만 근거합니다. 해석/추정/의견 금지.
- 섹션 제목은 다음 순서를 고정합니다:
  1) 주요 주제
  2) 핵심 내용
  3) 결정 사항
  4) 실행 항목
  5) 리스크/이슈
  6) 차기 일정

출력은 반드시 위 6개 섹션만 포함합니다."""

CHUNK_PROMPT = BASE_PROMPT + """

아래 청크를 요약하세요:
---
{chunk}
---"""

REDUCE_PROMPT = BASE_PROMPT + """

아래는 여러 청크 요약의 모음입니다. 중복을 제거하고 상충 내용을 조정하여 하나의 최종 요약으로 통합하세요:
---
{summaries}
---"""

class SummarizationError(Exception):
    """요약 처리 중 발생하는 예외"""
    pass

def setup_logging(verbose: bool) -> None:
    """로깅 설정"""
    level = logging.DEBUG if verbose else logging.INFO
    format_str = "%(asctime)s - %(levelname)s: %(message)s"
    logging.basicConfig(level=level, format=format_str, datefmt="%H:%M:%S")

def validate_model(model: str) -> bool:
    """모델 존재 여부 확인"""
    try:
        models = ollama.list()
        available_models = [m.get('name') for m in models.get('models', []) if m.get('name')]
        return model in available_models
    except Exception as e:
        logging.warning(f"모델 목록 조회 실패: {e}")
        return True  # 검증 실패 시 진행 허용

def remove_timestamps(text: str) -> str:
    """텍스트에서 타임스탬프 제거 (원본 파일은 수정하지 않음)"""
    # [00:00:28 - 00:00:36] 형태의 타임스탬프 패턴 제거
    timestamp_pattern = r'\[[\d:]+\s*-\s*[\d:]+\]\s*'
    cleaned_text = re.sub(timestamp_pattern, '', text)
    
    # 빈 줄 정리 (연속된 줄바꿈을 최대 2개로 제한)
    cleaned_text = re.sub(r'\n{3,}', '\n\n', cleaned_text)
    
    return cleaned_text.strip()

def read_text_with_fallback(path: Path, encoding: str = "utf-8") -> str:
    """인코딩 fallback을 지원하는 텍스트 읽기"""
    encodings = [encoding, "utf-8", "cp949", "euc-kr", "latin-1"]
    
    for enc in encodings:
        try:
            content = path.read_text(encoding=enc)
            if enc != encoding:
                logging.info(f"인코딩 변경: {encoding} → {enc}")
            return content
        except UnicodeDecodeError:
            continue
        except Exception as e:
            logging.error(f"파일 읽기 실패 ({enc}): {e}")
            continue
    
    raise SummarizationError(f"모든 인코딩으로 파일 읽기 실패: {path}")

def chunk_text(text: str, max_bytes: int) -> List[str]:
    """텍스트를 문단 기준으로 바이트 단위 청크 분할"""
    if not text.strip():
        return []
    
    paragraphs = [p.strip() for p in text.split('\n\n')]
    if not paragraphs:
        paragraphs = [line.strip() for line in text.splitlines() if line.strip()]
    
    chunks = []
    current_chunk = []
    current_size = 0
    
    for para in paragraphs:
        para_bytes = len(para.encode('utf-8'))
        
        # 단일 문단이 청크 크기를 초과하는 경우 강제 분할
        if para_bytes > max_bytes:
            if current_chunk:
                chunks.append('\n\n'.join(current_chunk))
                current_chunk = []
                current_size = 0
            
            # 문장 단위로 분할 시도
            sentences = para.split('. ')
            temp_chunk = []
            temp_size = 0
            
            for sentence in sentences:
                sentence = sentence.strip()
                if not sentence:
                    continue
                    
                if not sentence.endswith('.'):
                    sentence += '.'
                
                sentence_bytes = len(sentence.encode('utf-8'))
                if temp_size + sentence_bytes + 2 > max_bytes and temp_chunk:
                    chunks.append(' '.join(temp_chunk))
                    temp_chunk = []
                    temp_size = 0
                
                temp_chunk.append(sentence)
                temp_size += sentence_bytes + 2
            
            if temp_chunk:
                chunks.append(' '.join(temp_chunk))
            continue
        
        # 일반적인 청크 처리
        if current_size + para_bytes + 2 > max_bytes and current_chunk:
            chunks.append('\n\n'.join(current_chunk))
            current_chunk = []
            current_size = 0
        
        current_chunk.append(para)
        current_size += para_bytes + 2
    
    if current_chunk:
        chunks.append('\n\n'.join(current_chunk))
    
    return [chunk for chunk in chunks if chunk.strip()]

def call_ollama_with_timeout(
    model: str,
    prompt: str,
    options: dict,
    timeout: int = OLLAMA_TIMEOUT
) -> str:
    """타임아웃을 적용한 Ollama 호출"""
    def _call_ollama():
        return ollama.chat(
            model=model,
            messages=[{"role": "user", "content": prompt}],
            options=options,
            stream=False,
        )
    
    with ThreadPoolExecutor(max_workers=1) as executor:
        future = executor.submit(_call_ollama)
        try:
            response = future.result(timeout=timeout)
            return response
        except FutureTimeoutError:
            logging.error(f"Ollama 호출 타임아웃 ({timeout}초)")
            future.cancel()
            raise SummarizationError(f"Ollama 호출이 {timeout}초 내에 완료되지 않음")

def call_ollama_with_retry(
    model: str, 
    prompt: str, 
    temperature: float = DEFAULT_TEMPERATURE,
    num_ctx: int = DEFAULT_NUM_CTX,
    max_tokens: Optional[int] = None
) -> str:
    """재시도 로직과 타임아웃을 포함한 Ollama 호출"""
    options = {
        "temperature": temperature,
        "num_ctx": num_ctx,
    }
    
    if max_tokens:
        options["num_predict"] = max_tokens
    
    for attempt in range(MAX_RETRIES):
        try:
            logging.debug(f"모델 호출 시도 {attempt + 1}/{MAX_RETRIES}")
            
            response = call_ollama_with_timeout(model, prompt, options, OLLAMA_TIMEOUT)

            # 응답 형식 처리
            try:
                result = response["message"]["content"]
            except (TypeError, KeyError):
                raise SummarizationError(
                    f"지원하지 않는 응답 타입({type(response)})이거나 'message.content' 키가 없습니다."
                )
            
            if not result or not result.strip():
                raise SummarizationError("빈 응답 수신")
            
            logging.debug(f"모델 응답 수신 (길이: {len(result)} chars)")
            return result.strip()
            
        except Exception as e:
            logging.warning(f"모델 호출 실패 (시도 {attempt + 1}): {e}")
            if attempt < MAX_RETRIES - 1:
                logging.info(f"{RETRY_DELAY}초 후 재시도...")
                time.sleep(RETRY_DELAY)
            else:
                raise SummarizationError(f"모든 재시도 실패: {e}")

def summarize_text_mapreduce(
    text: str,
    model: str,
    chunk_size: int,
    max_tokens: Optional[int],
    temperature: float = DEFAULT_TEMPERATURE
) -> str:
    """맵-리듀스 패턴으로 텍스트 요약"""
    if not text.strip():
        return "요약할 내용이 없습니다."
    
    # 타임스탬프 제거 (원본 파일은 수정하지 않음)
    cleaned_text = remove_timestamps(text)
    original_bytes = len(text.encode('utf-8'))
    cleaned_bytes = len(cleaned_text.encode('utf-8'))
    logging.info(f"타임스탬프 제거 완료: {original_bytes:,} bytes → {cleaned_bytes:,} bytes ({original_bytes - cleaned_bytes:,} bytes 감소)")
    
    chunks = chunk_text(cleaned_text, chunk_size)
    logging.info(f"텍스트 분할 완료: {len(chunks)}개 청크 (청크당 최대 {chunk_size:,} bytes, 전체 {cleaned_bytes:,} bytes)")
    
    if len(chunks) == 0:
        return "분할된 청크가 없습니다."
    
    # 단일 청크인 경우 직접 요약
    if len(chunks) == 1:
        logging.info("단일 청크 요약 수행")
        prompt = CHUNK_PROMPT.format(chunk=chunks[0])
        return call_ollama_with_retry(model, prompt, temperature, max_tokens=max_tokens)
    
    # 1단계: 각 청크 요약 (Map)
    logging.info("1단계: 청크별 요약 시작")
    chunk_summaries = []
    
    for i, chunk in enumerate(chunks, 1):
        chunk_bytes = len(chunk.encode('utf-8'))
        logging.info(f"청크 {i}/{len(chunks)} 요약 중...")
        logging.debug(f"청크 크기: {chunk_bytes:,} bytes")
        
        try:
            prompt = CHUNK_PROMPT.format(chunk=chunk)
            summary = call_ollama_with_retry(model, prompt, temperature, max_tokens=max_tokens)
            chunk_summaries.append(summary)
            
            summary_bytes = len(summary.encode('utf-8'))
            logging.debug(f"청크 {i} 요약 완료 (크기: {summary_bytes:,} bytes)")
        except SummarizationError as e:
            logging.error(f"청크 {i} 요약 실패: {e}")
            raise  # 청크 요약 실패 시 전체 작업 중단
    
    # 2단계: 청크 요약들을 통합 요약 (Reduce)
    logging.info("2단계: 통합 요약 시작")
    combined_summaries = '\n\n---청크 요약 구분선---\n\n'.join(chunk_summaries)
    
    reduce_prompt = REDUCE_PROMPT.format(summaries=combined_summaries)
    final_summary = call_ollama_with_retry(model, reduce_prompt, temperature, max_tokens=max_tokens)
    
    logging.info("맵-리듀스 요약 완료")
    return final_summary

def parse_summary_to_sections(summary: str) -> Dict[str, List[str]]:
    """요약 텍스트를 섹션별로 파싱"""
    sections = {
        "주요 주제": [],
        "핵심 내용": [],
        "결정 사항": [],
        "실행 항목": [],
        "리스크/이슈": [],
        "차기 일정": []
    }
    
    current_section = None
    lines = summary.split('\n')
    
    for line in lines:
        line = line.strip()
        if not line:
            continue
        
        # 섹션 헤더 감지
        if '1)' in line and '주요 주제' in line:
            current_section = "주요 주제"
        elif '2)' in line and '핵심 내용' in line:
            current_section = "핵심 내용"
        elif '3)' in line and '결정' in line:
            current_section = "결정 사항"
        elif '4)' in line and '실행' in line:
            current_section = "실행 항목"
        elif '5)' in line and ('리스크' in line or '이슈' in line):
            current_section = "리스크/이슈"
        elif '6)' in line and '차기 일정' in line:
            current_section = "차기 일정"
        elif current_section and (line.startswith('- ') or line.startswith('• ') or line.startswith('* ')):
            # 불릿 포인트 내용 추가
            content = line.lstrip('- •* ').strip()
            if content:
                sections[current_section].append(content)
    
    return sections

def save_output(content: str, output_path: Path, as_json: bool = False) -> None:
    """출력 파일 저장"""
    try:
        output_path.parent.mkdir(parents=True, exist_ok=True)
        
        if as_json:
            sections = parse_summary_to_sections(content)
            json_content = json.dumps(sections, ensure_ascii=False, indent=2)
            output_path = output_path.with_suffix('.summary.json')
            output_path.write_text(json_content, encoding='utf-8')
        else:
            output_path.write_text(content, encoding='utf-8')
        
        logging.info(f"요약 결과 저장: {output_path}")
        
    except Exception as e:
        raise SummarizationError(f"파일 저장 실패: {e}")

def read_from_stdin() -> str:
    """표준 입력에서 텍스트 읽기"""
    try:
        return sys.stdin.read()
    except Exception as e:
        raise SummarizationError(f"표준 입력 읽기 실패: {e}")

def main() -> None:
    """메인 함수"""
    parser = argparse.ArgumentParser(
        description="Ollama를 사용한 텍스트 요약 도구 (맵-리듀스 지원)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
사용 예시:
  %(prog)s meeting.txt                    # 기본 요약
  %(prog)s meeting.txt --verbose          # 상세 로그와 함께
  %(prog)s meeting.txt --json             # JSON 형식 출력
  %(prog)s meeting.txt --model llama3.1   # 다른 모델 사용
  cat meeting.txt | %(prog)s --stdin      # 표준 입력 사용
        """
    )
    
    parser.add_argument(
        "input_file", 
        nargs='?',
        help="입력 텍스트 파일 경로 (--stdin 사용 시 생략 가능)"
    )
    parser.add_argument(
        "--model", 
        default=DEFAULT_MODEL,
        help=f"사용할 Ollama 모델 (기본값: {DEFAULT_MODEL})"
    )
    parser.add_argument(
        "--output", "--out",
        help="출력 파일 경로 (기본값: <입력파일명>.summary.md)"
    )
    parser.add_argument(
        "--chunk-size",
        type=int,
        default=DEFAULT_CHUNK_SIZE,
        help=f"청크 최대 바이트 수 (기본값: {DEFAULT_CHUNK_SIZE})"
    )
    parser.add_argument(
        "--max-tokens",
        type=int,
        help="모델 출력 토큰 상한"
    )
    parser.add_argument(
        "--temperature",
        type=float,
        default=DEFAULT_TEMPERATURE,
        help=f"모델 온도 설정 (기본값: {DEFAULT_TEMPERATURE})"
    )
    parser.add_argument(
        "--json",
        action="store_true",
        help="JSON 형식으로 섹션별 구조화된 출력"
    )
    parser.add_argument(
        "--verbose", "-v",
        action="store_true",
        help="상세한 진행 로그 출력"
    )
    parser.add_argument(
        "--stdin",
        action="store_true",
        help="표준 입력에서 텍스트 읽기"
    )
    parser.add_argument(
        "--encoding",
        default="utf-8",
        help="입력 파일 인코딩 (기본값: utf-8)"
    )
    
    args = parser.parse_args()
    
    # 로깅 설정
    setup_logging(args.verbose)
    
    try:
        # 입력 검증
        if args.stdin:
            if args.input_file:
                logging.warning("--stdin 사용 시 입력 파일 무시됨")
            text = read_from_stdin()
            output_path = Path(args.output) if args.output else Path("summary.md")
        else:
            if not args.input_file:
                parser.error("입력 파일을 지정하거나 --stdin을 사용하세요")
            
            input_path = Path(args.input_file)
            if not input_path.exists():
                logging.error(f"입력 파일이 존재하지 않습니다: {input_path}")
                sys.exit(2)
            
            text = read_text_with_fallback(input_path, args.encoding)
            output_path = Path(args.output) if args.output else input_path.with_name(f"{input_path.stem}.summary.md")
        
        # 모델 검증
        logging.info(f"모델 검증: {args.model}")
        if not validate_model(args.model):
            logging.warning(f"모델 '{args.model}'을 확인할 수 없습니다. 계속 진행합니다.")
        
        # 입력 텍스트 검증
        if not text.strip():
            logging.error("입력 텍스트가 비어 있습니다")
            sys.exit(3)
        
        text_bytes = len(text.encode('utf-8'))
        logging.info(f"입력 텍스트 크기: {len(text):,} 문자 ({text_bytes:,} bytes)")
        logging.info(f"청크 크기: {args.chunk_size:,} bytes")
        logging.info(f"모델: {args.model}")
        
        # 요약 실행
        summary = summarize_text_mapreduce(
            text=text,
            model=args.model,
            chunk_size=args.chunk_size,
            max_tokens=args.max_tokens,
            temperature=args.temperature
        )
        
        # 결과 저장
        save_output(summary, output_path, args.json)
        
        # 완료 메시지
        logging.info("요약 작업이 성공적으로 완료되었습니다")
        if args.verbose:
            logging.info(f"요약 길이: {len(summary):,} 문자")
        
    except KeyboardInterrupt:
        logging.info("사용자에 의해 중단되었습니다")
        sys.exit(130)
    except SummarizationError as e:
        logging.error(f"요약 오류: {e}")
        sys.exit(1)
    except Exception as e:
        logging.error(f"예상치 못한 오류: {e}")
        if args.verbose:
            import traceback
            logging.debug(traceback.format_exc())
        sys.exit(1)

if __name__ == "__main__":
    main()