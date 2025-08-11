# 필요한 라이브러리를 임포트합니다.
# 이 스크립트를 실행하기 전에 'pip install openai-whisper'를 통해 라이브러리를 설치해야 합니다.
import whisper
import os
import argparse
import logging
import traceback
import platform
from pathlib import Path
from concurrent.futures import ThreadPoolExecutor, as_completed
from typing import List, Dict

# Whisper가 지원하는 파일 확장자 목록
SUPPORTED_EXTS = {'.flac', '.m4a', '.mp3', '.mp4', '.mpeg', '.mpga', '.oga', '.ogg', '.wav', '.webm'}

def list_media_files(root: Path, recursive: bool):
    """지원하는 미디어 파일 목록을 반환합니다."""
    iterator = root.rglob("*") if recursive else root.iterdir()
    return [f for f in iterator if f.is_file() and f.suffix.lower() in SUPPORTED_EXTS]

def write_atomic(path: Path, data: str):
    """원자적 파일 쓰기: 임시 파일에 작성 후 rename으로 안전하게 저장"""
    tmp_path = path.with_suffix(path.suffix + ".tmp")
    with open(tmp_path, "w", encoding="utf-8") as f:
        f.write(data)
    tmp_path.replace(path)

def normalize_text(text: str, normalize_punct: bool) -> str:
    """텍스트 정규화: 생략부호 및 공백 처리"""
    text = text.strip()
    
    if normalize_punct:
        # 연속된 마침표 4개 이상을 '...'로 정규화
        while "...." in text:
            text = text.replace("....", "...")
        
        # 연속 공백을 단일 공백으로 정규화
        import re
        text = re.sub(r'\s+', ' ', text)
    
    return text

def merge_segments(segments, max_gap: float = 0.2):
    """
    연속 세그먼트가 동일한 텍스트이고 시간 간격이 max_gap 이내이면 병합
    시간 연속성과 텍스트 동일성을 모두 고려하여 안전하게 병합
    """
    if not segments:
        return []
    
    # 필수 키 존재 검증 (방어 코딩)
    safe_segments = []
    for segment in segments:
        if "text" in segment and "start" in segment and "end" in segment:
            safe_segments.append(segment)
        else:
            logging.warning("세그먼트에 필수 키(text, start, end)가 누락되어 건너뜁니다: %s", segment)
    
    if not safe_segments:
        return []
    
    merged = [safe_segments[0].copy()]
    
    for segment in safe_segments[1:]:
        current = merged[-1]
        same_text = segment["text"].strip() == current["text"].strip()
        time_continuous = segment["start"] <= current["end"] + max_gap
        
        if same_text and time_continuous:
            # 병합: 종료 시간을 더 늦은 것으로 업데이트
            current["end"] = max(current["end"], segment["end"])
        else:
            merged.append(segment.copy())
    
    return merged

def should_keep_segment(text: str, enable_filter: bool, min_length: int):
    """세그먼트 유지 여부를 판단합니다."""
    text = text.strip()
    
    # 빈 텍스트 제거
    if not text:
        return False
    
    # 최소 길이 검사
    if len(text) < min_length:
        return False
    
    # 필터링이 비활성화되면 유지
    if not enable_filter:
        return True
    
    # 보수적 필러 필터: 단독으로 나타나는 필러만 제거
    filler_words = {"아", "으", "음", "어", "저", "그", "뭐", "얍", "흠", "네", "예"}
    return text not in filler_words

def get_unique_output_path(base_path: Path) -> Path:
    """파일명 충돌 시 접미사를 붙여 고유한 경로를 반환"""
    if not base_path.exists():
        return base_path
    
    counter = 1
    stem = base_path.stem
    suffix = base_path.suffix
    parent = base_path.parent
    
    while True:
        new_path = parent / f"{stem}_{counter}{suffix}"
        if not new_path.exists():
            return new_path
        counter += 1


