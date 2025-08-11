# correct.py
import argparse
import logging
from pathlib import Path
import sys
import time
import re
import ollama
from typing import List, Optional

DEFAULT_MODEL = "gpt-oss:20b"

SYSTEM_PROMPT = (
    "당신은 한국어 텍스트를 전문적으로 교정하는 편집자입니다. "
    "원문 의미와 사실을 보존하고, 오탈자/문법/어법을 고치되 새로운 정보를 추가하지 마십시오. "
    "마크다운 구조(헤더, 목록, 코드블록, 표, 줄바꿈)를 최대한 보존하고 병합하지 마십시오. "
    "발화자 표기와 타임스탬프가 있으면 유지하십시오. "
    "중복어, 군말(음, 어, 아 등)은 삭제하되 의미 손실은 피하십시오. "
    "수치/단위/고유명사는 그대로 보존하십시오. "
    "최종 출력은 교정된 한국어 텍스트 '전문'만 포함하십시오."
)

USER_PROMPT_TEMPLATE = """아래는 교정 대상 텍스트입니다. 삼중 백틱 내부만 교정하여 돌려주십시오.

```markdown
{content}
```"""

CORRECTION_INSTRUCTIONS = (
    "주의: 설명, 사과, 인사, 메타 코멘트, 요약을 출력하지 마세요. "
    "오직 교정 결과 텍스트만 반환하세요."
)

def setup_logging(verbose: bool = False):
    """로깅 설정"""
    level = logging.DEBUG if verbose else logging.INFO
    format_str = "%(levelname)s: %(message)s"
    logging.basicConfig(level=level, format=format_str)

def read_text(path: Path, encoding: str = "utf-8") -> str:
    """안전한 텍스트 파일 읽기 (인코딩 후퇴 전략 포함)"""
    encodings_to_try = [encoding]
    if encoding == "utf-8":
        encodings_to_try.extend(["utf-8-sig", "cp949", "euc-kr"])
    
    last_error = None
    for enc in encodings_to_try:
        try:
            content = path.read_text(encoding=enc)
            if enc != encoding:
                logging.info("인코딩 %s로 파일을 성공적으로 읽었습니다.", enc)
            return content
        except UnicodeDecodeError as e:
            last_error = e
            logging.debug("인코딩 %s 실패: %s", enc, e)
            continue
    
    raise UnicodeDecodeError(f"모든 인코딩 시도 실패. 마지막 오류: {last_error}")

def maybe_strip_heading(text: str, strip_heading: bool) -> str:
    """옵션에 따라 첫 번째 H1 헤더만 제거"""
    if not strip_heading:
        return text
    
    lines = text.splitlines()
    if lines and lines[0].startswith("# "):
        # H1만 제거하고, 연속된 빈 줄은 하나만 남김
        i = 1
        while i < len(lines) and lines[i].strip() == "":
            i += 1
        return "\n".join(lines[i:])
    
    return text

def smart_chunk_text(text: str, max_chars: int = 8000, overlap: int = 300) -> List[str]:
    """문단 경계를 고려한 지능형 청킹"""
    if len(text) <= max_chars:
        return [text]
    
    chunks = []
    # 먼저 문단으로 분할 시도
    paragraphs = text.split('\n\n')
    
    current_chunk = ""
    for para in paragraphs:
        # 현재 청크에 문단을 추가했을 때 길이 확인
        test_chunk = current_chunk + ("\n\n" if current_chunk else "") + para
        
        if len(test_chunk) <= max_chars:
            current_chunk = test_chunk
        else:
            # 현재 청크가 비어있지 않으면 저장
            if current_chunk:
                chunks.append(current_chunk)
                # 오버랩을 위해 현재 청크의 끝부분을 다음 청크 시작에 포함
                if len(current_chunk) > overlap:
                    overlap_text = current_chunk[-overlap:]
                    current_chunk = overlap_text + "\n\n" + para
                else:
                    current_chunk = para
            else:
                # 단일 문단이 너무 긴 경우 문장 단위로 분할
                if len(para) > max_chars:
                    sentences = re.split(r'(?<=[.!?])\s+', para)
                    sentence_chunk = ""
                    for sentence in sentences:
                        if len(sentence_chunk + sentence) <= max_chars:
                            sentence_chunk += (" " if sentence_chunk else "") + sentence
                        else:
                            if sentence_chunk:
                                chunks.append(sentence_chunk)
                            sentence_chunk = sentence
                    if sentence_chunk:
                        current_chunk = sentence_chunk
                else:
                    current_chunk = para
    
    # 마지막 청크 추가
    if current_chunk:
        chunks.append(current_chunk)
    
    return chunks

