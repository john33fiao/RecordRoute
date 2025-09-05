# run_workflow.py
import os
import sys
import subprocess
import platform
from pathlib import Path

from logger import setup_logging

setup_logging()

# --- Configuration ---
# 이 스크립트가 있는 디렉토리를 기준으로 경로 설정
BASE_DIR = Path(__file__).parent.resolve()
# Python 실행 파일 자동 감지 (Windows 호환)
PYTHON_EXEC = Path(os.environ.get("PYTHON_EXEC", sys.executable))
OUTPUT_DIR = BASE_DIR.parent / "DB" / "whisper_output"

# 실행할 스크립트 경로
WORKFLOW_DIR = BASE_DIR / "workflow"
TRANSCRIBE_SCRIPT = WORKFLOW_DIR / "transcribe.py"
SUMMARIZE_SCRIPT = WORKFLOW_DIR / "summarize.py"

def run_command(command):
    """주어진 명령어를 실행하고 진행 상황을 출력합니다."""
    print(f"\n--- 실행: {" ".join(map(str, command))} ---")
    try:
        # 실시간 출력을 위해 Popen 사용 (Windows 호환성 개선)
        process = subprocess.Popen(command, stdout=subprocess.PIPE, stderr=subprocess.STDOUT, 
                                 text=True, encoding='utf-8', shell=platform.system() == "Windows")
        while True:
            output = process.stdout.readline()
            if output == '' and process.poll() is not None:
                break
            if output:
                print(output.strip())
        
        rc = process.poll()
        if rc != 0:
            print(f"오류: 명령어가 비정상적으로 종료되었습니다 (종료 코드: {rc})")
            return False
        return True
    except FileNotFoundError:
        print(f"오류: 파이썬 실행 파일을 찾을 수 없습니다. 경로를 확인하세요: {PYTHON_EXEC}")
        return False
    except Exception as e:
        print(f"명령어 실행 중 예외 발생: {e}")
        return False

def run_transcription():
    """STT 변환 단계를 실행하고 생성된 원본 마크다운 파일 목록을 반환합니다."""
    while True:
        input_path_str = input("음성 파일이 있는 폴더의 경로를 입력하세요: ").strip()
        input_path = Path(input_path_str)
        if input_path.is_dir():
            break
        else:
            print(f"오류: '{input_path_str}'는 유효한 폴더가 아닙니다. 다시 입력해주세요.")

    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    files_before = set(OUTPUT_DIR.glob("*.md"))

    if not run_command([PYTHON_EXEC, TRANSCRIBE_SCRIPT, input_path]):
        print("음성 변환 단계에서 오류가 발생하여 중단합니다.")
        return []

    files_after = set(OUTPUT_DIR.glob("*.md"))
    newly_created_files = list(files_after - files_before)

    files_to_process = [p for p in newly_created_files if not p.name.endswith(('.corrected.md', '.summary.md'))]

    if not files_to_process:
        print("새로 변환된 파일이 없어 다음 단계를 진행할 수 없습니다.")
    return files_to_process

def main():
    """메인 워크플로우를 실행합니다."""
    print("---오디오 처리 자동화 워크플로우 시작---")

    # 1. 실행할 과정 선택
    while True:
        print("\n실행할 과정의 번호를 입력하세요.")
        print("  1: 음성 -> 텍스트 변환 (Transcribe)")
        print("  2: 텍스트 요약 (Summarize)")
        steps_to_run = input("입력 (예: 1, 12): ").strip()
        if steps_to_run and all(c in '12' for c in steps_to_run):
            # 중복을 제거하고 순서대로 정렬 (예: '312' -> '123')
            steps_to_run = "".join(sorted(set(steps_to_run)))
            break
        else:
            print("오류: 1, 2의 조합으로만 입력해주세요.")
    
    print(f"선택된 과정: {steps_to_run}")

    files_to_process = []

    # --- 1단계: 음성 -> 텍스트 변환 ---
    if '1' in steps_to_run:
        print("\n--- [단계 1] 음성 변환을 시작합니다 ---")
        files_to_process = run_transcription()
        if not files_to_process and len(steps_to_run) > 1:
            return

    # --- 2단계: 텍스트 요약 ---
    if '2' in steps_to_run:
        print("\n--- [단계 2] 텍스트 요약을 시작합니다 ---")
        # 이전 단계를 거치지 않았다면, STT 실행 여부를 먼저 묻습니다.
        if '1' not in steps_to_run:
            run_stt = input("STT 변환을 먼저 실행하시겠습니까? (y/n): ").strip().lower()
            if run_stt == 'y':
                files_to_process = run_transcription()
            else:
                while True:
                    input_path_str = input("요약할 .md 파일이 있는 폴더의 경로를 입력하세요: ").strip()
                    input_path = Path(input_path_str)
                    if input_path.is_dir():
                        files_to_process = [p for p in input_path.glob("*.md") if not p.name.endswith('.summary.md')]
                        break
                    else:
                        print(f"오류: '{input_path_str}'는 유효한 폴더가 아닙니다. 다시 입력해주세요.")

        if not files_to_process:
            print("요약할 파일이 없습니다.")
            return
        
        print(f"총 {len(files_to_process)}개 파일에 대해 요약을 진행합니다.")
        for file_path in files_to_process:
            if not run_command([PYTHON_EXEC, SUMMARIZE_SCRIPT, file_path]):
                print(f"{file_path.name} 요약 중 오류가 발생하여 중단합니다.")
                return

    print("\n--- 모든 요청된 작업이 성공적으로 완료되었습니다. ---")


if __name__ == "__main__":
    main()
