from __future__ import annotations

import json
import uuid
from datetime import datetime, timedelta
from pathlib import Path
from typing import Dict, List, Any, Optional
import hashlib

# 캐시 디렉토리 설정
CACHE_DIR = Path(__file__).parent.parent / "vector_store" / "cache"
CACHE_DIR.mkdir(parents=True, exist_ok=True)

# 캐시 만료 시간 (24시간)
CACHE_EXPIRY_HOURS = 24


def get_query_hash(query: str, top_k: int) -> str:
    """검색 쿼리와 파라미터에 대한 해시값 생성"""
    query_data = f"{query}:{top_k}"
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
        if cache_time.tzinfo is None:
            cache_time = cache_time.replace(tzinfo=None)
        current_time = datetime.now()
        
        return (current_time - cache_time) > timedelta(hours=CACHE_EXPIRY_HOURS)
    except (ValueError, AttributeError):
        return True


def get_cached_search_result(query: str, top_k: int) -> Optional[List[Dict[str, Any]]]:
    """캐시된 검색 결과 조회"""
    query_hash = get_query_hash(query, top_k)
    record = load_cache_record(query_hash)
    
    if not record:
        return None
    
    if is_cache_expired(record.get('timestamp', '')):
        return None
    
    return record.get('results', [])


def cache_search_result(query: str, top_k: int, results: List[Dict[str, Any]], 
                       existing_uuid: Optional[str] = None) -> str:
    """검색 결과를 캐시에 저장"""
    query_hash = get_query_hash(query, top_k)
    
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
        "query": query,
        "top_k": top_k,
        "timestamp": datetime.now().isoformat(),
        "query_hash": query_hash,
        "results": results
    }
    
    save_cache_record(query_hash, record)
    return search_uuid


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