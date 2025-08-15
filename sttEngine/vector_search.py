from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import List, Dict, Any

import numpy as np
from sentence_transformers import SentenceTransformer

from embedding_pipeline import VECTOR_DIR, INDEX_FILE, load_index, embed_text

# 설정 모듈 임포트
sys.path.append(str(Path(__file__).parent / "sttEngine"))
from sttEngine.config import get_model_for_task, get_default_model


def search(query: str, base_dir: Path, top_k: int = 10) -> List[Dict[str, Any]]:
    """Return top_k most similar documents for the given query."""
    try:
        model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
    except:
        # 환경변수 설정이 없을 때 기존 로직 사용
        model_name = os.environ.get("EMBEDDING_MODEL", "snowflake-arctic-embed2:latest")
    
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
