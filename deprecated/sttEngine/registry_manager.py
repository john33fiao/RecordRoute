"""File registry management for UUID-based file tracking."""

import json
import os
import uuid
from datetime import datetime
from pathlib import Path

try:
    from .path_utils import normalize_record_path, resolve_record_path
    from .file_utils import is_valid_uuid
except ImportError:
    from path_utils import normalize_record_path, resolve_record_path
    from file_utils import is_valid_uuid


def load_file_registry(registry_file: Path) -> dict:
    """Load file registry from JSON file."""
    if registry_file.exists():
        try:
            with open(registry_file, 'r', encoding='utf-8') as f:
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
                    save_file_registry(registry, registry_file)
                return registry
        except (json.JSONDecodeError, IOError):
            return {}
    return {}


def save_file_registry(registry: dict, registry_file: Path) -> None:
    """Save file registry to JSON file."""
    try:
        with open(registry_file, 'w', encoding='utf-8') as f:
            json.dump(registry, f, ensure_ascii=False, indent=2)
    except IOError:
        pass


def register_file(
    file_path: str,
    record_id: str,
    task_type: str,
    base_dir: Path,
    registry_file: Path,
    original_filename: str = None
) -> str:
    """Register a file with UUID and return the file UUID."""
    registry = load_file_registry(registry_file)
    file_uuid = str(uuid.uuid4())

    normalized_path = normalize_record_path(file_path, base_dir)

    file_info = {
        "file_uuid": file_uuid,
        "file_path": normalized_path,
        "record_id": record_id,
        "task_type": task_type,
        "original_filename": original_filename or os.path.basename(normalized_path),
        "created_at": datetime.now().isoformat(),
        "deleted": False,
        "deleted_at": None
    }

    registry[file_uuid] = file_info
    save_file_registry(registry, registry_file)
    return file_uuid


def get_file_by_uuid(file_uuid: str, registry_file: Path) -> dict | None:
    """Get file info by UUID."""
    registry = load_file_registry(registry_file)
    return registry.get(file_uuid)


def resolve_file_identifier(
    file_identifier: str,
    base_dir: Path,
    registry_file: Path
) -> tuple[Path | None, str | None, str | None, str]:
    """Resolve a file identifier (UUID or path) to an absolute path and metadata."""
    if not file_identifier:
        return None, None, None, None

    identifier = file_identifier.strip()
    if identifier.startswith("/download/"):
        identifier = identifier[len("/download/"):]

    identifier = identifier.lstrip("/").replace('\\', '/')

    registry = load_file_registry(registry_file)

    if is_valid_uuid(identifier):
        file_info = registry.get(identifier)
        if not file_info:
            return None, None, None, identifier

        file_path = normalize_record_path(file_info.get("file_path", ""), base_dir)
        if not file_path:
            return None, file_info.get("record_id"), file_info.get("task_type"), identifier

        full_path = resolve_record_path(file_path, base_dir)
        record_id = file_info.get("record_id")
        task_type = file_info.get("task_type")
        return full_path, record_id, task_type, identifier

    # Legacy path-based identifier
    file_path = normalize_record_path(identifier, base_dir)
    if not file_path:
        return None, None, None, identifier

    full_path = resolve_record_path(file_path, base_dir)
    record_id = None
    task_type = None
    resolved_identifier = identifier

    for uuid_key, info in registry.items():
        stored_path = normalize_record_path(info.get("file_path", ""), base_dir)
        if resolve_record_path(stored_path, base_dir) == full_path:
            record_id = info.get("record_id")
            task_type = info.get("task_type")
            resolved_identifier = uuid_key
            break

    return full_path, record_id, task_type, resolved_identifier


def migrate_existing_files(
    base_dir: Path,
    registry_file: Path,
    history_file: Path,
    load_history_func,
    save_history_func
) -> None:
    """Migrate existing files from upload history to file registry."""
    history = load_history_func()
    registry = load_file_registry(registry_file)
    updated = False

    for record in history:
        if record.get("deleted"):
            continue
        record_id = record["id"]
        download_links = record.get("download_links", {})

        # Process each download link
        for task_type, download_url in download_links.items():
            if download_url.startswith("/download/"):
                file_path = normalize_record_path(download_url[10:], base_dir)  # Remove "/download/" prefix

                # Check if this file is already registered
                already_registered = False
                for file_info in registry.values():
                    if normalize_record_path(file_info["file_path"], base_dir) == file_path and file_info["record_id"] == record_id:
                        already_registered = True
                        break

                if not already_registered:
                    # Register the file and update download link
                    full_path = resolve_record_path(file_path, base_dir)
                    if full_path.exists():
                        file_uuid = register_file(file_path, record_id, task_type, base_dir, registry_file, os.path.basename(full_path))
                        # Update the download link to use UUID
                        record["download_links"][task_type] = f"/download/{file_uuid}"
                        updated = True

    if updated:
        save_history_func(history)
        print("기존 파일들이 레지스트리에 등록되었습니다.")
