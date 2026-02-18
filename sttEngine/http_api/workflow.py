from __future__ import annotations

import subprocess
from datetime import datetime
from pathlib import Path

from ..obsidian_mcp import send_summary_to_obsidian_sync
from ..workflow.transcribe import transcribe_audio_files
from ..workflow.correct import correct_text_file
from ..workflow.summarize import (
    DEFAULT_CHUNK_SIZE,
    DEFAULT_MODEL,
    DEFAULT_TEMPERATURE,
    read_text_with_fallback,
    save_output,
    summarize_text_mapreduce,
)
from .embedding import generate_embedding
from .history import load_upload_history
from .paths import OUTPUT_DIR, UPLOAD_DIR, to_record_path
from .registry import generate_and_store_title_summary, update_task_completion
from ..server.services.errors import map_workflow_exception
from .state import (
    clear_task_progress,
    is_task_cancelled,
    unregister_process,
    update_task_progress,
    TaskStage,
)




def _workflow_error_result(task_id: str | None, exc: Exception, failed_step: str):
    mapped = map_workflow_exception(exc, failed_step)
    if task_id:
        update_task_progress(
            task_id,
            f"{failed_step} 실패: {mapped.message}",
            stage=TaskStage.CORRECT if (mapped.failed_step or failed_step)=="correct" else (TaskStage.SUMMARY if (mapped.failed_step or failed_step)=="summary" else TaskStage.TRANSFORM),
            error_code=mapped.code,
            retryable=mapped.retryable,
            failed_step=mapped.failed_step or failed_step,
        )
    return {
        "error": mapped.message,
        "error_code": mapped.code,
        "retryable": mapped.retryable,
        "failed_step": mapped.failed_step or failed_step,
    }

def get_file_type(file_path: Path) -> str:
    """Determine if the file is audio or text."""
    audio_extensions = {
        ".flac",
        ".m4a",
        ".mp3",
        ".mp4",
        ".mpeg",
        ".mpga",
        ".oga",
        ".ogg",
        ".qta",
        ".wav",
        ".webm",
    }
    text_extensions = {".md", ".txt", ".text", ".markdown"}
    pdf_extensions = {".pdf"}

    suffix = file_path.suffix.lower()
    if suffix in audio_extensions:
        return "audio"
    if suffix in text_extensions:
        return "text"
    if suffix in pdf_extensions:
        return "pdf"
    return "unknown"


