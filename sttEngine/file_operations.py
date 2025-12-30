"""File operations for deletion, reset, and management of upload records."""

import shutil
from datetime import datetime
from pathlib import Path
from typing import Any

try:
    from .embedding_pipeline import load_index, save_index
    from .file_utils import is_valid_uuid
    from .history_manager import load_upload_history, save_upload_history, TASK_TYPES
    from .path_utils import normalize_record_path, resolve_record_path, to_record_path
    from .registry_manager import (
        get_file_by_uuid,
        load_file_registry,
        resolve_file_identifier,
        save_file_registry,
    )
except ImportError:
    from embedding_pipeline import load_index, save_index
    from file_utils import is_valid_uuid
    from history_manager import load_upload_history, save_upload_history, TASK_TYPES
    from path_utils import normalize_record_path, resolve_record_path, to_record_path
    from registry_manager import (
        get_file_by_uuid,
        load_file_registry,
        resolve_file_identifier,
        save_file_registry,
    )


def reset_upload_record(
    record_id: str,
    base_dir: Path,
    history_file: Path,
    output_dir: Path,
    vector_dir: Path
) -> bool:
    """Remove processed files and reset completion status for a record."""
    history = load_upload_history(history_file)

    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return False
            folder = record.get("folder_name")
            record_output_dir = output_dir / folder if folder else None
            try:
                if record_output_dir and record_output_dir.exists():
                    shutil.rmtree(record_output_dir)
            except Exception:
                pass

            # Remove embedding vectors and index entries related to this record
            if record_output_dir:
                index = load_index()
                keys_to_remove = []
                for key, meta in index.items():
                    try:
                        Path(key).resolve().relative_to(record_output_dir.resolve())
                        keys_to_remove.append((key, meta))
                    except ValueError:
                        continue

                for key, meta in keys_to_remove:
                    vector_name = meta.get("vector")
                    if vector_name:
                        vector_path = vector_dir / vector_name
                        if vector_path.exists():
                            # Check if this vector is referenced elsewhere
                            if not any(
                                v.get("vector") == vector_name and k != key
                                for k, v in index.items()
                            ):
                                try:
                                    vector_path.unlink()
                                except Exception:
                                    pass
                    del index[key]

                if keys_to_remove:
                    save_index(index)

            record["completed_tasks"] = {
                task: False for task in TASK_TYPES
            }
            record["download_links"] = {}
            record["title_summary"] = ""

            save_upload_history(history, history_file)
            return True

    return False


def delete_file(
    file_identifier: str,
    file_type: str,
    base_dir: Path,
    history_file: Path,
    registry_file: Path
) -> tuple[bool, str]:
    """Delete a specific file (STT or summary) and update history.

    Args:
        file_identifier: File UUID from download URL
        file_type: Type of file ('stt' or 'summary')

    Returns:
        Tuple of (success, error_message)
    """
    try:
        # Get file info by UUID
        file_info = get_file_by_uuid(file_identifier, registry_file)
        if not file_info:
            return False, "파일을 찾을 수 없습니다."

        # Get the actual file path
        file_path = Path(base_dir) / file_info["file_path"]

        if not file_path.exists():
            return False, "파일이 존재하지 않습니다."

        # Verify file type matches
        if file_type == 'stt' and not file_path.name.endswith('.md'):
            return False, "STT 파일이 아닙니다."
        elif file_type == 'summary' and not file_path.name.endswith('.summary.md'):
            return False, "요약 파일이 아닙니다."

        # Delete the file
        try:
            file_path.unlink()
        except Exception as e:
            print(f"Failed to delete {file_path}: {e}")
            return False, "파일 삭제에 실패했습니다."

        # Update history record
        history = load_upload_history(history_file)
        record_id = file_info["record_id"]

        for record in history:
            if record["id"] == record_id:
                if record.get("deleted"):
                    return False, "삭제된 항목입니다."
                # Update completion status
                record["completed_tasks"][file_type] = False

                # Remove download link
                if file_type in record["download_links"]:
                    del record["download_links"][file_type]

                # If deleting summary, also clear title_summary
                if file_type == 'summary':
                    record["title_summary"] = ""

                break

        # Remove from file registry
        registry = load_file_registry(registry_file)
        if file_identifier in registry:
            del registry[file_identifier]
            save_file_registry(registry, registry_file)

        # Save updated history
        save_upload_history(history, history_file)

        return True, ""

    except Exception as e:
        print(f"Error deleting file: {e}")
        return False, f"삭제 중 오류가 발생했습니다: {str(e)}"


