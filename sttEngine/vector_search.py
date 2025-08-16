from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import List, Dict, Any

import numpy as np
import requests
import json

from embedding_pipeline import VECTOR_DIR, INDEX_FILE, load_index

# 설정 모듈 임포트
sys.path.append(str(Path(__file__).parent / "sttEngine"))
from sttEngine.config import get_model_for_task, get_default_model


def embed_text_ollama(text: str, model_name: str) -> np.ndarray:
    """Ollama API를 사용하여 텍스트를 임베딩"""
    try:
        response = requests.post(
            "http://localhost:11434/api/embeddings",
            json={
                "model": model_name,
                "prompt": text
            },
            timeout=30
        )
        response.raise_for_status()
        result = response.json()
        embedding = result.get("embedding", [])
        if not embedding:
            raise ValueError("Empty embedding received from Ollama")
        return np.array(embedding, dtype=np.float32)
    except Exception as e:
        print(f"Ollama 임베딩 실패: {e}")
        raise


def search(query: str, base_dir: Path, top_k: int = 10) -> List[Dict[str, Any]]:
    """Return top_k most similar documents for the given query."""
    try:
        model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
    except:
        # 환경변수 설정이 없을 때 기본 모델 사용
        model_name = os.environ.get("EMBEDDING_MODEL", "nomic-embed-text")
    
    try:
        query_vec = embed_text_ollama(query, model_name)
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
    
    except Exception as e:
        print(f"검색 중 오류 발생: {e}")
        # 오류 발생 시 빈 결과 반환
        return []
