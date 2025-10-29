from __future__ import annotations

import os
import sys
from pathlib import Path
from typing import List, Dict, Any, Optional

import numpy as np
import json
from datetime import datetime

from embedding_pipeline import VECTOR_DIR, INDEX_FILE, load_index, embed_text_ollama
from search_cache import get_cached_search_result, cache_search_result

# 설정 모듈 임포트
sys.path.append(str(Path(__file__).parent / "sttEngine"))
from config import get_default_model, get_model_for_task, normalize_db_record_path


def search(query: str, base_dir: Path, top_k: int = 10,
           start_date: Optional[str] = None,
           end_date: Optional[str] = None) -> List[Dict[str, Any]]:
    """Return top_k most similar documents for the given query.

    날짜/시간 필터링을 위해 ISO 형식의 ``start_date``와 ``end_date``를
    선택적으로 받을 수 있다.
    """
    # 캐시된 결과 확인
    cached_results = get_cached_search_result(query, top_k, start_date, end_date)
    if cached_results is not None:
        print(f"캐시에서 검색 결과 반환: {len(cached_results)}개 항목")
        return cached_results
    
    try:
        model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
    except:
        # 환경변수 설정이 없을 때 기본 모델 사용
        model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")
    
    try:
        query_vec = embed_text_ollama(query, model_name)
        index = load_index()
        results: List[Dict[str, Any]] = []

        start_dt = datetime.fromisoformat(start_date) if start_date else None
        end_dt = datetime.fromisoformat(end_date) if end_date else None

        for path_str, meta in index.items():
            timestamp_str = meta.get("timestamp")
            if start_dt or end_dt:
                if not timestamp_str:
                    continue
                try:
                    doc_time = datetime.fromisoformat(timestamp_str)
                except ValueError:
                    continue
                if start_dt and doc_time < start_dt:
                    continue
                if end_dt and doc_time > end_dt:
                    continue

            vec_file = VECTOR_DIR / meta.get("vector", "")
            if not vec_file.exists():
                continue
            doc_vec = np.load(vec_file)
            # cosine similarity
            denom = (np.linalg.norm(query_vec) * np.linalg.norm(doc_vec))
            if denom == 0:
                continue
            score = float(np.dot(query_vec, doc_vec) / denom)
            resolved_path = Path(path_str).resolve()
            try:
                rel_path = str(resolved_path.relative_to(base_dir))
            except ValueError:
                rel_path = resolved_path.as_posix()

            rel_path = normalize_db_record_path(rel_path, base_dir)
            results.append({"file": rel_path, "score": score})
        
        results.sort(key=lambda x: x["score"], reverse=True)
        final_results = results[:top_k]
        
        # 결과를 캐시에 저장
        cache_search_result(query, top_k, final_results,
                            start_date=start_date, end_date=end_date)
        print(f"새로운 검색 결과를 캐시에 저장: {len(final_results)}개 항목")
        
        return final_results
    
    except Exception as e:
        print(f"검색 중 오류 발생: {e}")
        # 오류 발생 시 빈 결과 반환
        return []