def _delete_single_record_assets(
    record: dict,
    registry: dict,
    index: dict,
    moved_vector_names: set[str],
    base_dir: Path,
    upload_dir: Path,
    output_dir: Path,
    vector_dir: Path,
    deleted_upload_dir: Path,
    deleted_output_dir: Path,
    deleted_vector_dir: Path,
    db_alias: str
) -> dict:
    """Move record assets to the deleted area and update metadata."""

    record_id = record.get("id")
    folder_name = record.get("folder_name")
    deleted_at = datetime.now().isoformat()

    record_upload_dir = (upload_dir / folder_name).resolve() if folder_name else None
    record_deleted_upload_dir = (deleted_upload_dir / folder_name).resolve() if folder_name else None
    record_output_dir = (output_dir / folder_name).resolve() if folder_name else None
    record_deleted_output_dir = (deleted_output_dir / folder_name).resolve() if folder_name else None
    vec_dir = vector_dir.resolve()
    del_vec_dir = deleted_vector_dir.resolve()

    registry_changed = False
    index_changed = False

    files_assets: dict[str, list[str]] = {}
    record_assets: dict[str, Any] = {}

    registry_updates: list[tuple[str, dict, Path]] = []
    for file_uuid, info in (registry or {}).items():
        if not isinstance(info, dict):
            continue
        if info.get("record_id") != record_id:
            continue

        file_path_str = info.get("file_path")
        if not file_path_str:
            continue

        try:
            absolute_path = resolve_record_path(file_path_str, base_dir)
        except Exception:
            absolute_path = None

        new_path: Path | None = None
        if absolute_path is not None:
            if record_upload_dir:
                try:
                    rel = absolute_path.relative_to(record_upload_dir)
                    new_path = record_deleted_upload_dir / rel
                except ValueError:
                    pass
            if new_path is None and record_output_dir:
                try:
                    rel = absolute_path.relative_to(record_output_dir)
                    new_path = record_deleted_output_dir / rel
                except ValueError:
                    pass
            if new_path is None:
                try:
                    rel = absolute_path.relative_to(vec_dir)
                    new_path = del_vec_dir / rel
                except ValueError:
                    pass

        if new_path is None:
            continue

        registry_updates.append((file_uuid, info, new_path))

    index_entries: list[tuple[str, dict, Path]] = []
    vector_names: set[str] = set()
    if record_output_dir:
        for key, meta in (index or {}).items():
            if not isinstance(meta, dict):
                continue
            try:
                rel = Path(key).resolve().relative_to(record_output_dir)
            except (ValueError, FileNotFoundError):
                continue

            deleted_path = (record_deleted_output_dir / rel) if record_deleted_output_dir else None
            if deleted_path is None:
                continue

            index_entries.append((key, meta, deleted_path))

            vector_name = meta.get("vector")
            if vector_name:
                vector_names.add(vector_name)

    if record_upload_dir and record_upload_dir.exists():
        record_deleted_upload_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(record_upload_dir), str(record_deleted_upload_dir))
        record_assets["uploads"] = to_record_path(record_deleted_upload_dir, base_dir)

    if record_output_dir and record_output_dir.exists():
        record_deleted_output_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(record_output_dir), str(record_deleted_output_dir))
        record_assets["outputs"] = to_record_path(record_deleted_output_dir, base_dir)

    for file_uuid, info, new_path in registry_updates:
        info["file_path"] = to_record_path(new_path, base_dir)
        info["deleted"] = True
        info["deleted_at"] = deleted_at
        task_type = info.get("task_type")
        if task_type:
            files_assets.setdefault(task_type, []).append(info["file_path"])
        registry_changed = True

    if files_assets:
        record_assets["files"] = files_assets

    for key, meta, deleted_path in index_entries:
        meta["deleted"] = True
        meta["deleted_at"] = deleted_at
        meta["deleted_path"] = str(deleted_path)
        vector_name = meta.get("vector")
        if vector_name:
            deleted_vector_path = deleted_vector_dir / vector_name
            meta["vector_deleted_path"] = str(deleted_vector_path)
        index_changed = True

    for vector_name in vector_names:
        target_path = deleted_vector_dir / vector_name
        if vector_name not in moved_vector_names:
            source_path = vector_dir / vector_name
            target_path.parent.mkdir(parents=True, exist_ok=True)
            if source_path.exists():
                shutil.move(str(source_path), str(target_path))
            moved_vector_names.add(vector_name)
        moved_vector_paths = to_record_path(target_path, base_dir)

    if vector_names:
        record_assets["vectors"] = [to_record_path(deleted_vector_dir / vn, base_dir) for vn in vector_names]

    record["deleted"] = True
    record["deleted_at"] = deleted_at
    record["deleted_assets"] = record_assets

    return {
        "registry_changed": registry_changed,
        "index_changed": index_changed,
    }


