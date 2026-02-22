from __future__ import annotations

import json
import uuid
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Any, Optional
import hashlib
import time

try:  # pragma: no cover - import resolution for both package/script execution
    from .config import DB_ALIAS, get_db_base_path
except Exception:  # pragma: no cover - fallback when imported as a script
    try:
        from config import DB_ALIAS, get_db_base_path  # type: ignore
    except Exception:  # pragma: no cover - keep module usable without config
        DB_ALIAS = "DB"  # type: ignore
        get_db_base_path = None  # type: ignore


def _resolve_cache_directory() -> Path:
    """Return the cache directory derived from the configured DB base path."""

    if callable(globals().get("get_db_base_path")):
        try:
            db_root = Path(get_db_base_path())  # type: ignore[arg-type]
            cache_dir = (db_root / "cache").resolve()
            cache_dir.mkdir(parents=True, exist_ok=True)
            return cache_dir
        except Exception:
            pass

    project_root = Path(__file__).resolve().parent.parent
    fallback = (project_root / DB_ALIAS / "cache").resolve()
    fallback.mkdir(parents=True, exist_ok=True)
    return fallback


# 캐시 디렉토리 설정
CACHE_DIR = _resolve_cache_directory()

# 캐시 만료 시간 (24시간)
CACHE_EXPIRY_HOURS = 24
CACHE_CLEANUP_INTERVAL_SECONDS = 600
_LAST_CLEANUP_AT = 0.0


def _build_cache_key_payload(
    query: str,
    top_k: int,
    start_date: Optional[str] = None,
    end_date: Optional[str] = None,
    sort_by: Optional[str] = None,
    sort_order: Optional[str] = None,
    min_score: Optional[float] = None,
    page: Optional[int] = None,
    page_size: Optional[int] = None,
    filter_signature: Optional[str] = None,
    index_signature: Optional[str] = None,
) -> Dict[str, Any]:
    """검색 캐시 키를 구성하는 정규화된 페이로드를 반환한다."""
    return {
        "query": (query or "").strip(),
        "top_k": int(top_k),
        "filters": {
            "start_date": start_date or "",
            "end_date": end_date or "",
            "min_score": float(min_score) if min_score is not None else None,
        },
        "sort": {
            "by": (sort_by or "similarity").lower(),
            "order": (sort_order or "desc").lower(),
        },
        "pagination": {
            "page": max(1, int(page)) if page is not None else None,
            "page_size": max(1, int(page_size)) if page_size is not None else None,
        },
        "filter_signature": filter_signature or "",
        "index_signature": index_signature or "",
    }


def get_query_hash(query: str, top_k: int,
                   start_date: Optional[str] = None,
                   end_date: Optional[str] = None,
                   sort_by: Optional[str] = None,
                   sort_order: Optional[str] = None,
                   min_score: Optional[float] = None,
                   page: Optional[int] = None,
                   page_size: Optional[int] = None,
                   filter_signature: Optional[str] = None,
                   index_signature: Optional[str] = None) -> str:
    """검색 쿼리와 파라미터에 대한 해시값 생성"""
    payload = _build_cache_key_payload(
        query,
        top_k,
        start_date=start_date,
        end_date=end_date,
        sort_by=sort_by,
        sort_order=sort_order,
        min_score=min_score,
        page=page,
        page_size=page_size,
        filter_signature=filter_signature,
        index_signature=index_signature,
    )
    query_data = json.dumps(payload, ensure_ascii=False, sort_keys=True, separators=(",", ":"))
    return hashlib.md5(query_data.encode('utf-8')).hexdigest()


def load_cache_record(query_hash: str) -> Optional[Dict[str, Any]]:
    """특정 검색 쿼리에 대한 캐시 레코드 로드"""
    cache_file = CACHE_DIR / f"{query_hash}.json"
    if not cache_file.exists():
        return None
    
    try:
        with open(cache_file, 'r', encoding='utf-8') as f:
            record = json.load(f)
        return record
    except (json.JSONDecodeError, IOError):
        return None


def save_cache_record(query_hash: str, record: Dict[str, Any]) -> None:
    """캐시 레코드 저장"""
    cache_file = CACHE_DIR / f"{query_hash}.json"
    try:
        with open(cache_file, 'w', encoding='utf-8') as f:
            json.dump(record, f, ensure_ascii=False, indent=2)
    except IOError:
        pass


def is_cache_expired(timestamp_str: str) -> bool:
    """캐시가 만료되었는지 확인 (24시간 기준)"""
    try:
        cache_time = datetime.fromisoformat(timestamp_str.replace('Z', '+00:00'))
        if cache_time.tzinfo is not None:
            cache_time = cache_time.astimezone().replace(tzinfo=None)
        current_time = datetime.now()
        return (current_time - cache_time) > timedelta(hours=CACHE_EXPIRY_HOURS)
    except (ValueError, AttributeError):
        return True


def maybe_cleanup_expired_cache(force: bool = False) -> int:
    """Throttle cleanup calls to avoid directory scans on every request."""
    global _LAST_CLEANUP_AT
    now = time.time()
    if not force and now - _LAST_CLEANUP_AT < CACHE_CLEANUP_INTERVAL_SECONDS:
        return 0

    cleaned = cleanup_expired_cache()
    _LAST_CLEANUP_AT = now
    return cleaned


