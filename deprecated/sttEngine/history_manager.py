"""Upload history management for tracking file uploads and task completion."""

import json
import uuid
from datetime import datetime
from pathlib import Path

try:
    from .path_utils import to_record_path
    from .registry_manager import register_file
except ImportError:
    from path_utils import to_record_path
    from registry_manager import register_file


TASK_TYPES = ("stt", "embedding", "summary")


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


def load_upload_history(history_file: Path) -> list[dict]:
    """Load upload history from JSON file and normalize record schema."""
    if history_file.exists():
        try:
            with open(history_file, 'r', encoding='utf-8') as f:
                history = json.load(f)

            if not isinstance(history, list):
                return []

            updated = False
            for record in history:
                if _ensure_record_schema(record):
                    updated = True

            if updated:
                save_upload_history(history, history_file)

            return history
        except (json.JSONDecodeError, IOError):
            return []
    return []


def get_active_history(history: list[dict] | None = None, history_file: Path = None) -> list[dict]:
    """Return history entries that are not marked as deleted."""
    if history is None:
        if history_file is None:
            raise ValueError("Either history or history_file must be provided")
        history = load_upload_history(history_file)
    return [record for record in history if not record.get("deleted")]


def save_upload_history(history: list[dict], history_file: Path) -> None:
    """Save upload history to JSON file."""
    try:
        with open(history_file, 'w', encoding='utf-8') as f:
            json.dump(history, f, ensure_ascii=False, indent=2)
    except IOError:
        pass


def add_upload_record(
    file_path: Path,
    file_type: str,
    base_dir: Path,
    history_file: Path,
    duration: str = None,
    file_hash: str = None
) -> dict:
    """Add a new upload record to history."""
    history = load_upload_history(history_file)

    record = {
        "id": str(uuid.uuid4()),
        "timestamp": datetime.now().isoformat(),
        "filename": file_path.name,
        "file_type": file_type,
        "duration": duration,
        "file_path": to_record_path(file_path, base_dir),
        "folder_name": file_path.parent.name,  # UUID folder name
        "completed_tasks": {task: False for task in TASK_TYPES},
        "download_links": {},
        "title_summary": "",
        "tags": [],
        "file_hash": file_hash,
        "deleted": False,
        "deleted_at": None,
        "deleted_assets": {}
    }

    _ensure_record_schema(record)

    history.insert(0, record)  # Add to beginning (most recent first)

    # Keep only last 100 records
    if len(history) > 100:
        history = history[:100]

    save_upload_history(history, history_file)
    return record


def update_task_completion(
    record_id: str,
    task: str,
    file_path: str,
    base_dir: Path,
    history_file: Path,
    registry_file: Path
) -> str:
    """Update task completion status and register file with UUID."""
    history = load_upload_history(history_file)

    # Register the file and get UUID
    file_uuid = register_file(file_path, record_id, task, base_dir, registry_file)
    download_url = f"/download/{file_uuid}"

    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return file_uuid
            record["completed_tasks"][task] = True
            record["download_links"][task] = download_url
            break

    save_upload_history(history, history_file)
    return file_uuid


def update_title_summary(record_id: str, summary: str, history_file: Path) -> None:
    """Store one-line summary for a record."""
    history = load_upload_history(history_file)
    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return
            record["title_summary"] = summary
            break
    save_upload_history(history, history_file)


def update_filename(record_id: str, new_filename: str, history_file: Path) -> None:
    """Update filename for a record."""
    history = load_upload_history(history_file)
    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return
            record["filename"] = new_filename
            break
    save_upload_history(history, history_file)
