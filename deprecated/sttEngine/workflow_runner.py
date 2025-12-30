"""Workflow runner for executing STT, embedding, and summary tasks."""

import shutil
from pathlib import Path

try:
    from .workflow.transcribe import transcribe_audio_files
    from .workflow.summarize import (
        summarize_text_mapreduce,
        read_text_with_fallback,
        save_output,
        DEFAULT_MODEL,
        DEFAULT_CHUNK_SIZE,
        DEFAULT_TEMPERATURE,
    )
    from .file_utils import get_file_type
    from .path_utils import to_record_path
    from .history_manager import update_task_completion
    from .embedding_manager import (
        find_existing_stt_file,
        generate_embedding,
        generate_and_store_title_summary,
    )
    from .task_manager import (
        is_task_cancelled,
        update_task_progress,
        unregister_process,
        clear_task_progress,
    )
except ImportError:
    from workflow.transcribe import transcribe_audio_files
    from workflow.summarize import (
        summarize_text_mapreduce,
        read_text_with_fallback,
        save_output,
        DEFAULT_MODEL,
        DEFAULT_CHUNK_SIZE,
        DEFAULT_TEMPERATURE,
    )
    from file_utils import get_file_type
    from path_utils import to_record_path
    from history_manager import update_task_completion
    from embedding_manager import (
        find_existing_stt_file,
        generate_embedding,
        generate_and_store_title_summary,
    )
    from task_manager import (
        is_task_cancelled,
        update_task_progress,
        unregister_process,
        clear_task_progress,
    )


