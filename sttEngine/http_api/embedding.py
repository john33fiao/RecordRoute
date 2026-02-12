from __future__ import annotations

import os
from pathlib import Path

import numpy as np

from ..embedding_pipeline import embed_text_ollama, load_index, save_index
from .paths import OUTPUT_DIR, VECTOR_DIR, to_record_path
from .registry import update_task_completion


def file_hash(path: Path) -> str:
    """Return a stable SHA256 checksum for the given file."""
    import hashlib

    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def run_incremental_embedding(base_dir: Path | None = None) -> int:
    """Run incremental embedding on all existing STT result files."""
    if base_dir is None:
        base_dir = OUTPUT_DIR

    try:
        # Get embedding model name
        try:
            from sttEngine.config import get_model_for_task, get_default_model

            model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
        except Exception:
            model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")

        # Load existing index
        index = load_index()
        processed_count = 0

        # Find all STT result files
        for md_file in base_dir.glob("**/*.md"):
            # Skip summary files
            if md_file.name.endswith(".summary.md"):
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
                VECTOR_DIR.mkdir(parents=True, exist_ok=True)

                # Save embedding vector with unique name
                vector_file = VECTOR_DIR / f"{md_file.parent.name}_{md_file.stem}.npy"
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


def generate_embedding(file_path: Path, record_id: str | None = None) -> bool:
    """Generate embedding for a text file and store it."""
    try:
        # Get embedding model name
        try:
            from sttEngine.config import get_model_for_task, get_default_model

            model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
        except Exception:
            model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")

        # Read text content
        text = file_path.read_text(encoding="utf-8")

        # Generate embedding
        vector = embed_text_ollama(text, model_name)

        # Create vector directory if not exists
        VECTOR_DIR.mkdir(parents=True, exist_ok=True)

        # Save embedding vector
        vector_file = VECTOR_DIR / f"{file_path.stem}.npy"
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
            file_path_str = to_record_path(file_path)
            update_task_completion(record_id, "embedding", file_path_str)

        print(f"Embedding generated for {file_path.name}")
        return True

    except Exception as e:
        print(f"Embedding generation failed for {file_path.name}: {e}")
        return False