def diarize_audio(file_path: Path) -> List[Dict[str, float]]:
    """pyannote.audio를 사용하여 화자 구간을 추출합니다."""
    try:
        from pyannote.audio import Pipeline
    except ImportError:
        logging.error("pyannote.audio가 설치되어 있지 않아 화자 구분을 건너뜁니다.")
        return []

    token = os.getenv("PYANNOTE_TOKEN")
    try:
        pipeline = Pipeline.from_pretrained(
            "pyannote/speaker-diarization", use_auth_token=token
        )
    except Exception as e:
        logging.error("화자 구분 파이프라인 로드 실패: %s", e)
        return []

    try:
        diarization = pipeline(str(file_path))
    except Exception as e:
        logging.error("화자 구분 수행 중 오류: %s", e)
        return []

    segments: List[Dict[str, float]] = []
    for turn, _, speaker in diarization.itertracks(yield_label=True):
        segments.append({
            "start": float(turn.start),
            "end": float(turn.end),
            "speaker": speaker,
        })
    return segments

def transcribe_single_file(file_path: Path, output_dir: Path, model,
                          language: str, initial_prompt: str,
                          filter_fillers: bool, min_seg_length: int,
                          normalize_punct: bool, diarize: bool):
    """단일 파일을 변환하고 결과를 저장합니다."""
    
    # 출력 파일 경로 결정 (중복 방지)
    base_output_path = output_dir / f"{file_path.stem}.md"
    output_file_path = get_unique_output_path(base_output_path)

    # 화자 구분 수행
    speaker_segments = diarize_audio(file_path) if diarize else []

    # Whisper 변환 실행
    transcribe_params = {
        "fp16": False,  # Apple Silicon 안정성 고려
        "verbose": False,
        "temperature": 0.0
    }
    
    if language:
        transcribe_params["language"] = language
    if initial_prompt:
        transcribe_params["initial_prompt"] = initial_prompt

    result = model.transcribe(str(file_path), **transcribe_params)

    # 세그먼트 처리
    segments = result.get("segments", []) or []
    segments = merge_segments(segments, max_gap=0.2)

    # 화자 정보 매칭
    if speaker_segments:
        for segment in segments:
            mid = (segment.get("start", 0.0) + segment.get("end", 0.0)) / 2
            for sp in speaker_segments:
                if sp["start"] <= mid <= sp["end"]:
                    segment["speaker"] = sp["speaker"]
                    break

    speaker_map: Dict[str, int] = {}
    next_id = 1
    for segment in segments:
        label = segment.get("speaker")
        if label and label not in speaker_map:
            speaker_map[label] = next_id
            next_id += 1
        segment["speaker_id"] = speaker_map.get(label, 0)

    # 필터링 및 정규화
    processed_segments = []
    for segment in segments:
        text = segment.get("text", "").strip()

        if not should_keep_segment(text, filter_fillers, min_seg_length):
            continue

        text = normalize_text(text, normalize_punct)
        processed_segments.append((segment.get("speaker_id", 0), text))

    # 마크다운 생성
    markdown_content = f"# {file_path.stem}\n\n"

    if processed_segments:
        lines = []
        for spk_id, text in processed_segments:
            if diarize and spk_id:
                lines.append(f"Speaker {spk_id}: {text}")
            else:
                lines.append(text)
        markdown_content += "\n".join(lines)
    else:
        # 모든 세그먼트가 필터링된 경우 원본 텍스트 사용 (문장부호 보존)
        original_text = result.get("text", "")
        markdown_content += normalize_text(original_text, normalize_punct)

    # 원자적 저장
    write_atomic(output_file_path, markdown_content)
    return output_file_path

