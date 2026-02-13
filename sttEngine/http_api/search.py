from __future__ import annotations

import hashlib
import json
import re
from datetime import datetime
from pathlib import Path
from typing import Any

from ..search_cache import CACHE_DIR
from ..workflow.summarize import read_text_with_fallback
from .history import get_active_history
from .paths import SEARCHABLE_SUFFIXES, normalize_record_path, resolve_record_path
from .registry import load_file_registry

_KEYWORD_CACHE_DIR = CACHE_DIR / "keyword_frequency"
_KEYWORD_CACHE_DIR.mkdir(parents=True, exist_ok=True)

_TEXT_CACHE: dict[str, dict[str, Any]] = {}
_FREQUENCY_CACHE: dict[str, int] = {}


def _timestamp_to_sort_key(timestamp_str: str) -> float:
    """Convert ISO timestamp string to numeric sort key."""
    if not timestamp_str:
        return float("-inf")
    try:
        return datetime.fromisoformat(timestamp_str).timestamp()
    except ValueError:
        return float("-inf")


def collect_searchable_documents():
    """Return documents eligible for keyword search and similarity mapping."""
    documents = []
    path_index = {}
    registry = load_file_registry()

    for file_uuid, info in registry.items():
        if isinstance(info, dict) and info.get("deleted"):
            continue
        rel_path = normalize_record_path(info.get("file_path"))
        if not rel_path:
            continue

        full_path = resolve_record_path(rel_path)
        if not full_path.exists():
            continue

        if full_path.suffix.lower() not in SEARCHABLE_SUFFIXES:
            continue

        doc = {
            "uuid": file_uuid,
            "info": info,
            "full_path": full_path,
            "relative_path": rel_path,
        }

        documents.append(doc)
        path_index.setdefault(rel_path, doc)

    return documents, path_index


def _get_file_version(path: Path) -> int:
    return path.stat().st_mtime_ns


def _get_cached_text(path: Path) -> str:
    cache_key = str(path)
    file_version = _get_file_version(path)
    cached = _TEXT_CACHE.get(cache_key)
    if cached and cached.get("version") == file_version:
        return cached["text"]

    text = read_text_with_fallback(path)
    _TEXT_CACHE[cache_key] = {"version": file_version, "text": text}
    return text


def _build_frequency_cache_key(path: Path, query: str, file_version: int) -> str:
    raw = json.dumps(
        {
            "path": str(path),
            "query": query,
            "file_version": file_version,
        },
        ensure_ascii=False,
        sort_keys=True,
    )
    return hashlib.md5(raw.encode("utf-8")).hexdigest()


def _load_disk_frequency_cache(cache_key: str) -> int | None:
    cache_file = _KEYWORD_CACHE_DIR / f"{cache_key}.json"
    if not cache_file.exists():
        return None
    try:
        with open(cache_file, "r", encoding="utf-8") as f:
            payload = json.load(f)
        return int(payload.get("count", 0))
    except (OSError, ValueError, TypeError, json.JSONDecodeError):
        return None


def _save_disk_frequency_cache(cache_key: str, count: int) -> None:
    cache_file = _KEYWORD_CACHE_DIR / f"{cache_key}.json"
    try:
        with open(cache_file, "w", encoding="utf-8") as f:
            json.dump({"count": count}, f, ensure_ascii=False)
    except OSError:
        return


def _count_keyword_occurrence(pattern: re.Pattern[str], path: Path, query: str) -> int:
    file_version = _get_file_version(path)
    frequency_key = _build_frequency_cache_key(path, query, file_version)

    if frequency_key in _FREQUENCY_CACHE:
        return _FREQUENCY_CACHE[frequency_key]

    cached_disk = _load_disk_frequency_cache(frequency_key)
    if cached_disk is not None:
        _FREQUENCY_CACHE[frequency_key] = cached_disk
        return cached_disk

    text = _get_cached_text(path)
    count = len(pattern.findall(text))
    _FREQUENCY_CACHE[frequency_key] = count
    _save_disk_frequency_cache(frequency_key, count)
    return count


def collect_keyword_matches(query: str, documents, history_map: dict, limit: int = 5):
    """Return top keyword matches sorted by frequency and recency."""
    if not query:
        return []

    pattern = re.compile(re.escape(query), re.IGNORECASE)
    matches = []

    for doc in documents:
        try:
            count = _count_keyword_occurrence(pattern, doc["full_path"], query)
        except Exception as exc:  # pragma: no cover - defensive logging
            print(f"키워드 검색을 위한 파일 읽기 실패 {doc['full_path']}: {exc}")
            continue

        if count <= 0:
            continue

        record = history_map.get(doc["info"].get("record_id"), {})
        timestamp = record.get("timestamp")

        matches.append(
            {
                "file_uuid": doc["uuid"],
                "file": doc["relative_path"],
                "display_name": doc["info"].get("original_filename") or Path(doc["relative_path"]).name,
                "count": count,
                "uploaded_at": timestamp,
                "source_filename": record.get("filename"),
                "link": f"/download/{doc['uuid']}",
            }
        )

    matches.sort(
        key=lambda item: (-item["count"], -_timestamp_to_sort_key(item.get("uploaded_at")))
    )
    return matches[:limit]


def build_history_map():
    history = get_active_history()
    return {record.get("id"): record for record in history}
