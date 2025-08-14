"""Incremental embedding pipeline for summarized files.

Scans a directory for `*.summary.md` files that have completed the
STT→교정→요약 workflow and generates embeddings using the
`snowflake-arctic-embed:latest` model. Embeddings and minimal metadata are
stored in `vector_store/` with a JSON index so that only newly added or
modified files are processed on subsequent runs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
from pathlib import Path
from typing import Dict

import numpy as np
from sentence_transformers import SentenceTransformer

VECTOR_DIR = Path("vector_store")
INDEX_FILE = VECTOR_DIR / "index.json"


def load_index() -> Dict[str, Dict[str, str]]:
    """Load the JSON index mapping absolute file paths to metadata."""
    if INDEX_FILE.exists():
        with INDEX_FILE.open("r", encoding="utf-8") as f:
            return json.load(f)
    return {}


def save_index(index: Dict[str, Dict[str, str]]) -> None:
    """Persist the JSON index to disk."""
    VECTOR_DIR.mkdir(parents=True, exist_ok=True)
    with INDEX_FILE.open("w", encoding="utf-8") as f:
        json.dump(index, f, ensure_ascii=False, indent=2)


def file_hash(path: Path) -> str:
    """Return a stable SHA256 checksum for the given file."""
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def embed_text(model: SentenceTransformer, text: str) -> np.ndarray:
    """Embed text using the provided model and return a float32 vector."""
    vec = model.encode(text)
    return np.asarray(vec, dtype="float32")


def process_file(model: SentenceTransformer, path: Path, index: Dict[str, Dict[str, str]]) -> None:
    """Embed a single file if it is new or has changed since last run."""
    checksum = file_hash(path)
    key = str(path.resolve())
    if index.get(key, {}).get("sha256") == checksum:
        return  # already up-to-date

    text = path.read_text(encoding="utf-8")
    vector = embed_text(model, text)
    out_file = VECTOR_DIR / f"{path.stem}.npy"
    np.save(out_file, vector)

    index[key] = {"sha256": checksum, "vector": out_file.name}


def main(src_dir: str) -> None:
    """Scan for summary files under ``src_dir`` and embed newly added ones."""
    model = SentenceTransformer("snowflake-arctic-embed:latest")
    index = load_index()
    for file in Path(src_dir).glob("*.summary.md"):
        process_file(model, file, index)
    save_index(index)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Embed summary files for RAG usage")
    parser.add_argument("src", nargs="?", default=".", help="Directory to scan for *.summary.md files")
    args = parser.parse_args()
    main(args.src)