def validate_correction(original: str, corrected: str) -> bool:
    """교정 결과 품질 검증"""
    if not corrected.strip():
        logging.warning("교정 결과가 비어있습니다.")
        return False
    
    # 길이 비율 검증 (너무 많이 축약되거나 확장되지 않았는지)
    ratio = len(corrected) / len(original) if len(original) > 0 else 0
    if ratio < 0.5 or ratio > 2.0:
        logging.warning("교정 결과 길이가 원본과 너무 다릅니다. 비율: %.2f", ratio)
        return False
    
    return True

def chat_once(model: str, system: str, user: str, temperature: float = 0.0, 
              num_ctx: int = 8192, retries: int = 3, backoff: float = 1.5) -> str:
    """단일 교정 요청 (재시도 및 에러 처리 포함)"""
    last_error = None
    
    for attempt in range(1, retries + 1):
        try:
            logging.debug("모델 %s에 요청 중... (시도 %d/%d)", model, attempt, retries)
            
            resp = ollama.chat(
                model=model,
                messages=[
                    {"role": "system", "content": system},
                    {"role": "user", "content": user},
                    {"role": "user", "content": CORRECTION_INSTRUCTIONS},
                ],
                options={
                    "temperature": temperature,
                    "num_ctx": num_ctx,
                },
                stream=False,
            )
            
            # 응답 검증
            if not isinstance(resp, dict):
                raise RuntimeError("예상하지 못한 응답 형식입니다.")
            
            message = resp.get("message", {})
            content = message.get("content")
            
            if not content:
                raise RuntimeError("모델 응답에 content가 없습니다.")
            
            # 기본적인 응답 정제 (마크다운 코드 블록 제거)
            content = content.strip()
            if content.startswith("```") and content.endswith("```"):
                lines = content.split('\n')
                if len(lines) > 2:
                    content = '\n'.join(lines[1:-1])
            
            logging.debug("교정 완료. 응답 길이: %d자", len(content))
            return content
            
        except Exception as e:
            last_error = e
            logging.warning("시도 %d/%d 실패: %s", attempt, retries, str(e))
            
            if attempt < retries:
                sleep_time = backoff ** attempt
                logging.info("%.1f초 후 재시도합니다...", sleep_time)
                time.sleep(sleep_time)
    
    raise RuntimeError(f"모델 통신 {retries}회 시도 모두 실패: {last_error}")

def correct_text_file(input_file: Path, output_file: Optional[Path] = None, 
                     model: str = DEFAULT_MODEL, temperature: float = 0.0, 
                     num_ctx: int = 8192, encoding: str = "utf-8", 
                     strip_heading: bool = False, inplace: bool = False, 
                     max_chunk_chars: int = 8000) -> bool:
    """단일 파일 교정"""
    try:
        logging.info("파일 읽는 중: %s", input_file)
        
        # 파일 존재 확인
        if not input_file.exists():
            logging.error("파일을 찾을 수 없습니다: %s", input_file)
            return False
        
        # 텍스트 읽기
        original_text = read_text(input_file, encoding=encoding)
        
        if not original_text.strip():
            logging.warning("입력 파일이 비어있거나 공백만 포함되어 있습니다.")
            return False
        
        # 헤더 제거 (옵션)
        target_text = maybe_strip_heading(original_text, strip_heading=strip_heading)
        
        # 청킹
        chunks = smart_chunk_text(target_text, max_chars=max_chunk_chars, overlap=300)
        logging.info("총 %d개 청크로 분할하여 처리합니다.", len(chunks))
        
        # 각 청크 교정
        corrected_chunks = []
        for idx, chunk in enumerate(chunks, 1):
            logging.info("청크 %d/%d 교정 중... (%d자)", idx, len(chunks), len(chunk))
            
            user_prompt = USER_PROMPT_TEMPLATE.format(content=chunk)
            corrected_chunk = chat_once(
                model=model, 
                system=SYSTEM_PROMPT, 
                user=user_prompt, 
                temperature=temperature, 
                num_ctx=num_ctx
            )
            
            # 교정 결과 검증
            if not validate_correction(chunk, corrected_chunk):
                logging.warning("청크 %d 교정 결과에 문제가 있을 수 있습니다.", idx)
            
            corrected_chunks.append(corrected_chunk)
        
        # 결과 결합
        final_text = "".join(corrected_chunks)
        
        # 출력 처리
        if inplace:
            # 백업 생성 후 원본 파일에 덮어쓰기
            backup_path = input_file.with_suffix(input_file.suffix + ".bak")
            logging.info("원본 파일을 %s로 백업합니다.", backup_path)
            input_file.rename(backup_path)
            input_file.write_text(final_text, encoding="utf-8")
            logging.info("원본 파일에 교정 결과를 저장했습니다: %s", input_file)
        else:
            # 지정된 출력 파일에 저장
            if output_file is None:
                output_file = input_file.with_name(f"{input_file.stem}.corrected.md")
            
            # 출력 디렉토리 생성
            output_file.parent.mkdir(parents=True, exist_ok=True)
            output_file.write_text(final_text, encoding="utf-8")
            logging.info("교정 결과를 저장했습니다: %s", output_file)
        
        return True
        
    except Exception as e:
        logging.error("파일 교정 중 오류 발생: %s", str(e))
        return False