def run_workflow(
    file_path: Path,
    steps: list[str],
    base_dir: Path,
    output_dir: Path,
    vector_dir: Path,
    history_file: Path,
    registry_file: Path,
    record_id: str = None,
    task_id: str = None,
    model_settings: dict = None
) -> dict:
    """Run the requested workflow steps sequentially.

    Args:
        file_path: Path to the uploaded audio or text file.
        steps: list of step names, e.g. ["stt", "correct", "summary"].
        base_dir: Base directory for the application.
        output_dir: Output directory for processed files.
        vector_dir: Vector storage directory.
        history_file: Path to history JSON file.
        registry_file: Path to registry JSON file.
        record_id: Upload record ID for updating history.
        task_id: Unique task ID for tracking and cancellation.
        model_settings: Model configuration settings.

    Returns:
        Dict mapping step name to download URL.
    """

    results = {}
    current_file = file_path
    file_type = get_file_type(file_path)

    # Create individual output directory based on upload folder structure
    upload_folder_name = current_file.parent.name  # Get UUID folder name
    individual_output_dir = output_dir / upload_folder_name
    individual_output_dir.mkdir(exist_ok=True)

    try:
        # For text files, skip STT step and copy to output directory
        if file_type == 'text':
            if "stt" in steps:
                # Check if task was cancelled
                if task_id and is_task_cancelled(task_id):
                    return {"error": "Task was cancelled"}

                # For text files, we already have the text content, so just copy it to output
                text_file = individual_output_dir / f"{file_path.stem}.md"
                # Copy the text file to output directory with .md extension
                shutil.copy2(file_path, text_file)
                download_url = f"/download/{upload_folder_name}/{text_file.name}"
                results["stt"] = download_url
                current_file = text_file

                # Update history
                if record_id:
                    file_path_str = to_record_path(text_file, base_dir)
                    update_task_completion(record_id, "stt", file_path_str, base_dir, history_file, registry_file)
            else:
                # If no STT step for text file, use the original file as starting point
                # Copy to output directory for consistency
                text_file = individual_output_dir / f"{file_path.stem}.md"
                shutil.copy2(file_path, text_file)
                current_file = text_file

        # For PDF files, extract text and treat as markdown
        elif file_type == 'pdf':
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            try:
                from pypdf import PdfReader
                reader = PdfReader(str(current_file))
                pdf_text = "\n".join(page.extract_text() or "" for page in reader.pages)
            except Exception as e:
                print(f"PDF text extraction failed: {e}")
                return {"error": f"PDF text extraction failed: {e}"}

            text_file = individual_output_dir / f"{file_path.stem}.md"
            text_file.write_text(pdf_text, encoding='utf-8')

            if "stt" in steps:
                download_url = f"/download/{upload_folder_name}/{text_file.name}"
                results["stt"] = download_url
                if record_id:
                    file_path_str = to_record_path(text_file, base_dir)
                    update_task_completion(record_id, "stt", file_path_str, base_dir, history_file, registry_file)

            current_file = text_file

        # For audio files, run STT step
        elif file_type == 'audio' and "stt" in steps:
            # Check if task was cancelled before starting STT
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            print(f"Starting STT for task {task_id}")

            # Create progress callback function
            def progress_callback(message):
                if task_id:
                    update_task_progress(task_id, message)

            # Get Whisper model from settings, default to large-v3-turbo
            whisper_model = "large-v3-turbo"
            if model_settings and model_settings.get("whisper"):
                whisper_model = model_settings["whisper"]

            # Get Whisper language from settings, default to Korean
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
                    progress_callback=progress_callback
                )
            except Exception as e:
                print(f"STT process failed: {e}")
                if task_id:
                    update_task_progress(task_id, f"STT 실패: {e}")
                return {"error": f"STT process failed: {e}"}

            stt_file = individual_output_dir / f"{file_path.stem}.md"
            download_url = f"/download/{upload_folder_name}/{stt_file.name}"
            results["stt"] = download_url
            current_file = stt_file

            # Update history
            if record_id:
                file_path_str = to_record_path(stt_file, base_dir)
                update_task_completion(record_id, "stt", file_path_str, base_dir, history_file, registry_file)

        if "embedding" in steps and current_file:
            # Check if task was cancelled
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            # For audio files, check if we have the text file (STT completed)
            if file_type == 'audio' and current_file == file_path:
                # current_file is still the original audio file, check for existing STT result first
                existing_stt = find_existing_stt_file(file_path, output_dir)

                if existing_stt:
                    # Use existing STT result
                    if task_id:
                        update_task_progress(task_id, f"기존 STT 결과 발견: {existing_stt.name}")
                    current_file = existing_stt

                    # Update results to include existing STT
                    download_url = f"/download/{upload_folder_name}/{existing_stt.name}"
                    results["stt"] = download_url

                    # Update history if needed
                    if record_id:
                        file_path_str = to_record_path(current_file, base_dir)
                        update_task_completion(record_id, "stt", file_path_str, base_dir, history_file, registry_file)
                else:
                    # No existing STT result, run STT first
                    if task_id:
                        update_task_progress(task_id, "STT 자동 실행 시작")
                    try:
                        def progress_callback(message):
                            if task_id:
                                update_task_progress(task_id, message)

                        # Get Whisper model from settings, default to large-v3-turbo
                        whisper_model = "large-v3-turbo"
                        if model_settings and model_settings.get("whisper"):
                            whisper_model = model_settings["whisper"]

                        # Get Whisper language from settings, default to Korean
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
                            progress_callback=progress_callback
                        )
                    except Exception as e:
                        print(f"STT process failed: {e}")
                        if task_id:
                            update_task_progress(task_id, f"STT 실패: {e}")
                        return {"error": f"STT process failed: {e}"}

                    stt_file = individual_output_dir / f"{file_path.stem}.md"
                    download_url = f"/download/{upload_folder_name}/{stt_file.name}"
                    results["stt"] = download_url
                    current_file = stt_file

                    # Update history
                    if record_id:
                        file_path_str = to_record_path(current_file, base_dir)
                        update_task_completion(record_id, "stt", file_path_str, base_dir, history_file, registry_file)

            if task_id:
                update_task_progress(task_id, "임베딩 생성 시작")

            if generate_embedding(current_file, vector_dir, base_dir, history_file, registry_file, record_id):
                if task_id:
                    update_task_progress(task_id, "임베딩 생성 완료")
            else:
                if task_id:
                    update_task_progress(task_id, "임베딩 생성 실패")

        if "summary" in steps:
            # Check if task was cancelled before starting summary
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            # For audio files, check if we have the text file (STT completed)
            if file_type == 'audio' and current_file == file_path:
                # current_file is still the original audio file, check for existing STT result first
                existing_stt = find_existing_stt_file(file_path, output_dir)

                if existing_stt:
                    # Use existing STT result
                    if task_id:
                        update_task_progress(task_id, f"기존 STT 결과 발견: {existing_stt.name}")
                    current_file = existing_stt

                    # Update results to include existing STT
                    download_url = f"/download/{upload_folder_name}/{existing_stt.name}"
                    results["stt"] = download_url

                    # Update history if needed
                    if record_id:
                        file_path_str = to_record_path(current_file, base_dir)
                        update_task_completion(record_id, "stt", file_path_str, base_dir, history_file, registry_file)
                else:
                    # No existing STT result, run STT first
                    if task_id:
                        update_task_progress(task_id, "STT 자동 실행 시작")
                    try:
                        def progress_callback(message):
                            if task_id:
                                update_task_progress(task_id, message)

                        # Get Whisper model from settings, default to large-v3-turbo
                        whisper_model = "large-v3-turbo"
                        if model_settings and model_settings.get("whisper"):
                            whisper_model = model_settings["whisper"]

                        # Get Whisper language from settings, default to Korean
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
                            progress_callback=progress_callback
                        )
                    except Exception as e:
                        print(f"STT process failed: {e}")
                        if task_id:
                            update_task_progress(task_id, f"STT 실패: {e}")
                        return {"error": f"STT process failed: {e}"}

                    stt_file = individual_output_dir / f"{file_path.stem}.md"
                    download_url = f"/download/{upload_folder_name}/{stt_file.name}"
                    results["stt"] = download_url
                    current_file = stt_file

                    # Update history
                    if record_id:
                        file_path_str = to_record_path(current_file, base_dir)
                        update_task_completion(record_id, "stt", file_path_str, base_dir, history_file, registry_file)

            source_text_path = Path(current_file) if current_file else None

            print(f"Starting summary for task {task_id}")
            if task_id:
                update_task_progress(task_id, "요약 생성 시작")

            # Get summarize model from settings, default to DEFAULT_MODEL
            summarize_model = DEFAULT_MODEL
            if model_settings and model_settings.get("summarize"):
                summarize_model = model_settings["summarize"]

            try:
                text = read_text_with_fallback(Path(current_file))
                if task_id:
                    update_task_progress(task_id, "텍스트 분석 중...")

                # Create progress callback function for summary
                def summary_progress_callback(message):
                    if task_id:
                        update_task_progress(task_id, message)

                summary = summarize_text_mapreduce(
                    text=text,
                    model=summarize_model,
                    chunk_size=DEFAULT_CHUNK_SIZE,
                    max_tokens=None,
                    temperature=DEFAULT_TEMPERATURE,
                    progress_callback=summary_progress_callback
                )

                if task_id:
                    update_task_progress(task_id, "요약 파일 저장 중...")

                output_file = Path(current_file).with_name(f"{Path(current_file).stem}.summary.md")
                save_output(summary, output_file, as_json=False)

                if task_id:
                    update_task_progress(task_id, "요약 생성 완료")
            except Exception as e:
                print(f"Summary process failed: {e}")
                if task_id:
                    update_task_progress(task_id, f"요약 생성 실패: {e}")
                return {"error": f"Summary process failed: {e}"}

            summary_file = current_file.with_name(f"{current_file.stem}.summary.md")
            download_url = f"/download/{upload_folder_name}/{summary_file.name}"
            results["summary"] = download_url
            current_file = summary_file

            # Update history
            if record_id:
                file_path_str = to_record_path(summary_file, base_dir)
                update_task_completion(record_id, "summary", file_path_str, base_dir, history_file, registry_file)
                if source_text_path:
                    generate_and_store_title_summary(record_id, source_text_path, history_file, summarize_model)

    except Exception as exc:  # pragma: no cover - best effort error handling
        # Clean up process registration if something goes wrong
        if task_id:
            unregister_process(task_id)
            update_task_progress(task_id, f"작업 실패: {exc}")
        return {"error": str(exc)}

    finally:
        # Clear progress when task completes
        if task_id:
            clear_task_progress(task_id)

    return results