def delete_records(
    record_ids: list[str],
    base_dir: Path,
    history_file: Path,
    registry_file: Path,
    upload_dir: Path,
    output_dir: Path,
    vector_dir: Path,
    deleted_upload_dir: Path,
    deleted_output_dir: Path,
    deleted_vector_dir: Path,
    db_alias: str
) -> tuple[bool, dict[str, dict]]:
    """Delete multiple upload records by moving their assets to a deleted folder."""

    if not record_ids:
        return False, {}

    history = load_upload_history(history_file)
    registry = load_file_registry(registry_file)
    index = load_index()

    history_by_id = {record.get("id"): record for record in history}
    results: dict[str, dict] = {}

    history_changed = False
    registry_changed = False
    index_changed = False
    moved_vector_names: set[str] = set()

    for record_id in record_ids:
        record = history_by_id.get(record_id)
        if not record:
            results[record_id] = {
                "success": False,
                "error": "기록을 찾을 수 없습니다.",
            }
            continue

        if record.get("deleted"):
            results[record_id] = {
                "success": False,
                "error": "이미 삭제된 항목입니다.",
            }
            continue

        try:
            summary = _delete_single_record_assets(
                record, registry, index, moved_vector_names,
                base_dir, upload_dir, output_dir, vector_dir,
                deleted_upload_dir, deleted_output_dir, deleted_vector_dir,
                db_alias
            )
            history_changed = True
            registry_changed = registry_changed or summary.get("registry_changed", False)
            index_changed = index_changed or summary.get("index_changed", False)
            results[record_id] = {"success": True}
        except Exception as exc:
            results[record_id] = {
                "success": False,
                "error": str(exc),
            }

    if history_changed:
        save_upload_history(history, history_file)
    if registry_changed:
        save_file_registry(registry, registry_file)
    if index_changed:
        save_index(index)

    overall_success = (
        bool(results)
        and all(result.get("success") for result in results.values())
    )
    return overall_success, results


