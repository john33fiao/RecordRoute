from __future__ import annotations

import os
from pathlib import Path
from typing import List, Dict, Any, Optional
import time

import numpy as np
import json
from datetime import datetime

from .embedding_pipeline import (
    INDEX_FILE,
    VECTOR_DIR,
    embed_text_ollama,
    load_index,
    resolve_index_path,
)
from .search_cache import cache_search_result, get_cached_search_result
from .similarity_matrix import invalidate_similarity_cache

from .config import get_default_model, get_model_for_task, normalize_db_record_path


def search(query: str, base_dir: Path, top_k: int = 10,
           start_date: Optional[str] = None,
           end_date: Optional[str] = None,
           sort_by: str = "similarity",
           sort_order: str = "desc",
           min_score: Optional[float] = None,
           page: int = 1,
           page_size: int = 5,
           include_timing: bool = False) -> List[Dict[str, Any]] | Dict[str, Any]:
    """Return top_k most similar documents for the given query.

    날짜/시간 필터링을 위해 ISO 형식의 ``start_date``와 ``end_date``를
    선택적으로 받을 수 있다.
    """
    # 캐시된 결과 확인
    overall_start = time.perf_counter()
    normalized_sort_by = (sort_by or "similarity").lower()
    normalized_sort_order = (sort_order or "desc").lower()
    page = max(1, int(page or 1))
    page_size = max(1, int(page_size or 5))

    timing = {
        "cache_lookup_ms": 0.0,
        "embedding_ms": 0.0,
        "index_load_ms": 0.0,
        "vector_scan_ms": 0.0,
        "postprocess_ms": 0.0,
        "total_ms": 0.0,
    }

    cache_lookup_start = time.perf_counter()
    cached_results = get_cached_search_result(
        query,
        top_k,
        start_date,
        end_date,
        sort_by=normalized_sort_by,
        sort_order=normalized_sort_order,
        min_score=min_score,
        page=page,
        page_size=page_size,
    )
    timing["cache_lookup_ms"] = (time.perf_counter() - cache_lookup_start) * 1000
    if cached_results is not None:
        print(f"캐시에서 검색 결과 반환: {len(cached_results)}개 항목")
        timing["total_ms"] = (time.perf_counter() - overall_start) * 1000
        if include_timing:
            return {"results": cached_results, "timing": timing, "cache_hit": True}
        return cached_results
    
    try:
        model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
    except:
        # 환경변수 설정이 없을 때 기본 모델 사용
        model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")
    
    try:
        embedding_start = time.perf_counter()
        query_vec = embed_text_ollama(query, model_name)
        timing["embedding_ms"] = (time.perf_counter() - embedding_start) * 1000

        index_load_start = time.perf_counter()
        index = load_index()
        timing["index_load_ms"] = (time.perf_counter() - index_load_start) * 1000
        results: List[Dict[str, Any]] = []

        start_dt = datetime.fromisoformat(start_date) if start_date else None
        end_dt = datetime.fromisoformat(end_date) if end_date else None

        vector_scan_start = time.perf_counter()
        for path_str, meta in index.items():
            if isinstance(meta, dict) and meta.get("deleted"):
                continue
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
            if min_score is not None and score < float(min_score):
                continue
            try:
                resolved_path = resolve_index_path(path_str, meta if isinstance(meta, dict) else None)
            except Exception:
                resolved_path = Path(path_str).resolve()
            try:
                rel_path = str(resolved_path.relative_to(base_dir))
            except ValueError:
                rel_path = resolved_path.as_posix()

            rel_path = normalize_db_record_path(rel_path, base_dir)
            results.append({"file": rel_path, "score": score, "uploaded_at": timestamp_str})
        
        timing["vector_scan_ms"] = (time.perf_counter() - vector_scan_start) * 1000

        postprocess_start = time.perf_counter()
        reverse = normalized_sort_order != "asc"
        if normalized_sort_by == "date":
            def _date_key(item: Dict[str, Any]) -> datetime:
                value = item.get("uploaded_at")
                if not value:
                    return datetime.min
                try:
                    return datetime.fromisoformat(value)
                except ValueError:
                    return datetime.min

            results.sort(key=_date_key, reverse=reverse)
        else:
            results.sort(key=lambda x: x["score"], reverse=reverse)

        offset = (page - 1) * page_size
        truncated_results = results[:top_k]
        final_results = truncated_results[offset: offset + page_size]

        timing["postprocess_ms"] = (time.perf_counter() - postprocess_start) * 1000

        # 결과를 캐시에 저장
        cache_search_result(
            query,
            top_k,
            final_results,
            start_date=start_date,
            end_date=end_date,
            sort_by=normalized_sort_by,
            sort_order=normalized_sort_order,
            min_score=min_score,
            page=page,
            page_size=page_size,
        )
        print(f"새로운 검색 결과를 캐시에 저장: {len(final_results)}개 항목")

        timing["total_ms"] = (time.perf_counter() - overall_start) * 1000
        if include_timing:
            return {"results": final_results, "timing": timing, "cache_hit": False, "total_candidates": len(results)}
        return final_results
    
    except Exception as e:
        print(f"검색 중 오류 발생: {e}")
        # 오류 발생 시 빈 결과 반환
        if include_timing:
            timing["total_ms"] = (time.perf_counter() - overall_start) * 1000
            return {"results": [], "timing": timing, "cache_hit": False, "total_candidates": 0}
        return []


def refresh_similarity_data() -> None:
    """Invalidate similarity graph caches after embedding/index updates."""
    invalidate_similarity_cache()

