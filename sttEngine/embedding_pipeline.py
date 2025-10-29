"""Incremental embedding pipeline for summarized files.

Scans a directory for `*.summary.md` files that have completed the
STT→교정→요약 workflow and generates embeddings using the
`bge-m3:latest` model. Embeddings and minimal metadata are stored in
`vector_store/` with a JSON index so that only newly added or modified
files are processed on subsequent runs.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
from pathlib import Path
from typing import Dict
import os
from datetime import datetime

import numpy as np
import requests

# 설정 모듈 임포트
# 패키지 내부 실행 시에는 최상위 디렉토리가 ``sys.path``에 없어
# ``sttEngine`` 모듈을 찾지 못하는 문제가 있었다.
# 이를 방지하기 위해 현재 파일의 부모(프로젝트 루트)를 경로에 추가한다.
sys.path.append(str(Path(__file__).resolve().parent.parent))
from config import get_db_base_path, get_default_model, get_model_for_task
from ollama_utils import ensure_ollama_server

DB_BASE_PATH = get_db_base_path()
VECTOR_DIR = DB_BASE_PATH / "vector_store"
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


DEFAULT_MAX_CHARS = int(os.environ.get("EMBEDDING_MAX_PROMPT_CHARS", "7500"))


def _chunk_text(text: str, max_chars: int = DEFAULT_MAX_CHARS) -> list[str]:
    """Split text into reasonably sized chunks for the embedding API."""

    if max_chars <= 0 or len(text) <= max_chars:
        return [text]

    chunks: list[str] = []
    start = 0
    text_length = len(text)

    while start < text_length:
        end = min(start + max_chars, text_length)

        if end < text_length:
            # Try to split on a natural boundary to avoid mid-word breaks
            split_pos = text.rfind("\n\n", start, end)
            if split_pos == -1:
                split_pos = text.rfind("\n", start, end)
            if split_pos == -1:
                split_pos = text.rfind(" ", start, end)
            if split_pos == -1 or split_pos <= start:
                split_pos = end
        else:
            split_pos = end

        chunk = text[start:split_pos].strip()
        if chunk:
            chunks.append(chunk)

        start = split_pos
        while start < text_length and text[start] in ("\n", " "):
            start += 1

    return chunks or [text]


def _request_embedding(model_name: str, prompt: str) -> np.ndarray:
    response = requests.post(
        "http://localhost:11434/api/embeddings",
        json={
            "model": model_name,
            "prompt": prompt
        },
        timeout=30
    )

    try:
        response.raise_for_status()
    except requests.HTTPError as exc:
        detail = response.text.strip()
        message = f"Ollama 응답 오류 {response.status_code}: {detail or 'no details'}"
        raise requests.HTTPError(message) from exc

    result = response.json()
    embedding = result.get("embedding", [])
    if not embedding:
        raise ValueError("Empty embedding received from Ollama")
    return np.array(embedding, dtype=np.float32)


def embed_text_ollama(text: str, model_name: str) -> np.ndarray:
    """Ollama API를 사용하여 텍스트를 임베딩.

    Ollama가 긴 입력에서 500 오류를 반환하는 문제를 피하기 위해 입력을 여러 조각으로
    나누어 호출한 뒤 평균 임베딩을 사용한다.
    """

    try:
        server_ok, server_msg = ensure_ollama_server()
        if not server_ok:
            raise Exception(f"Ollama 서버를 사용할 수 없습니다: {server_msg}")

        text = text.strip()
        if not text:
            raise ValueError("임베딩할 텍스트가 비어 있습니다.")

        chunks = _chunk_text(text)
        vectors = []
        for chunk in chunks:
            vectors.append(_request_embedding(model_name, chunk))

        if len(vectors) == 1:
            return vectors[0]

        stacked = np.vstack(vectors)
        return np.mean(stacked, axis=0)
    except Exception as e:
        print(f"Ollama 임베딩 실패: {e}")
        raise


def process_file(model_name: str, path: Path, index: Dict[str, Dict[str, str]]) -> None:
    """Embed a single file if it is new or has changed since last run."""
    checksum = file_hash(path)
    key = str(path.resolve())
    if index.get(key, {}).get("sha256") == checksum:
        return  # already up-to-date

    text = path.read_text(encoding="utf-8")
    vector = embed_text_ollama(text, model_name)
    out_file = VECTOR_DIR / f"{path.stem}.npy"
    np.save(out_file, vector)

    index[key] = {
        "sha256": checksum,
        "vector": out_file.name,
        "timestamp": datetime.fromtimestamp(path.stat().st_mtime).isoformat()
    }


def main(src_dir: str) -> None:
    """Scan for summary files under ``src_dir`` and embed newly added ones."""
    try:
        model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
    except:
        # 환경변수 설정이 없을 때 기본 모델 사용
        model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")
    
    index = load_index()
    for file in Path(src_dir).glob("*.summary.md"):
        try:
            process_file(model_name, file, index)
        except Exception as e:
            print(f"파일 {file} 임베딩 실패: {e}")
            continue
    save_index(index)


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description="Embed summary files for RAG usage")
    parser.add_argument("src", nargs="?", default=".", help="Directory to scan for *.summary.md files")
    args = parser.parse_args()
    main(args.src)
