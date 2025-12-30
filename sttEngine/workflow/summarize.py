# summarize.py
import argparse
import json
import logging
import platform
import re
import sys
from pathlib import Path
from typing import Dict, List, Optional
import time
from concurrent.futures import ThreadPoolExecutor, TimeoutError as FutureTimeoutError

# 설정 모듈 임포트
sys.path.append(str(Path(__file__).parent.parent))
from config import get_model_for_task, get_default_model, get_config_value
from logger import setup_logging

setup_logging()
from llamacpp_utils import check_model_available, generate_text

# 설정 상수 - .env 파일에서 로드
try:
    DEFAULT_MODEL = get_model_for_task("SUMMARY", get_default_model("SUMMARY"))
except:
    # 환경변수 설정이 없을 때 기본 GGUF 모델 사용
    if platform.system() == "Windows":
        DEFAULT_MODEL = "gemma-2-2b-it-Q4_K_M.gguf"
    else:
        DEFAULT_MODEL = "qwen2.5-14b-instruct-q4_k_m.gguf"

DEFAULT_CHUNK_SIZE = get_config_value("DEFAULT_CHUNK_SIZE", 32000, int)  # 청킹 크기 증가로 불필요한 분할 방지
DEFAULT_TEMPERATURE = get_config_value("DEFAULT_TEMPERATURE_SUMMARY", 0.2, float)
DEFAULT_NUM_CTX = get_config_value("DEFAULT_NUM_CTX", 8192, int)
MAX_RETRIES = get_config_value("MAX_RETRIES", 3, int)
RETRY_DELAY = get_config_value("RETRY_DELAY", 2, int)
LLM_TIMEOUT = get_config_value("LLM_TIMEOUT", 300, int)  # 5분 타임아웃

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
    """GGUF 모델 파일 존재 여부 확인"""
    try:
        # 모델 파일 사용 가능성 확인
        model_ok, model_msg = check_model_available(model)
        if not model_ok:
            logging.warning(f"모델 확인 오류: {model_msg}")
        else:
            logging.info(model_msg)
        return model_ok
    except Exception as e:
        logging.warning(f"모델 확인 실패: {e}")
        return True  # 검증 실패 시 진행 허용

def is_repetitive_text(text: str, max_repetition: int = 5) -> bool:
    """반복되는 무의미한 텍스트인지 확인"""
    words = text.split()
    if len(words) < 2:
        return False
    
    # 동일한 단어가 연속으로 반복되는지 확인
    for i in range(len(words) - 1):
        if words[i] == words[i + 1]:
            # 연속된 동일 단어의 개수 세기
            count = 1
            j = i + 1
            while j < len(words) and words[j] == words[i]:
                count += 1
                j += 1
            if count >= max_repetition:
                return True
    
    # 전체가 같은 단어의 반복인지 확인 (예: "아 아 아 아 아")
    unique_words = set(words)
    if len(unique_words) == 1 and len(words) >= max_repetition:
        return True
    
    return False