def process_multiple_files(input_files: List[Path], **kwargs) -> None:
    """여러 파일 배치 처리"""
    success_count = 0
    total_count = len(input_files)
    
    logging.info("총 %d개 파일 배치 처리를 시작합니다.", total_count)
    
    for i, input_file in enumerate(input_files, 1):
        logging.info("진행률: %d/%d - %s", i, total_count, input_file.name)
        
        try:
            if correct_text_file(input_file, **kwargs):
                success_count += 1
                logging.info("✓ 성공: %s", input_file.name)
            else:
                logging.error("✗ 실패: %s", input_file.name)
        except KeyboardInterrupt:
            logging.info("사용자에 의해 중단되었습니다.")
            break
        except Exception as e:
            logging.error("✗ 오류: %s - %s", input_file.name, str(e))
    
    logging.info("배치 처리 완료: %d/%d 성공", success_count, total_count)

def main():
    """메인 함수"""
    parser = argparse.ArgumentParser(
        description="Ollama를 사용해 한국어 텍스트를 문어체로 교정합니다.",
        formatter_class=argparse.RawDescriptionHelpFormatter,
        epilog="""
사용 예시:
  %(prog)s document.txt                    # 기본 교정
  %(prog)s -o output.md input.txt          # 출력 파일 지정
  %(prog)s --inplace document.txt          # 원본 파일 덮어쓰기 (백업 생성)
  %(prog)s --strip-heading meeting.md     # 첫 번째 헤더 제거
  %(prog)s -m llama2 --temperature 0.1 *.txt  # 모델과 옵션 지정
        """
    )
    
    # 입력 파일
    parser.add_argument("input_files", nargs="+", type=Path, 
                       help="입력 파일 경로 (여러 파일 지원)")
    
    # 출력 옵션
    parser.add_argument("-o", "--output", type=Path, 
                       help="출력 파일 경로 (.md). 미지정 시 <원본명>.corrected.md")
    parser.add_argument("--inplace", action="store_true", 
                       help="백업 생성 후 같은 파일에 덮어쓰기")
    
    # 모델 옵션
    parser.add_argument("-m", "--model", default=DEFAULT_MODEL, 
                       help=f"사용할 Ollama 모델 (기본: {DEFAULT_MODEL})")
    parser.add_argument("--temperature", type=float, default=0.0, 
                       help="샘플링 온도 (기본: 0.0, 재현성 확보)")
    parser.add_argument("--num-ctx", type=int, default=8192, 
                       help="컨텍스트 길이 (기본: 8192)")
    
    # 텍스트 처리 옵션
    parser.add_argument("--encoding", default="utf-8", 
                       help="입력 파일 인코딩 (기본: utf-8, 자동 후퇴 지원)")
    parser.add_argument("--strip-heading", action="store_true", 
                       help="맨 앞 H1 헤더(# )만 제거")
    parser.add_argument("--max-chars", type=int, default=8000, 
                       help="청크 최대 문자수 (기본: 8000)")
    
    # 로깅 옵션
    parser.add_argument("-v", "--verbose", action="store_true", 
                       help="상세 로그 출력")
    
    args = parser.parse_args()
    
    # 로깅 설정
    setup_logging(args.verbose)
    
    # 입력 검증
    input_files = []
    for file_path in args.input_files:
        if file_path.exists():
            input_files.append(file_path)
        else:
            logging.error("파일을 찾을 수 없습니다: %s", file_path)
    
    if not input_files:
        logging.error("처리할 파일이 없습니다.")
        sys.exit(1)
    
    # 출력 파일 옵션 검증
    if len(input_files) > 1 and args.output:
        logging.error("여러 파일 처리 시에는 --output 옵션을 사용할 수 없습니다.")
        sys.exit(1)
    
    # 공통 옵션
    common_kwargs = {
        "model": args.model,
        "temperature": args.temperature,
        "num_ctx": args.num_ctx,
        "encoding": args.encoding,
        "strip_heading": args.strip_heading,
        "inplace": args.inplace,
        "max_chunk_chars": args.max_chars,
    }
    
    try:
        if len(input_files) == 1:
            # 단일 파일 처리
            success = correct_text_file(
                input_file=input_files[0],
                output_file=args.output,
                **common_kwargs
            )
            sys.exit(0 if success else 1)
        else:
            # 다중 파일 배치 처리
            process_multiple_files(input_files, **common_kwargs)
    
    except KeyboardInterrupt:
        logging.info("사용자에 의해 프로그램이 중단되었습니다.")
        sys.exit(130)
    except Exception as e:
        logging.error("예상치 못한 오류가 발생했습니다: %s", str(e))
        if args.verbose:
            import traceback
            traceback.print_exc()
        sys.exit(1)

if __name__ == "__main__":
    main()