def get_audio_duration(file_path: Path) -> str | None:
    """Get audio file duration using ffprobe."""
    try:
        result = subprocess.run(
            [
                "ffprobe",
                "-v",
                "quiet",
                "-show_entries",
                "format=duration",
                "-of",
                "csv=p=0",
                str(file_path),
            ],
            capture_output=True,
            text=True,
            check=True,
        )
        duration = float(result.stdout.strip())
        minutes = int(duration // 60)
        seconds = int(duration % 60)
        return f"{minutes:02d}:{seconds:02d}"
    except (subprocess.CalledProcessError, ValueError, FileNotFoundError):
        return None


def find_existing_stt_file(original_file_path: Path) -> Path | None:
    """Find existing STT result file for the given original file."""
    stem = original_file_path.stem

    # Extract UUID from the original file path (DB/uploads/UUID/filename)
    upload_uuid = original_file_path.parent.name
    print(f"[DEBUG] 업로드 UUID: {upload_uuid}")

    # Look for STT file in whisper_output/UUID/filename.md
    stt_output_dir = OUTPUT_DIR / upload_uuid
    potential_files = [stt_output_dir / f"{stem}.md", stt_output_dir / f"{stem}.corrected.md"]

    for stt_file in potential_files:
        if stt_file.exists() and not stt_file.name.endswith(".summary.md"):
            print(f"[DEBUG] STT 파일 발견: {stt_file}")
            return stt_file

    print(f"[DEBUG] '{stem}.md' STT 파일을 찾지 못함 (경로: {stt_output_dir})")
    return None


def run_workflow(
    file_path: Path,
    steps,
    record_id: str | None = None,
    task_id: str | None = None,
    model_settings: dict | None = None,
):
    """Run the requested workflow steps sequentially."""

    results = {}
    current_file = file_path
    file_type = get_file_type(file_path)

    # Create individual output directory based on upload folder structure
    upload_folder_name = current_file.parent.name  # Get UUID folder name
    individual_output_dir = OUTPUT_DIR / upload_folder_name
    individual_output_dir.mkdir(exist_ok=True)

    def run_diarize_step(source_file: Path) -> dict:
        """Run diarization for audio inputs and return a normalized result payload."""
        if file_type != "audio":
            return {
                "status": "skipped",
                "reason": "non_audio_input",
                "input_file_type": file_type,
                "segments": [],
            }

        duration = get_audio_duration(source_file)
        duration_seconds: float | None = None
        if duration:
            minutes, seconds = duration.split(":")
            duration_seconds = float(int(minutes) * 60 + int(seconds))

        # NOTE:
        # - This is a lightweight baseline diarization payload for contract stability.
        # - Future diarization backends should preserve this top-level schema.
        segments = []
        if duration_seconds and duration_seconds > 0:
            segments.append(
                {
                    "speaker": "SPEAKER_00",
                    "start": 0.0,
                    "end": duration_seconds,
                    "confidence": 1.0,
                }
            )

        return {
            "status": "completed",
            "input_file_type": file_type,
            "duration": duration,
            "segments": segments,
        }

    try:
        # For text files, skip STT step and copy to output directory
        if file_type == "text":
            if "stt" in steps:
                # Check if task was cancelled
                if task_id and is_task_cancelled(task_id):
                    return {"error": "Task was cancelled"}

                # For text files, we already have the text content, so just copy it to output
                text_file = individual_output_dir / f"{file_path.stem}.md"
                import shutil

                shutil.copy2(file_path, text_file)
                download_url = f"/download/{upload_folder_name}/{text_file.name}"
                results["stt"] = download_url
                current_file = text_file

                if record_id:
                    file_path_str = to_record_path(text_file)
                    update_task_completion(record_id, "stt", file_path_str)
            else:
                text_file = individual_output_dir / f"{file_path.stem}.md"
                import shutil

                shutil.copy2(file_path, text_file)
                current_file = text_file

        # For PDF files, extract text and treat as markdown
        elif file_type == "pdf":
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            try:
                from pypdf import PdfReader

                reader = PdfReader(str(current_file))
                pdf_text = "\n".join(page.extract_text() or "" for page in reader.pages)
            except Exception as e:
                print(f"PDF text extraction failed: {e}")
                return _workflow_error_result(task_id, e, "stt")

            text_file = individual_output_dir / f"{file_path.stem}.md"
            text_file.write_text(pdf_text, encoding="utf-8")

            if "stt" in steps:
                download_url = f"/download/{upload_folder_name}/{text_file.name}"
                results["stt"] = download_url
                if record_id:
                    file_path_str = to_record_path(text_file)
                    update_task_completion(record_id, "stt", file_path_str)

            current_file = text_file

        # For audio files, run STT step
        elif file_type == "audio" and "stt" in steps:
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            print(f"Starting STT for task {task_id}")

            def progress_callback(message):
                if task_id:
                    update_task_progress(task_id, message, stage=TaskStage.TRANSFORM)

            whisper_model = "large-v3-turbo"
            if model_settings and model_settings.get("whisper"):
                whisper_model = model_settings["whisper"]

            language = "ko"
            if model_settings and model_settings.get("language") is not None:
                lang = model_settings.get("language")
                if lang in ("", "auto"):
                    language = None
                else:
                    language = lang

            device_choice = "auto"
            if model_settings and model_settings.get("device"):
                device_choice = model_settings.get("device")

            try:
                transcribe_audio_files(
                    input_dir=str(current_file.parent),
                    output_dir=str(individual_output_dir),
                    model_identifier=whisper_model,
                    language=language,
                    initial_prompt="",
                    workers=1,
                    recursive=False,
                    filter_fillers=False,
                    min_seg_length=2,
                    normalize_punct=False,
                    requested_device=device_choice,
                    progress_callback=progress_callback,
                )
            except Exception as e:
                print(f"STT process failed: {e}")
                if task_id:
                    update_task_progress(task_id, f"STT 실패: {e}", stage=TaskStage.TRANSFORM)
                return _workflow_error_result(task_id, e, "stt")

            stt_file = individual_output_dir / f"{file_path.stem}.md"
            download_url = f"/download/{upload_folder_name}/{stt_file.name}"
            results["stt"] = download_url
            current_file = stt_file

            if record_id:
                file_path_str = to_record_path(stt_file)
                update_task_completion(record_id, "stt", file_path_str)

        if "diarize" in steps:
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            if task_id:
                update_task_progress(task_id, "화자 분리 시작", stage=TaskStage.TRANSFORM)

            try:
                diarize_source = file_path if file_type == "audio" else current_file
                results["diarize"] = run_diarize_step(Path(diarize_source))
            except Exception as e:
                return _workflow_error_result(task_id, e, "diarize")

            if task_id:
                status = results["diarize"].get("status")
                if status == "skipped":
                    update_task_progress(task_id, "화자 분리 스킵(비오디오 입력)", stage=TaskStage.TRANSFORM)
                else:
                    update_task_progress(task_id, "화자 분리 완료", stage=TaskStage.TRANSFORM)

        if "embedding" in steps and current_file:
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            if file_type == "audio" and current_file == file_path:
                existing_stt = find_existing_stt_file(file_path)

                if existing_stt:
                    if task_id:
                        update_task_progress(task_id, f"기존 STT 결과 발견: {existing_stt.name}", stage=TaskStage.TRANSFORM)
                    current_file = existing_stt
                    download_url = f"/download/{upload_folder_name}/{existing_stt.name}"
                    results["stt"] = download_url

                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)
                else:
                    if task_id:
                        update_task_progress(task_id, "STT 자동 실행 시작", stage=TaskStage.TRANSFORM)
                    try:

                        def progress_callback(message):
                            if task_id:
                                update_task_progress(task_id, message, stage=TaskStage.TRANSFORM)

                        whisper_model = "large-v3-turbo"
                        if model_settings and model_settings.get("whisper"):
                            whisper_model = model_settings["whisper"]

                        language = "ko"
                        if model_settings and model_settings.get("language") is not None:
                            lang = model_settings.get("language")
                            if lang in ("", "auto"):
                                language = None
                            else:
                                language = lang

                        device_choice = "auto"
                        if model_settings and model_settings.get("device"):
                            device_choice = model_settings.get("device")

                        transcribe_audio_files(
                            input_dir=str(current_file.parent),
                            output_dir=str(individual_output_dir),
                            model_identifier=whisper_model,
                            language=language,
                            initial_prompt="",
                            workers=1,
                            recursive=False,
                            filter_fillers=False,
                            min_seg_length=2,
                            normalize_punct=False,
                            requested_device=device_choice,
                            progress_callback=progress_callback,
                        )
                    except Exception as e:
                        print(f"STT process failed: {e}")
                        if task_id:
                            update_task_progress(task_id, f"STT 실패: {e}", stage=TaskStage.TRANSFORM)
                        return _workflow_error_result(task_id, e, "stt")

                    stt_file = individual_output_dir / f"{file_path.stem}.md"
                    download_url = f"/download/{upload_folder_name}/{stt_file.name}"
                    results["stt"] = download_url
                    current_file = stt_file

                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)

            if task_id:
                update_task_progress(task_id, "임베딩 생성 시작", stage=TaskStage.TRANSFORM)

            if generate_embedding(current_file, record_id):
                if task_id:
                    update_task_progress(task_id, "임베딩 생성 완료", stage=TaskStage.TRANSFORM)
            else:
                if task_id:
                    update_task_progress(task_id, "임베딩 생성 실패", stage=TaskStage.TRANSFORM)

        if "correct" in steps and current_file:
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            if task_id:
                update_task_progress(task_id, "교정 시작", stage=TaskStage.CORRECT)

            corrected_file = Path(current_file).with_name(f"{Path(current_file).stem}.corrected.md")
            try:
                ok = correct_text_file(
                    input_file=Path(current_file),
                    output_file=corrected_file,
                    model=(model_settings or {}).get("correct") or (model_settings or {}).get("summarize") or DEFAULT_MODEL,
                    provider_name=(model_settings or {}).get("provider") or (model_settings or {}).get("llm_provider"),
                )
                if not ok:
                    raise RuntimeError("교정 처리 결과가 실패로 반환되었습니다")
            except Exception as e:
                return _workflow_error_result(task_id, e, "correct")

            current_file = corrected_file
            results["correct"] = f"/download/{upload_folder_name}/{corrected_file.name}"


        if "summary" in steps:
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            if file_type == "audio" and current_file == file_path:
                existing_stt = find_existing_stt_file(file_path)

                if existing_stt:
                    if task_id:
                        update_task_progress(task_id, f"기존 STT 결과 발견: {existing_stt.name}", stage=TaskStage.TRANSFORM)
                    current_file = existing_stt
                    download_url = f"/download/{upload_folder_name}/{existing_stt.name}"
                    results["stt"] = download_url

                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)
                else:
                    if task_id:
                        update_task_progress(task_id, "STT 자동 실행 시작", stage=TaskStage.TRANSFORM)
                    try:

                        def progress_callback(message):
                            if task_id:
                                update_task_progress(task_id, message, stage=TaskStage.TRANSFORM)

                        whisper_model = "large-v3-turbo"
                        if model_settings and model_settings.get("whisper"):
                            whisper_model = model_settings["whisper"]

                        language = "ko"
                        if model_settings and model_settings.get("language") is not None:
                            lang = model_settings.get("language")
                            if lang in ("", "auto"):
                                language = None
                            else:
                                language = lang

                        device_choice = "auto"
                        if model_settings and model_settings.get("device"):
                            device_choice = model_settings.get("device")

                        transcribe_audio_files(
                            input_dir=str(current_file.parent),
                            output_dir=str(individual_output_dir),
                            model_identifier=whisper_model,
                            language=language,
                            initial_prompt="",
                            workers=1,
                            recursive=False,
                            filter_fillers=False,
                            min_seg_length=2,
                            normalize_punct=False,
                            requested_device=device_choice,
                            progress_callback=progress_callback,
                        )
                    except Exception as e:
                        print(f"STT process failed: {e}")
                        if task_id:
                            update_task_progress(task_id, f"STT 실패: {e}", stage=TaskStage.TRANSFORM)
                        return _workflow_error_result(task_id, e, "stt")

                    stt_file = individual_output_dir / f"{file_path.stem}.md"
                    download_url = f"/download/{upload_folder_name}/{stt_file.name}"
                    results["stt"] = download_url
                    current_file = stt_file

                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)

            source_text_path = Path(current_file) if current_file else None

            print(f"Starting summary for task {task_id}")
            if task_id:
                update_task_progress(task_id, "요약 생성 시작", stage=TaskStage.SUMMARY)

            summarize_model = DEFAULT_MODEL
            if model_settings and model_settings.get("summarize"):
                summarize_model = model_settings["summarize"]

            try:
                text = read_text_with_fallback(Path(current_file))
                if task_id:
                    update_task_progress(task_id, "텍스트 분석 중...", stage=TaskStage.SUMMARY)

                def summary_progress_callback(message):
                    if task_id:
                        update_task_progress(task_id, message, stage=TaskStage.SUMMARY)

                summary = summarize_text_mapreduce(
                    text=text,
                    model=summarize_model,
                    chunk_size=DEFAULT_CHUNK_SIZE,
                    max_tokens=None,
                    temperature=DEFAULT_TEMPERATURE,
                    progress_callback=summary_progress_callback,
                    provider_name=(model_settings or {}).get("provider") or (model_settings or {}).get("llm_provider"),
                )

                if task_id:
                    update_task_progress(task_id, "요약 파일 저장 중...", stage=TaskStage.SUMMARY)

                output_file = Path(current_file).with_name(f"{Path(current_file).stem}.summary.md")
                save_output(summary, output_file, as_json=False)

                # Obsidian MCP 자동 전송
                try:
                    file_uuid = record_id if record_id else output_file.parent.name

                    original_filename = None
                    if record_id:
                        history = load_upload_history()
                        for rec in history:
                            if rec.get("id") == record_id:
                                original_filename = rec.get("info", {}).get("original_filename")
                                break

                    if not original_filename:
                        original_filename = Path(current_file).name

                    created_at = datetime.now()

                    if task_id:
                        update_task_progress(task_id, "Obsidian 전송 중...", stage=TaskStage.SUMMARY)

                    mcp_result = send_summary_to_obsidian_sync(
                        uuid=file_uuid,
                        summary_text=summary,
                        original_filename=original_filename,
                        created_at=created_at,
                    )

                    if mcp_result["success"]:
                        print(f"Obsidian MCP 전송 성공: {mcp_result['message']}")
                        if task_id:
                            update_task_progress(task_id, "Obsidian 전송 완료", stage=TaskStage.SUMMARY)
                    else:
                        print(f"Obsidian MCP 전송 실패 (처리는 계속): {mcp_result['message']}")

                except Exception as e:
                    print(f"Obsidian MCP 전송 중 오류 (처리는 계속): {e}")

                if task_id:
                    update_task_progress(task_id, "요약 생성 완료", stage=TaskStage.SUMMARY)
            except Exception as e:
                print(f"Summary process failed: {e}")
                if task_id:
                    update_task_progress(task_id, f"요약 생성 실패: {e}", stage=TaskStage.SUMMARY)
                return _workflow_error_result(task_id, e, "summary")

            summary_file = current_file.with_name(f"{current_file.stem}.summary.md")
            download_url = f"/download/{upload_folder_name}/{summary_file.name}"
            results["summary"] = download_url
            current_file = summary_file

            if record_id:
                file_path_str = to_record_path(summary_file)
                update_task_completion(record_id, "summary", file_path_str)
                if source_text_path:
                    generate_and_store_title_summary(record_id, source_text_path, summarize_model)

    except Exception as exc:  # pragma: no cover
        if task_id:
            unregister_process(task_id)
            update_task_progress(task_id, f"작업 실패: {exc}", stage=TaskStage.TRANSFORM)
        return _workflow_error_result(task_id, exc, "workflow")

    finally:
        if task_id:
            clear_task_progress(task_id)

    return results
