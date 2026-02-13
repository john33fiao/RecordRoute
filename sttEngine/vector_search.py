from __future__ import annotations

import os
import time
from datetime import datetime
from pathlib import Path
from typing import Any, Dict, List, Optional

import numpy as np

from .config import get_default_model, get_model_for_task, normalize_db_record_path
from .embedding_pipeline import INDEX_FILE, VECTOR_DIR, embed_text_ollama, load_index, resolve_index_path
from .search_cache import cache_search_result, get_cached_search_result
from .similarity_matrix import invalidate_similarity_cache


def _build_index_signature() -> str:
    try:
        stat = INDEX_FILE.stat()
        return f"{int(stat.st_mtime_ns)}:{int(stat.st_size)}"
    except OSError:
        return "missing"


def _paginate(results: List[Dict[str, Any]], page: int, page_size: int) -> List[Dict[str, Any]]:
    offset = (page - 1) * page_size
    return results[offset: offset + page_size]


def search(query: str, base_dir: Path, top_k: int = 10,
           start_date: Optional[str] = None,
           end_date: Optional[str] = None,
           sort_by: str = "similarity",
           sort_order: str = "desc",
           min_score: Optional[float] = None,
           page: int = 1,
           page_size: int = 5,
           include_timing: bool = False,
           filter_signature: Optional[str] = None) -> List[Dict[str, Any]] | Dict[str, Any]:
    overall_start = time.perf_counter()
    normalized_sort_by = (sort_by or "similarity").lower()
    normalized_sort_order = (sort_order or "desc").lower()
    page = max(1, int(page or 1))
    page_size = max(1, int(page_size or 5))
    index_signature = _build_index_signature()

    timing = {
        "cache_lookup_ms": 0.0,
        "embedding_ms": 0.0,
        "index_load_ms": 0.0,
        "vector_scan_ms": 0.0,
        "postprocess_ms": 0.0,
        "total_ms": 0.0,
    }

    cache_lookup_start = time.perf_counter()
    cached_ranked_results = get_cached_search_result(
        query,
        top_k,
        start_date,
        end_date,
        sort_by=normalized_sort_by,
        sort_order=normalized_sort_order,
        min_score=min_score,
        page=None,
        page_size=None,
        filter_signature=filter_signature,
        index_signature=index_signature,
    )
    timing["cache_lookup_ms"] = (time.perf_counter() - cache_lookup_start) * 1000
    if cached_ranked_results is not None:
        paged = _paginate(cached_ranked_results, page, page_size)
        timing["total_ms"] = (time.perf_counter() - overall_start) * 1000
        if include_timing:
            return {
                "results": paged,
                "timing": timing,
                "cache_hit": True,
                "total_candidates": len(cached_ranked_results),
            }
        return paged

    try:
        model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
    except Exception:
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

        ranked_results = results[:top_k]
        final_results = _paginate(ranked_results, page, page_size)
        timing["postprocess_ms"] = (time.perf_counter() - postprocess_start) * 1000

        cache_search_result(
            query,
            top_k,
            ranked_results,
            start_date=start_date,
            end_date=end_date,
            sort_by=normalized_sort_by,
            sort_order=normalized_sort_order,
            min_score=min_score,
            page=None,
            page_size=None,
            filter_signature=filter_signature,
            index_signature=index_signature,
        )

        timing["total_ms"] = (time.perf_counter() - overall_start) * 1000
        if include_timing:
            return {
                "results": final_results,
                "timing": timing,
                "cache_hit": False,
                "total_candidates": len(ranked_results),
            }
        return final_results

    except Exception as e:
        print(f"검색 중 오류 발생: {e}")
        if include_timing:
            timing["total_ms"] = (time.perf_counter() - overall_start) * 1000
            return {"results": [], "timing": timing, "cache_hit": False, "total_candidates": 0}
        return []


def refresh_similarity_data() -> None:
    """Invalidate similarity graph caches after embedding/index updates."""
    invalidate_similarity_cache()