def get_cached_search_result(query: str, top_k: int,
                             start_date: Optional[str] = None,
                             end_date: Optional[str] = None,
                             sort_by: Optional[str] = None,
                             sort_order: Optional[str] = None,
                             min_score: Optional[float] = None,
                             page: Optional[int] = None,
                             page_size: Optional[int] = None,
                             filter_signature: Optional[str] = None,
                             index_signature: Optional[str] = None) -> Optional[List[Dict[str, Any]]]:
    """캐시된 검색 결과 조회"""
    # 캐시 사용 전 만료된 항목을 주기적으로 정리하여 디스크 사용량을 관리
    maybe_cleanup_expired_cache()

    query_hash = get_query_hash(
        query,
        top_k,
        start_date,
        end_date,
        sort_by=sort_by,
        sort_order=sort_order,
        min_score=min_score,
        page=page,
        page_size=page_size,
        filter_signature=filter_signature,
        index_signature=index_signature,
    )
    record = load_cache_record(query_hash)
    
    if not record:
        return None
    
    if is_cache_expired(record.get('timestamp', '')):
        try:
            (CACHE_DIR / f"{query_hash}.json").unlink()
        except OSError:
            pass
        return None
    
    return record.get('results', [])


def cache_search_result(query: str, top_k: int, results: List[Dict[str, Any]],
                       existing_uuid: Optional[str] = None,
                       start_date: Optional[str] = None,
                       end_date: Optional[str] = None,
                       sort_by: Optional[str] = None,
                       sort_order: Optional[str] = None,
                       min_score: Optional[float] = None,
                       page: Optional[int] = None,
                       page_size: Optional[int] = None,
                       filter_signature: Optional[str] = None,
                       index_signature: Optional[str] = None) -> str:
    """검색 결과를 캐시에 저장"""
    query_hash = get_query_hash(
        query,
        top_k,
        start_date,
        end_date,
        sort_by=sort_by,
        sort_order=sort_order,
        min_score=min_score,
        page=page,
        page_size=page_size,
        filter_signature=filter_signature,
        index_signature=index_signature,
    )
    
    # 기존 UUID 유지하거나 새로 생성
    if existing_uuid:
        search_uuid = existing_uuid
    else:
        # 기존 캐시 레코드에서 UUID 추출 시도
        existing_record = load_cache_record(query_hash)
        if existing_record and 'uuid' in existing_record:
            search_uuid = existing_record['uuid']
        else:
            search_uuid = str(uuid.uuid4())
    
    record = {
        "uuid": search_uuid,
        "query": (query or "").strip(),
        "top_k": top_k,
        "timestamp": datetime.now().isoformat(),
        "query_hash": query_hash,
        "results": results,
        "start_date": start_date,
        "end_date": end_date,
        "sort_by": sort_by,
        "sort_order": sort_order,
        "min_score": min_score,
        "page": page,
        "page_size": page_size,
        "filter_signature": filter_signature,
        "index_signature": index_signature,
    }
    
    save_cache_record(query_hash, record)
    return search_uuid


def delete_cache_record(query: str, top_k: int,
                        start_date: Optional[str] = None,
                        end_date: Optional[str] = None,
                        sort_by: Optional[str] = None,
                        sort_order: Optional[str] = None,
                        min_score: Optional[float] = None,
                        page: Optional[int] = None,
                        page_size: Optional[int] = None) -> bool:
    """Delete cached search result for a given query."""
    query_hash = get_query_hash(
        query,
        top_k,
        start_date,
        end_date,
        sort_by=sort_by,
        sort_order=sort_order,
        min_score=min_score,
        page=page,
        page_size=page_size,
    )
    cache_file = CACHE_DIR / f"{query_hash}.json"
    try:
        cache_file.unlink()
        return True
    except FileNotFoundError:
        return False


def cleanup_expired_cache() -> int:
    """만료된 캐시 파일들 정리"""
    cleaned = 0
    if not CACHE_DIR.exists():
        return cleaned
    
    for cache_file in CACHE_DIR.glob("*.json"):
        try:
            with open(cache_file, 'r', encoding='utf-8') as f:
                record = json.load(f)
            
            if is_cache_expired(record.get('timestamp', '')):
                cache_file.unlink()
                cleaned += 1
        except (json.JSONDecodeError, IOError, KeyError):
            # 잘못된 캐시 파일도 삭제
            try:
                cache_file.unlink()
                cleaned += 1
            except OSError:
                pass
    
    return cleaned


def get_cache_stats() -> Dict[str, Any]:
    """캐시 통계 정보 반환"""
    if not CACHE_DIR.exists():
        return {"total_entries": 0, "expired_entries": 0, "valid_entries": 0}
    
    total = 0
    expired = 0
    
    for cache_file in CACHE_DIR.glob("*.json"):
        try:
            with open(cache_file, 'r', encoding='utf-8') as f:
                record = json.load(f)
            
            total += 1
            if is_cache_expired(record.get('timestamp', '')):
                expired += 1
        except (json.JSONDecodeError, IOError, KeyError):
            total += 1
            expired += 1  # 잘못된 파일도 만료된 것으로 간주
    
    return {
        "total_entries": total,
        "expired_entries": expired,
        "valid_entries": total - expired
    }