def transcribe_audio_files(input_dir: str, output_dir: str, model_identifier: str,
                          language: str, initial_prompt: str, workers: int,
                          recursive: bool, filter_fillers: bool,
                          min_seg_length: int, normalize_punct: bool,
                          diarize: bool):
    """
    지정된 입력 디렉토리 내의 모든 오디오/비디오 파일을 Whisper를 사용하여 
    텍스트로 변환하고, 변환된 텍스트를 마크다운(.md) 파일로 저장합니다.
    
    Args:
        input_dir (str): 오디오/비디오 파일이 있는 입력 디렉토리 경로
        output_dir (str): 변환된 마크다운 파일을 저장할 출력 디렉토리 경로
        model_identifier (str): 사용할 Whisper 모델의 이름 또는 .pt 파일의 전체 경로
        language (str): 언어 힌트 (예: 'ko')
        initial_prompt (str): 도메인 특화 용어 힌트
        workers (int): 동시 처리 파일 수
        recursive (bool): 하위 폴더 포함 여부
        filter_fillers (bool): 필러 단어 필터링 활성화 여부
        min_seg_length (int): 세그먼트 최소 길이
        normalize_punct (bool): 연속 마침표 정규화 여부
        diarize (bool): 화자 구분 활성화 여부
    """
    
    input_path_obj = Path(input_dir)
    output_path_obj = Path(output_dir)
    
    # 출력 디렉토리 생성
    output_path_obj.mkdir(parents=True, exist_ok=True)
    logging.info("출력 디렉토리: %s", output_path_obj.resolve())

    # 처리할 파일 목록 수집
    files_to_process = list_media_files(input_path_obj, recursive=recursive)
    
    if not files_to_process:
        logging.info("입력 디렉토리 '%s'에 처리할 파일이 없습니다.", input_path_obj.resolve())
        logging.info("스크립트를 종료합니다.")
        return

    logging.info("처리 대상 파일 수: %d개", len(files_to_process))
    if recursive:
        logging.info("하위 폴더 포함 검색 활성화")

    # Whisper 모델 로드
    logging.info("'%s' 모델을 로드하는 중...", os.path.basename(model_identifier))
    try:
        model = whisper.load_model(model_identifier)
        logging.info("모델 로드 완료.")
    except Exception as e:
        logging.error("모델 로딩 중 오류 발생: %s", e)
        logging.error("스크립트를 종료합니다. 모델 이름이나 경로가 올바른지 확인하세요.")
        return

    # 변환 실행
    failures = []
    
    if workers <= 1:
        # 순차 처리
        for file_path in files_to_process:
            logging.info("'%s' 파일 변환 시작", file_path.name)
            try:
                output_path = transcribe_single_file(
                    file_path, output_path_obj, model, language, initial_prompt,
                    filter_fillers, min_seg_length, normalize_punct, diarize
                )
                logging.info("변환 완료: %s → %s", file_path.name, output_path.name)
            except Exception as e:
                failures.append((file_path, str(e)))
                logging.error("변환 실패: %s", file_path.name, exc_info=True)
    else:
        # 병렬 처리 (주의: 단일 GPU/MPS/CPU에서는 비권장)
        logging.warning("병렬 처리 모드 활성화 (workers=%d). 단일 GPU/MPS/CPU에서는 성능 향상이 제한적일 수 있습니다.", workers)
        with ThreadPoolExecutor(max_workers=workers) as executor:
            # 작업 제출
            futures = {
                executor.submit(
                    transcribe_single_file, file_path, output_path_obj, model,
                    language, initial_prompt, filter_fillers, min_seg_length,
                    normalize_punct, diarize
                ): file_path for file_path in files_to_process
            }
            
            # 결과 수집
            for future in as_completed(futures):
                file_path = futures[future]
                try:
                    output_path = future.result()
                    logging.info("변환 완료: %s → %s", file_path.name, output_path.name)
                except Exception as e:
                    failures.append((file_path, str(e)))
                    logging.error("변환 실패: %s", file_path.name, exc_info=True)

    # 최종 결과 요약
    logging.info("="*50)
    logging.info("변환 작업 완료")
    logging.info("="*50)
    logging.info("전체 파일: %d개", len(files_to_process))
    logging.info("성공: %d개", len(files_to_process) - len(failures))
    logging.info("실패: %d개", len(failures))
    
    if failures:
        logging.error("실패한 파일 목록:")
        for file_path, error_msg in failures:
            logging.error("- %s: %s", file_path.name, error_msg)
    
    logging.info("모든 파일 변환 처리가 완료되었습니다.")