def remove_timestamps(text: str) -> str:
    """텍스트에서 타임스탬프 제거, 중복 제거, 반복 텍스트 필터링, 문단 경계 유지 및 소프트 병합"""
    # 모든 타임스탬프 패턴 제거
    patterns = [
        r'\[[\d:]+\s*-\s*[\d:]+\]',  # [00:00:28 - 00:00:36]
        r'\d{2}:\d{2}:\d{2}\s*-\s*\d{2}:\d{2}:\d{2}',  # 00:00:28 - 00:00:36
        r'\[\d{2}:\d{2}:\d{2}\]',  # [00:00:28]
    ]
    
    cleaned_text = text
    for pattern in patterns:
        cleaned_text = re.sub(pattern, '', cleaned_text)
    
    # 줄별 처리로 중복, 무의미한 줄, 반복 텍스트 제거
    lines = cleaned_text.split('\n')
    meaningful_lines = []
    seen_lines = set()  # 중복 제거용
    repetitive_filtered = 0  # 반복 텍스트 필터링 개수
    
    for i, line in enumerate(lines):
        cleaned_line = line.strip()
        
        # 기본 필터링 조건
        if (cleaned_line and 
            len(cleaned_line) > 3 and  # 최소 4글자 이상
            not re.match(r'^[\s\[\]\d:-]+$', cleaned_line) and  # 타임스탬프 잔여물 제거
            cleaned_line not in seen_lines):  # 중복 제거
            
            # 반복 텍스트 필터링 추가
            if is_repetitive_text(cleaned_line):
                repetitive_filtered += 1
                logging.debug(f"반복 텍스트 필터링: '{cleaned_line[:50]}...'")
                continue
            
            meaningful_lines.append(cleaned_line)
            seen_lines.add(cleaned_line)
        elif not cleaned_line:  # 빈 줄은 문단 경계로 보존
            # 연속된 빈 줄은 하나만 유지
            if meaningful_lines and meaningful_lines[-1] != "":
                meaningful_lines.append("")
    
    # 소프트 병합: 너무 짧은 라인을 다음 줄과 결합
    merged_lines = []
    i = 0
    while i < len(meaningful_lines):
        current_line = meaningful_lines[i]
        
        # 빈 줄은 그대로 유지 (문단 경계)
        if current_line == "":
            merged_lines.append(current_line)
            i += 1
            continue
        
        # 현재 줄이 너무 짧고 (20자 미만) 다음 줄이 존재하며 빈 줄이 아닌 경우
        if (len(current_line) < 20 and 
            i + 1 < len(meaningful_lines) and 
            meaningful_lines[i + 1] != ""):
            
            # 다음 줄과 결합
            next_line = meaningful_lines[i + 1]
            merged_line = f"{current_line} {next_line}"
            merged_lines.append(merged_line)
            i += 2  # 두 줄을 처리했으므로 +2
        else:
            merged_lines.append(current_line)
            i += 1
    
    # 줄 수 제한을 더 유연하게 조정
    original_count = len(merged_lines)
    if len(merged_lines) > 500:  # 제한을 500줄로 증가
        logging.warning(f"의미있는 줄이 {len(merged_lines)}개로 많음. 처음 500줄만 사용")
        merged_lines = merged_lines[:500]
    
    result = '\n'.join(merged_lines)
    
    # 3개 이상의 연속 빈 줄을 2개로 제한
    result = re.sub(r'\n{3,}', '\n\n', result)
    
    if repetitive_filtered > 0:
        logging.info(f"반복 텍스트 {repetitive_filtered}개 줄 필터링됨")
    logging.info(f"타임스탬프 제거 및 소프트 병합: {len(text.splitlines())}줄 → {len(merged_lines)}줄")

    return result.strip()


def strip_prefix_before_bracket(text: str) -> str:
    """각 줄에서 "] " 앞의 부분을 제거하여 요약 프롬프트에 필요한 본문만 남깁니다."""
    processed_lines = []
    for line in text.splitlines():
        # 원본 파일은 수정하지 않고 프롬프트에 전달할 텍스트만 가공
        if "] " in line:
            processed_lines.append(line.split("] ", 1)[1])
        else:
            processed_lines.append(line)
    return "\n".join(processed_lines)

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

