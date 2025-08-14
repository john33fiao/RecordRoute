from __future__ import annotations

import os
from pathlib import Path
from typing import List, Dict, Any

import numpy as np
from sentence_transformers import SentenceTransformer

from embedding_pipeline import VECTOR_DIR, INDEX_FILE, load_index, embed_text


def search(query: str, base_dir: Path, top_k: int = 10) -> List[Dict[str, Any]]:
    """Return top_k most similar documents for the given query."""
    model_name = os.environ.get("EMBEDDING_MODEL", "snowflake-arctic-embed:latest")
    model = SentenceTransformer(model_name)
    query_vec = embed_text(model, query)
    index = load_index()
    results: List[Dict[str, Any]] = []
    for path_str, meta in index.items():
        vec_file = VECTOR_DIR / meta.get("vector", "")
        if not vec_file.exists():
            continue
        doc_vec = np.load(vec_file)
        # cosine similarity
        denom = (np.linalg.norm(query_vec) * np.linalg.norm(doc_vec))
        if denom == 0:
            continue
        score = float(np.dot(query_vec, doc_vec) / denom)
        try:
            rel_path = str(Path(path_str).resolve().relative_to(base_dir))
        except ValueError:
            rel_path = path_str
        results.append({"file": rel_path, "score": score})
    results.sort(key=lambda x: x["score"], reverse=True)
    return results[:top_k]
