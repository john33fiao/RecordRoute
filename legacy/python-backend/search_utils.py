"""Search utility functions for keyword search and document collection."""

import re
from datetime import datetime
from pathlib import Path

try:
    from .workflow.summarize import read_text_with_fallback
except ImportError:
    from workflow.summarize import read_text_with_fallback

try:
    from .path_utils import normalize_record_path, resolve_record_path
    from .registry_manager import load_file_registry
except ImportError:
    from path_utils import normalize_record_path, resolve_record_path
    from registry_manager import load_file_registry


SEARCHABLE_SUFFIXES = {".md", ".txt", ".text", ".markdown"}


def _timestamp_to_sort_key(timestamp_str: str) -> float:
    """Convert ISO timestamp string to numeric sort key."""
    if not timestamp_str:
        return float('-inf')
    try:
        return datetime.fromisoformat(timestamp_str).timestamp()
    except ValueError:
        return float('-inf')


def _collect_searchable_documents(base_dir: Path, registry_file: Path) -> tuple[list[dict], dict]:
    """Return documents eligible for keyword search and similarity mapping."""
    documents = []
    path_index = {}
    registry = load_file_registry(registry_file)

    for file_uuid, info in registry.items():
        if isinstance(info, dict) and info.get("deleted"):
            continue
        rel_path = normalize_record_path(info.get("file_path"), base_dir)
        if not rel_path:
            continue

        full_path = resolve_record_path(rel_path, base_dir)
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


def _collect_keyword_matches(query: str, documents: list[dict], history_map: dict, limit: int = 5) -> list[dict]:
    """Return top keyword matches sorted by frequency and recency."""
    if not query:
        return []

    pattern = re.compile(re.escape(query), re.IGNORECASE)
    matches = []

    for doc in documents:
        try:
            text = read_text_with_fallback(doc["full_path"])
        except Exception as exc:  # pragma: no cover - defensive logging
            print(f"키워드 검색을 위한 파일 읽기 실패 {doc['full_path']}: {exc}")
            continue

        count = len(pattern.findall(text))
        if count <= 0:
            continue

        record = history_map.get(doc["info"].get("record_id"), {})
        timestamp = record.get("timestamp")

        matches.append({
            "file_uuid": doc["uuid"],
            "file": doc["relative_path"],
            "display_name": doc["info"].get("original_filename") or Path(doc["relative_path"]).name,
            "count": count,
            "uploaded_at": timestamp,
            "source_filename": record.get("filename"),
            "link": f"/download/{doc['uuid']}",
        })

    matches.sort(key=lambda item: (-item["count"], -_timestamp_to_sort_key(item.get("uploaded_at"))))
    return matches[:limit]