def update_stt_text(
    file_identifier: str,
    new_text: str,
    base_dir: Path,
    history_file: Path,
    registry_file: Path,
    output_dir: Path
) -> tuple[bool, str, str | None]:
    """Update the contents of an STT result file."""

    if not file_identifier:
        return False, "파일 식별자가 필요합니다.", None

    file_path, record_id, task_type, _ = resolve_file_identifier(file_identifier, base_dir, registry_file)

    if not file_path or not file_path.exists():
        return False, "파일을 찾을 수 없습니다.", record_id

    if file_path.name.endswith('.summary.md'):
        return False, "요약 파일은 수정할 수 없습니다.", record_id

    if task_type and task_type not in ("stt", "embedding"):
        return False, "STT 파일만 수정할 수 있습니다.", record_id

    if file_path.suffix.lower() not in {'.md', '.txt', '.text', '.markdown'}:
        return False, "지원하지 않는 파일 형식입니다.", record_id

    try:
        file_path.write_text(new_text, encoding='utf-8')
    except Exception as exc:
        print(f"Failed to write updated STT text: {exc}")
        return False, "텍스트를 저장하지 못했습니다.", record_id

    if not record_id:
        history = load_upload_history(history_file)
        resolved_path = file_path.resolve()
        for record in history:
            folder = record.get("folder_name")
            if not folder:
                continue
            record_output_dir = (output_dir / folder).resolve()
            try:
                resolved_path.relative_to(record_output_dir)
                record_id = record["id"]
                break
            except ValueError:
                continue

    return True, "", record_id


def reset_tasks_for_record(
    record: dict,
    tasks: set[str],
    registry: dict,
    index: dict,
    base_dir: Path,
    registry_file: Path,
    output_dir: Path,
    vector_dir: Path,
    db_alias: str
) -> tuple[dict[str, bool], bool, bool]:
    """Reset selected task artifacts for a single record."""

    results = {task: False for task in TASK_TYPES}
    if not record or not tasks or record.get("deleted"):
        return results, False, False

    download_links = record.get("download_links", {})
    completed_tasks = record.setdefault("completed_tasks", {task: False for task in TASK_TYPES})

    registry_changed = False
    index_changed = False

    def cleanup_task(task_name: str, delete_file: bool) -> bool:
        nonlocal registry_changed

        link = download_links.get(task_name)
        if not link:
            return False

        file_path, _, _, resolved_identifier = resolve_file_identifier(link, base_dir, registry_file)

        if delete_file and file_path and file_path.exists():
            try:
                file_path.unlink()
            except Exception:
                pass

        if resolved_identifier and is_valid_uuid(resolved_identifier):
            entry = registry.get(resolved_identifier)
            if entry and entry.get("task_type") == task_name:
                del registry[resolved_identifier]
                registry_changed = True
        else:
            if file_path:
                try:
                    relative_path = to_record_path(file_path, base_dir)
                except Exception:
                    relative_path = file_path.as_posix()

                normalized = normalize_record_path(relative_path, base_dir)
                candidates = {relative_path, normalized}
                if normalized.startswith(f"{db_alias}/"):
                    candidates.add(normalized[len(db_alias) + 1 :])

                for key, info in list(registry.items()):
                    stored_path = normalize_record_path(info.get("file_path", ""), base_dir)
                    if info.get("task_type") == task_name and stored_path in candidates:
                        del registry[key]
                        registry_changed = True

        download_links.pop(task_name, None)
        completed_tasks[task_name] = False

        if task_name == "summary":
            record["title_summary"] = ""

        return True

    if "summary" in tasks and cleanup_task("summary", delete_file=True):
        results["summary"] = True

    embedding_removed = False
    if "embedding" in tasks and cleanup_task("embedding", delete_file=False):
        results["embedding"] = True
        embedding_removed = True

    stt_removed = False
    if "stt" in tasks and cleanup_task("stt", delete_file=True):
        results["stt"] = True
        stt_removed = True

    if embedding_removed:
        try:
            folder_name = record.get("folder_name", "")
            record_output_dir = output_dir / folder_name
            output_resolved = record_output_dir.resolve()
            keys_to_remove = []

            for key, meta in list(index.items()):
                try:
                    Path(key).resolve().relative_to(output_resolved)
                    keys_to_remove.append((key, meta))
                except (ValueError, FileNotFoundError):
                    continue

            if keys_to_remove:
                for key, meta in keys_to_remove:
                    vector_name = meta.get("vector")
                    if vector_name:
                        vector_path = vector_dir / vector_name
                        if vector_path.exists():
                            if not any(
                                v.get("vector") == vector_name and k != key
                                for k, v in index.items()
                            ):
                                try:
                                    vector_path.unlink()
                                except Exception:
                                    pass
                    del index[key]

                index_changed = True
        except Exception as exc:
            print(f"Failed to clean embedding vectors: {exc}")

    if stt_removed:
        try:
            original_path = record.get("file_path")
            folder_name = record.get("folder_name")
            if original_path and folder_name:
                source_path = resolve_record_path(original_path, base_dir)
                record_output_dir = output_dir / folder_name
                if source_path and record_output_dir.exists():
                    stem = Path(source_path).stem
                    corrected_file = record_output_dir / f"{stem}.corrected.md"
                    if corrected_file.exists():
                        try:
                            corrected_file.unlink()
                        except Exception:
                            pass
        except Exception as exc:
            print(f"Failed to clean STT artifacts: {exc}")

    return results, registry_changed, index_changed


