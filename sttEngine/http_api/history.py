from __future__ import annotations

import hashlib
import json
import uuid
from datetime import datetime
from pathlib import Path

from .paths import HISTORY_FILE, TASK_TYPES, to_record_path


def compute_file_hash(data: bytes) -> str:
    """Compute SHA256 hash for given file data."""
    return hashlib.sha256(data).hexdigest()


def _ensure_record_schema(record: dict) -> bool:
    """Ensure an upload history record has the expected structure."""
    updated = False

    completed = record.get("completed_tasks")
    if not isinstance(completed, dict):
        completed = {}
        record["completed_tasks"] = completed
        updated = True

    for task in TASK_TYPES:
        if task not in completed:
            completed[task] = False
            updated = True

    download_links = record.get("download_links")
    if not isinstance(download_links, dict):
        record["download_links"] = {}
        updated = True

    if not isinstance(record.get("deleted"), bool):
        record["deleted"] = False
        updated = True

    if "deleted_at" not in record:
        record["deleted_at"] = None
        updated = True

    if not isinstance(record.get("deleted_assets"), dict):
        record["deleted_assets"] = {}
        updated = True

    return updated


def load_upload_history() -> list[dict]:
    """Load upload history from JSON file and normalize record schema."""
    if HISTORY_FILE.exists():
        try:
            with open(HISTORY_FILE, "r", encoding="utf-8") as f:
                history = json.load(f)

            if not isinstance(history, list):
                return []

            updated = False
            for record in history:
                if _ensure_record_schema(record):
                    updated = True

            if updated:
                save_upload_history(history)

            return history
        except (json.JSONDecodeError, IOError):
            return []
    return []


def get_active_history(history: list[dict] | None = None) -> list[dict]:
    """Return history entries that are not marked as deleted."""
    if history is None:
        history = load_upload_history()
    return [record for record in history if not record.get("deleted")]


def save_upload_history(history: list[dict]) -> None:
    """Save upload history to JSON file."""
    try:
        with open(HISTORY_FILE, "w", encoding="utf-8") as f:
            json.dump(history, f, ensure_ascii=False, indent=2)
    except IOError:
        pass


def add_upload_record(
    file_path: Path,
    file_type: str,
    duration: str | None = None,
    file_hash: str | None = None,
) -> dict:
    """Add a new upload record to history."""
    history = load_upload_history()

    record = {
        "id": str(uuid.uuid4()),
        "timestamp": datetime.now().isoformat(),
        "filename": file_path.name,
        "file_type": file_type,
        "duration": duration,
        "file_path": to_record_path(file_path),
        "folder_name": file_path.parent.name,  # UUID folder name
        "completed_tasks": {task: False for task in TASK_TYPES},
        "download_links": {},
        "title_summary": "",
        "tags": [],
        "file_hash": file_hash,
        "deleted": False,
        "deleted_at": None,
        "deleted_assets": {},
    }

    _ensure_record_schema(record)

    history.insert(0, record)  # Add to beginning (most recent first)

    # Keep only last 100 records
    if len(history) > 100:
        history = history[:100]

    save_upload_history(history)
    return record