def main():
    """명령행 인자를 파싱하고 메인 함수를 실행합니다."""
    parser = argparse.ArgumentParser(
        description="Whisper를 사용하여 디렉토리의 오디오/비디오 파일을 텍스트로 변환합니다.",
        formatter_class=argparse.RawTextHelpFormatter
    )
    
    # 필수 인자
    parser.add_argument(
        "input_dir",
        type=str,
        help="음성 파일이 있는 폴더의 경로"
    )
    
    # 기본 옵션
    parser.add_argument(
        "--output_dir",
        type=str,
        default="./whisper_output",
        help="변환된 마크다운 파일을 저장할 출력 디렉토리 경로\n(기본값: ./whisper_output)"
    )
    
    parser.add_argument(
        "--model_size",
        type=str,
        default="large-v3-turbo",
        choices=['tiny', 'base', 'small', 'medium', 'large', 'large-v3-turbo'],
        help="사용할 Whisper 모델의 크기 또는 종류\n(기본값: large-v3-turbo)"
    )
    
    # 언어 및 품질 옵션
    parser.add_argument(
        "--language",
        type=str,
        default="ko",
        help="언어 힌트 (예: ko). 자동 감지를 원하면 빈 값으로 설정\n(기본값: ko)"
    )
    
    parser.add_argument(
        "--initial_prompt",
        type=str,
        default="",
        help="도메인 특화 용어 힌트 (예: '항공, 관제, ILS, VOR, 활주로')"
    )
    
    # 성능 옵션
    parser.add_argument(
        "--workers",
        type=int,
        default=1,
        help="동시 처리 파일 수 (기본값: 1)\n단일 GPU/MPS/CPU에서 병렬 추론은 비권장. 기본값 사용 권장."
    )
    
    # 파일 처리 옵션
    parser.add_argument(
        "--recursive",
        action="store_true",
        help="하위 폴더 포함 검색"
    )
    
    # 텍스트 처리 옵션
    parser.add_argument(
        "--filter_fillers",
        action="store_true",
        help="단독 필러 어절 제거 활성화 (예: '아', '음', '어' 등)"
    )
    
    parser.add_argument(
        "--min_seg_length",
        type=int,
        default=2,
        help="세그먼트 최소 길이 (문자 수, 기본값: 2)"
    )
    
    parser.add_argument(
        "--normalize_punct",
        action="store_true",
        help="과도한 연속 마침표를 '...'로 정규화"
    )

    parser.add_argument(
        "--diarize",
        action="store_true",
        help="pyannote.audio를 사용하여 화자 구분 수행"
    )
    
    args = parser.parse_args()

    # 모델 경로 결정 (플랫폼별 캐시 경로 지원)
    model_to_use = args.model_size
    if args.model_size == 'large-v3-turbo':
        # 플랫폼별 캐시 경로 결정
        if platform.system() == "Windows":
            # Windows: %USERPROFILE%\.cache\whisper\
            cache_dir = Path(os.environ.get("USERPROFILE", Path.home())) / ".cache" / "whisper"
        else:
            # macOS/Linux: ~/.cache/whisper/
            cache_dir = Path.home() / ".cache" / "whisper"
        
        # turbo 모델 파일 검색 (다양한 파일명 패턴 지원)
        model_path = None
        turbo_patterns = ["large-v3-turbo.pt", "whisper-turbo.pt", "turbo.pt"]
        
        for pattern in turbo_patterns:
            candidate_path = cache_dir / pattern
            if candidate_path.exists():
                model_path = candidate_path
                break
        
        if model_path and model_path.exists():
            model_to_use = str(model_path)
            logging.info("turbo 모델 파일을 찾았습니다: %s", model_path)
        else:
            logging.warning("turbo 모델 파일을 찾을 수 없습니다. 검색 경로: %s", cache_dir)
            logging.warning("검색한 파일명: %s", ', '.join(turbo_patterns))
            logging.warning("자동으로 'large' 모델을 사용합니다.")
            model_to_use = "large"

    # 입력 경로 검증
    input_path = Path(args.input_dir)
    if not input_path.is_dir():
        logging.error("입력한 경로 '%s'는 폴더가 아니거나 존재하지 않습니다.", args.input_dir)
        return

    # 로깅 설정
    logging.basicConfig(
        level=logging.INFO,
        format='%(asctime)s - %(levelname)s - %(message)s'
    )

    # 메인 변환 함수 실행
    transcribe_audio_files(
        input_dir=str(input_path),
        output_dir=args.output_dir,
        model_identifier=model_to_use,
        language=args.language if args.language else None,
        initial_prompt=args.initial_prompt,
        workers=max(1, args.workers),
        recursive=args.recursive,
        filter_fillers=args.filter_fillers,
        min_seg_length=max(2, args.min_seg_length),
        normalize_punct=args.normalize_punct,
        diarize=args.diarize
    )

if __name__ == "__main__":
    main()