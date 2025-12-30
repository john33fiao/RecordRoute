"""Embedding management for text vectorization and similarity search."""

import os
from pathlib import Path
import numpy as np

try:
    from .one_line_summary import generate_one_line_summary
    from .embedding_pipeline import embed_text_ollama, load_index, save_index
    from .file_utils import file_hash
    from .history_manager import update_task_completion, update_title_summary
    from .path_utils import to_record_path
except ImportError:
    from one_line_summary import generate_one_line_summary
    from embedding_pipeline import embed_text_ollama, load_index, save_index
    from file_utils import file_hash
    from history_manager import update_task_completion, update_title_summary
    from path_utils import to_record_path


def find_existing_stt_file(original_file_path: Path, output_dir: Path) -> Path | None:
    """Find existing STT result file for the given original file."""
    stem = original_file_path.stem

    # Extract UUID from the original file path (DB/uploads/UUID/filename)
    upload_uuid = original_file_path.parent.name
    print(f"[DEBUG] 업로드 UUID: {upload_uuid}")

    # Look for STT file in whisper_output/UUID/filename.md
    stt_output_dir = output_dir / upload_uuid
    potential_files = [
        stt_output_dir / f"{stem}.md",
        stt_output_dir / f"{stem}.corrected.md"
    ]

    for stt_file in potential_files:
        if stt_file.exists() and not stt_file.name.endswith('.summary.md'):
            print(f"[DEBUG] STT 파일 발견: {stt_file}")
            return stt_file

    print(f"[DEBUG] '{stem}.md' STT 파일을 찾지 못함 (경로: {stt_output_dir})")
    return None


def run_incremental_embedding(output_dir: Path, vector_dir: Path) -> int:
    """Run incremental embedding on all existing STT result files."""
    try:
        # Get embedding model name
        try:
            from sttEngine.config import get_model_for_task, get_default_model
            model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
        except:
            model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")

        # Load existing index
        index = load_index()
        processed_count = 0

        # Find all STT result files
        for md_file in output_dir.glob("**/*.md"):
            # Skip summary files
            if md_file.name.endswith('.summary.md'):
                continue

            # Check if already processed and up-to-date
            checksum = file_hash(md_file)
            key = str(md_file.resolve())
            if index.get(key, {}).get("sha256") == checksum:
                continue  # Already up-to-date

            try:
                # Read text content
                text = md_file.read_text(encoding="utf-8")

                # Generate embedding
                vector = embed_text_ollama(text, model_name)

                # Create vector directory if not exists
                vector_dir.mkdir(parents=True, exist_ok=True)

                # Save embedding vector with unique name
                vector_file = vector_dir / f"{md_file.parent.name}_{md_file.stem}.npy"
                np.save(vector_file, vector)

                # Update index
                index[key] = {
                    "sha256": checksum,
                    "vector": vector_file.name,
                    "deleted": False,
                    "deleted_path": None,
                    "vector_deleted_path": None,
                }

                processed_count += 1
                print(f"임베딩 생성 완료: {md_file.name}")

            except Exception as e:
                print(f"임베딩 생성 실패 {md_file.name}: {e}")
                continue

        # Save updated index
        save_index(index)
        print(f"증분 임베딩 완료: {processed_count}개 파일 처리됨")
        return processed_count

    except Exception as e:
        print(f"증분 임베딩 실행 실패: {e}")
        return 0


def generate_embedding(
    file_path: Path,
    vector_dir: Path,
    base_dir: Path,
    history_file: Path,
    registry_file: Path,
    record_id: str = None
) -> bool:
    """Generate embedding for a text file and store it."""
    try:
        # Get embedding model name
        try:
            from sttEngine.config import get_model_for_task, get_default_model
            model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
        except:
            model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")

        # Read text content
        text = file_path.read_text(encoding="utf-8")

        # Generate embedding
        vector = embed_text_ollama(text, model_name)

        # Create vector directory if not exists
        vector_dir.mkdir(parents=True, exist_ok=True)

        # Save embedding vector
        vector_file = vector_dir / f"{file_path.stem}.npy"
        np.save(vector_file, vector)

        # Update index
        index = load_index()
        checksum = file_hash(file_path)

        index[str(file_path.resolve())] = {
            "sha256": checksum,
            "vector": vector_file.name,
            "deleted": False,
            "deleted_path": None,
            "vector_deleted_path": None,
        }
        save_index(index)

        # Update task completion
        if record_id:
            file_path_str = to_record_path(file_path, base_dir)
            update_task_completion(record_id, "embedding", file_path_str, base_dir, history_file, registry_file)

        print(f"Embedding generated for {file_path.name}")
        return True

    except Exception as e:
        print(f"Embedding generation failed for {file_path.name}: {e}")
        return False


def generate_and_store_title_summary(
    record_id: str,
    file_path: Path,
    history_file: Path,
    model: str = None
) -> None:
    """Generate one-line summary and store it."""
    try:
        summary = generate_one_line_summary(file_path, model=model)
        update_title_summary(record_id, summary, history_file)
    except Exception as e:
        print(f"One-line summary generation failed: {e}")
