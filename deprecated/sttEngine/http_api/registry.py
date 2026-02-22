from __future__ import annotations

import json
import os
import uuid
from datetime import datetime
from pathlib import Path

from ..one_line_summary import generate_one_line_summary
from .history import load_upload_history, save_upload_history
from .paths import FILE_REGISTRY_FILE, normalize_record_path, resolve_record_path


def load_file_registry() -> dict:
    """Load file registry from JSON file."""
    if FILE_REGISTRY_FILE.exists():
        try:
            with open(FILE_REGISTRY_FILE, "r", encoding="utf-8") as f:
                registry = json.load(f)

            if isinstance(registry, dict):
                updated = False
                for info in registry.values():
                    if not isinstance(info, dict):
                        continue
                    if not isinstance(info.get("deleted"), bool):
                        info["deleted"] = False
                        updated = True
                    if "deleted_at" not in info:
                        info["deleted_at"] = None
                        updated = True
                if updated:
                    save_file_registry(registry)
                return registry
        except (json.JSONDecodeError, IOError):
            return {}
    return {}


def save_file_registry(registry: dict) -> None:
    """Save file registry to JSON file."""
    try:
        with open(FILE_REGISTRY_FILE, "w", encoding="utf-8") as f:
            json.dump(registry, f, ensure_ascii=False, indent=2)
    except IOError:
        pass


def is_valid_uuid(value: str) -> bool:
    """Check whether a string is a valid UUID value."""
    if not value:
        return False
    try:
        uuid.UUID(str(value))
        return True
    except (ValueError, TypeError):
        return False


def resolve_file_identifier(file_identifier: str):
    """Resolve a file identifier (UUID or path) to an absolute path and metadata."""
    if not file_identifier:
        return None, None, None, None

    identifier = file_identifier.strip()
    if identifier.startswith("/download/"):
        identifier = identifier[len("/download/") :]

    identifier = identifier.lstrip("/").replace("\\", "/")

    registry = load_file_registry()

    if is_valid_uuid(identifier):
        file_info = registry.get(identifier)
        if not file_info:
            return None, None, None, identifier

        file_path = normalize_record_path(file_info.get("file_path", ""))
        if not file_path:
            return None, file_info.get("record_id"), file_info.get("task_type"), identifier

        full_path = resolve_record_path(file_path)
        record_id = file_info.get("record_id")
        task_type = file_info.get("task_type")
        return full_path, record_id, task_type, identifier

    # Legacy path-based identifier
    file_path = normalize_record_path(identifier)
    if not file_path:
        return None, None, None, identifier

    full_path = resolve_record_path(file_path)
    record_id = None
    task_type = None
    resolved_identifier = identifier

    for uuid_key, info in registry.items():
        stored_path = normalize_record_path(info.get("file_path", ""))
        if resolve_record_path(stored_path) == full_path:
            record_id = info.get("record_id")
            task_type = info.get("task_type")
            resolved_identifier = uuid_key
            break

    return full_path, record_id, task_type, resolved_identifier


def register_file(
    file_path: str,
    record_id: str,
    task_type: str,
    original_filename: str | None = None,
) -> str:
    """Register a file with UUID and return the file UUID."""
    registry = load_file_registry()
    file_uuid = str(uuid.uuid4())

    normalized_path = normalize_record_path(file_path)

    file_info = {
        "file_uuid": file_uuid,
        "file_path": normalized_path,
        "record_id": record_id,
        "task_type": task_type,
        "original_filename": original_filename or os.path.basename(normalized_path),
        "created_at": datetime.now().isoformat(),
        "deleted": False,
        "deleted_at": None,
    }

    registry[file_uuid] = file_info
    save_file_registry(registry)
    return file_uuid


def get_file_by_uuid(file_uuid: str) -> dict | None:
    """Get file info by UUID."""
    registry = load_file_registry()
    return registry.get(file_uuid)


def migrate_existing_files() -> None:
    """Migrate existing files from upload history to file registry."""
    history = load_upload_history()
    registry = load_file_registry()
    updated = False

    for record in history:
        if record.get("deleted"):
            continue
        record_id = record["id"]
        download_links = record.get("download_links", {})

        # Process each download link
        for task_type, download_url in download_links.items():
            if download_url.startswith("/download/"):
                file_path = normalize_record_path(download_url[10:])  # Remove "/download/" prefix

                # Check if this file is already registered
                already_registered = False
                for file_info in registry.values():
                    if (
                        normalize_record_path(file_info["file_path"]) == file_path
                        and file_info["record_id"] == record_id
                    ):
                        already_registered = True
                        break

                if not already_registered:
                    # Register the file and update download link
                    full_path = resolve_record_path(file_path)
                    if full_path.exists():
                        file_uuid = register_file(
                            file_path,
                            record_id,
                            task_type,
                            os.path.basename(full_path),
                        )
                        # Update the download link to use UUID
                        record["download_links"][task_type] = f"/download/{file_uuid}"
                        updated = True

    if updated:
        save_upload_history(history)
        print("기존 파일들이 레지스트리에 등록되었습니다.")


def update_task_completion(record_id: str, task: str, file_path: str) -> str:
    """Update task completion status and register file with UUID."""
    history = load_upload_history()

    # Register the file and get UUID
    file_uuid = register_file(file_path, record_id, task)
    download_url = f"/download/{file_uuid}"

    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return file_uuid
            record["completed_tasks"][task] = True
            record["download_links"][task] = download_url
            break

    save_upload_history(history)
    return file_uuid


def update_title_summary(record_id: str, summary: str) -> None:
    """Store one-line summary for a record."""
    history = load_upload_history()
    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return
            record["title_summary"] = summary
            break
    save_upload_history(history)


def update_filename(record_id: str, new_filename: str) -> None:
    """Update filename for a record."""
    history = load_upload_history()
    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return
            record["filename"] = new_filename
            break
    save_upload_history(history)


def generate_and_store_title_summary(record_id: str, file_path: Path, model: str | None = None) -> None:
    """Generate one-line summary and store it."""
    try:
        summary = generate_one_line_summary(file_path, model=model)
        update_title_summary(record_id, summary)
    except Exception as e:
        print(f"One-line summary generation failed: {e}")