def chunk_text(text: str, max_bytes: int, target_chunks: Optional[int] = None) -> List[str]:
    """텍스트를 바이트 단위로 청크 분할 (안전한 방식)"""
    if not text.strip():
        return []
    
    # 최소 청크 크기 강제 적용 (비정상적으로 작은 청크 방지)
    if max_bytes < 5000:
        max_bytes = 15000
        logging.warning(f"청크 크기가 너무 작음. 15000 바이트로 강제 설정")
    
    # 텍스트 전체 크기 확인
    total_bytes = len(text.encode('utf-8'))
    
    # 목표 청크 수가 지정된 경우 청크 크기 자동 조정
    if target_chunks and target_chunks > 1:
        adjusted_max_bytes = max(5000, total_bytes // target_chunks)
        if adjusted_max_bytes != max_bytes:
            logging.info(f"목표 청크 수 {target_chunks}개에 맞춰 청크 크기 조정: {max_bytes:,} → {adjusted_max_bytes:,} bytes")
            max_bytes = adjusted_max_bytes
    
    logging.info(f"전체 텍스트 크기: {total_bytes:,} bytes, 청크 제한: {max_bytes:,} bytes")
    
    # 전체 텍스트가 최대 크기보다 작으면 하나의 청크로 반환
    if total_bytes <= max_bytes:
        logging.info("단일 청크로 처리")
        return [text.strip()]
    
    # 청킹 비활성화 - 전체 텍스트를 하나의 청크로 처리
    logging.info("청킹 로직 비활성화 - 전체 텍스트를 단일 청크로 처리")
    chunks = [text.strip()]
    
    final_chunks = [chunk for chunk in chunks if chunk.strip()]
    logging.info(f"최종 청크 수: {len(final_chunks)}개")
    
    return final_chunks

def call_llm_with_timeout(
    model: str,
    prompt: str,
    temperature: float,
    max_tokens: Optional[int],
    num_ctx: int,
    timeout: int = LLM_TIMEOUT
) -> str:
    """타임아웃을 적용한 llama.cpp 호출"""
    def _call_llm():
        return generate_text(
            model_filename=model,
            prompt=prompt,
            temperature=temperature,
            max_tokens=max_tokens or 512,
            n_ctx=num_ctx,
            stream=False
        )

    with ThreadPoolExecutor(max_workers=1) as executor:
        future = executor.submit(_call_llm)
        try:
            result = future.result(timeout=timeout)
            return result
        except FutureTimeoutError:
            logging.error(f"LLM 호출 타임아웃 ({timeout}초)")
            future.cancel()
            raise SummarizationError(f"LLM 호출이 {timeout}초 내에 완료되지 않음")

def call_llm_with_retry(
    model: str,
    prompt: str,
    temperature: float = DEFAULT_TEMPERATURE,
    num_ctx: int = DEFAULT_NUM_CTX,
    max_tokens: Optional[int] = None
) -> str:
    """재시도 로직과 타임아웃을 포함한 llama.cpp 호출"""
    for attempt in range(MAX_RETRIES):
        try:
            logging.debug(f"모델 호출 시도 {attempt + 1}/{MAX_RETRIES}")

            result = call_llm_with_timeout(
                model=model,
                prompt=prompt,
                temperature=temperature,
                max_tokens=max_tokens,
                num_ctx=num_ctx,
                timeout=LLM_TIMEOUT
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
    temperature: float = DEFAULT_TEMPERATURE,
    progress_callback=None,
    target_chunks: Optional[int] = None
) -> str:
    """맵-리듀스 패턴으로 텍스트 요약"""
    if not text.strip():
        return "요약할 내용이 없습니다."
    
    # 디버깅: 입력 텍스트 크기 확인
    original_bytes = len(text.encode('utf-8'))
    original_chars = len(text)
    original_lines = len(text.splitlines())
    print(f"[DEBUG] 입력 텍스트 크기: {original_chars:,} 문자, {original_bytes:,} bytes, {original_lines:,} 줄")
    print(f"[DEBUG] 텍스트 첫 200자: {repr(text[:200])}")
    
    # 요약 프롬프트 토큰 절감을 위해 각 줄의 접두사 제거 (원본 파일은 수정하지 않음)
    cleaned_text = strip_prefix_before_bracket(text)
    cleaned_bytes = len(cleaned_text.encode('utf-8'))
    logging.info(
        f"접두사 제거 완료: {original_bytes:,} bytes → {cleaned_bytes:,} bytes ({original_bytes - cleaned_bytes:,} bytes 감소)"
    )
    
    chunks = chunk_text(cleaned_text, chunk_size, target_chunks)
    logging.info(f"텍스트 분할 완료: {len(chunks)}개 청크 (청크당 최대 {chunk_size:,} bytes, 전체 {cleaned_bytes:,} bytes)")
    
    if len(chunks) == 0:
        return "분할된 청크가 없습니다."
    
    # 단일 청크인 경우 직접 요약
    if len(chunks) == 1:
        logging.info("단일 청크 요약 수행")
        prompt = CHUNK_PROMPT.format(chunk=chunks[0])
        return call_llm_with_retry(model, prompt, temperature, max_tokens=max_tokens)
    
    # 다중 청크 처리 시작 알림
    logging.info("텍스트가 길어 분할 처리중...")
    logging.info(f"총 {len(chunks)}개 청크로 분할되어 단계별 요약을 진행합니다")
    if progress_callback:
        progress_callback("텍스트가 길어 분할 처리중...")
    
    # 1단계: 각 청크 요약 (Map)
    logging.info("1단계: 청크별 요약 시작")
    chunk_summaries = []
    
    for i, chunk in enumerate(chunks, 1):
        chunk_bytes = len(chunk.encode('utf-8'))
        progress_msg = f"청크 처리중({i}/{len(chunks)})..."
        logging.info(progress_msg)
        if progress_callback:
            progress_callback(progress_msg)
        logging.debug(f"청크 크기: {chunk_bytes:,} bytes")
        
        try:
            prompt = CHUNK_PROMPT.format(chunk=chunk)
            prompt_bytes = len(prompt.encode('utf-8'))
            prompt_chars = len(prompt)
            print(f"[DEBUG] 청크 {i} 프롬프트 크기: {prompt_chars:,} 문자, {prompt_bytes:,} bytes")
            print(f"[DEBUG] 청크 {i} 내용 첫 200자: {repr(chunk[:200])}")
            summary = call_llm_with_retry(model, prompt, temperature, max_tokens=max_tokens)
            chunk_summaries.append(summary)
            
            summary_bytes = len(summary.encode('utf-8'))
            logging.debug(f"청크 {i} 요약 완료 (크기: {summary_bytes:,} bytes)")
        except SummarizationError as e:
            logging.error(f"청크 {i} 요약 실패: {e}")
            raise  # 청크 요약 실패 시 전체 작업 중단
    
    # 2단계: 청크 요약들을 통합 요약 (Reduce)
    logging.info("2단계: 통합 요약 시작")
    combined_summaries = '\n\n---청크 요약 구분선---\n\n'.join(chunk_summaries)
    
    # 배치 리듀스: 청크 요약이 많을 때 계층적 처리
    num_chunks = len(chunk_summaries)
    batch_size = 10  # K=10개씩 1차 리듀스
    
    if num_chunks > batch_size:
        logging.info(f"청크 요약 {num_chunks}개가 많아 배치 리듀스 적용 (배치 크기: {batch_size})")
        if progress_callback:
            progress_callback(f"청크 요약 {num_chunks}개를 배치로 처리중...")
        
        # 1차 리듀스: K개씩 묶어서 중간 요약 생성
        batch_summaries = []
        num_batches = (num_chunks + batch_size - 1) // batch_size  # 올림 계산
        
        for batch_idx in range(num_batches):
            start_idx = batch_idx * batch_size
            end_idx = min((batch_idx + 1) * batch_size, num_chunks)
            batch_chunk_summaries = chunk_summaries[start_idx:end_idx]
            
            progress_msg = f"1차 리듀스 배치 {batch_idx + 1}/{num_batches} 처리중..."
            logging.info(progress_msg)
            if progress_callback:
                progress_callback(progress_msg)
            
            batch_combined = '\n\n---청크 요약 구분선---\n\n'.join(batch_chunk_summaries)
            batch_prompt = REDUCE_PROMPT.format(summaries=batch_combined)
            batch_summary = call_llm_with_retry(model, batch_prompt, temperature, max_tokens=max_tokens)
            batch_summaries.append(batch_summary)
        
        # 2차 파이널 리듀스: 1차 리듀스 결과들을 최종 통합
        logging.info(f"2차 파이널 리듀스: {len(batch_summaries)}개 배치 요약 통합")
        if progress_callback:
            progress_callback("최종 통합 요약 생성중...")
        
        final_combined = '\n\n---배치 요약 구분선---\n\n'.join(batch_summaries)
        reduce_prompt = REDUCE_PROMPT.format(summaries=final_combined)
    else:
        # 청크 수가 적으면 기존 방식 사용
        combined_summaries = '\n\n---청크 요약 구분선---\n\n'.join(chunk_summaries)
        combined_bytes = len(combined_summaries.encode('utf-8'))
        logging.debug(f"통합 요약 크기: {combined_bytes:,} bytes")
        
        if combined_bytes > chunk_size * 2:  # 안전 마진 적용
            logging.info(f"통합 요약이 클 수 있음 ({combined_bytes:,} bytes). 재귀적 요약 적용")
            summary_chunks = chunk_text(combined_summaries, chunk_size)
            if len(summary_chunks) > 1:
                logging.info(f"청크 요약을 {len(summary_chunks)}개 그룹으로 재분할")
                final_summaries = []
                for i, summary_chunk in enumerate(summary_chunks, 1):
                    progress_msg = f"요약 그룹 처리중({i}/{len(summary_chunks)})..."
                    logging.info(progress_msg)
                    if progress_callback:
                        progress_callback(progress_msg)
                    group_prompt = REDUCE_PROMPT.format(summaries=summary_chunk)
                    group_summary = call_llm_with_retry(model, group_prompt, temperature, max_tokens=max_tokens)
                    final_summaries.append(group_summary)
                
                final_combined = '\n\n---최종 통합 구분선---\n\n'.join(final_summaries)
                reduce_prompt = REDUCE_PROMPT.format(summaries=final_combined)
            else:
                reduce_prompt = REDUCE_PROMPT.format(summaries=combined_summaries)
        else:
            reduce_prompt = REDUCE_PROMPT.format(summaries=combined_summaries)
    
    final_summary = call_llm_with_retry(model, reduce_prompt, temperature, max_tokens=max_tokens)
    
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
        description="llama.cpp를 사용한 텍스트 요약 도구 (맵-리듀스 지원)",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
사용 예시:
  %(prog)s meeting.txt                    # 기본 요약
  %(prog)s meeting.txt --verbose          # 상세 로그와 함께
  %(prog)s meeting.txt --json             # JSON 형식 출력
  %(prog)s meeting.txt --model gemma-2-2b-it-Q4_K_M.gguf   # 다른 모델 사용
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
        help=f"사용할 GGUF 모델 파일명 (기본값: {DEFAULT_MODEL})"
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
    parser.add_argument(
        "--target-chunks",
        type=int,
        help="목표 청크 수 (지정시 청크 크기 자동 조정)"
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
            temperature=args.temperature,
            target_chunks=args.target_chunks
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