def reset_summary_and_embedding(
    record_id: str,
    base_dir: Path,
    history_file: Path,
    registry_file: Path,
    output_dir: Path,
    vector_dir: Path,
    db_alias: str
) -> tuple[bool, str]:
    """Reset summary and embedding artifacts for a record."""

    if not record_id:
        return False, "record_id가 필요합니다."

    history = load_upload_history(history_file)
    record = next((item for item in history if item.get("id") == record_id), None)

    if not record:
        return False, "기록을 찾을 수 없습니다."

    registry = load_file_registry(registry_file)
    index = load_index()

    results, registry_changed, index_changed = reset_tasks_for_record(
        record,
        {"summary", "embedding"},
        registry,
        index,
        base_dir,
        registry_file,
        output_dir,
        vector_dir,
        db_alias
    )

    if registry_changed:
        save_file_registry(registry, registry_file)

    if index_changed:
        save_index(index)

    save_upload_history(history, history_file)

    summary_reset = results.get("summary", False)
    embedding_reset = results.get("embedding", False)

    if summary_reset and embedding_reset:
        message = "색인과 요약이 초기화되었습니다."
    elif summary_reset:
        message = "요약이 초기화되었습니다."
    elif embedding_reset:
        message = "색인이 초기화되었습니다."
    else:
        message = "초기화할 항목이 없습니다."

    return True, message


def reset_tasks_for_all_records(
    tasks: set[str],
    base_dir: Path,
    history_file: Path,
    registry_file: Path,
    output_dir: Path,
    vector_dir: Path,
    db_alias: str
) -> tuple[bool, dict[str, int], str]:
    """Reset selected task artifacts for every record in history."""

    valid_tasks = set(TASK_TYPES)
    requested_tasks = {task for task in tasks if task in valid_tasks}

    if not requested_tasks:
        return False, {task: 0 for task in valid_tasks}, "유효한 초기화 항목을 선택해주세요."

    history = load_upload_history(history_file)
    if not history:
        return True, {task: 0 for task in valid_tasks}, "초기화할 기록이 없습니다."

    registry = load_file_registry(registry_file)
    index = load_index()

    registry_changed = False
    index_changed = False
    reset_counts = {task: 0 for task in valid_tasks}

    for record in history:
        if record.get("deleted"):
            continue
        results, reg_changed, idx_changed = reset_tasks_for_record(
            record,
            requested_tasks,
            registry,
            index,
            base_dir,
            registry_file,
            output_dir,
            vector_dir,
            db_alias
        )

        if reg_changed:
            registry_changed = True
        if idx_changed:
            index_changed = True

        for task in requested_tasks:
            if results.get(task):
                reset_counts[task] += 1

    if registry_changed:
        save_file_registry(registry, registry_file)

    if index_changed:
        save_index(index)

    save_upload_history(history, history_file)

    return True, reset_counts, "초기화가 완료되었습니다."
