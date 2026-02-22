from __future__ import annotations

import shutil
from datetime import datetime
from pathlib import Path
from typing import Any

from ..embedding_pipeline import load_index, save_index
from .history import load_upload_history, save_upload_history
from .paths import (
    DB_ALIAS,
    DELETED_OUTPUT_DIR,
    DELETED_UPLOAD_DIR,
    DELETED_VECTOR_DIR,
    OUTPUT_DIR,
    TASK_TYPES,
    UPLOAD_DIR,
    VECTOR_DIR,
    normalize_record_path,
    resolve_record_path,
    to_record_path,
)
from .registry import (
    get_file_by_uuid,
    is_valid_uuid,
    load_file_registry,
    resolve_file_identifier,
    save_file_registry,
)


def reset_upload_record(record_id: str) -> bool:
    """Remove processed files and reset completion status for a record."""
    history = load_upload_history()

    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return False
            folder = record.get("folder_name")
            output_dir = OUTPUT_DIR / folder if folder else None
            try:
                if output_dir and output_dir.exists():
                    shutil.rmtree(output_dir)
            except Exception:
                pass

            # Remove embedding vectors and index entries related to this record
            if output_dir:
                index = load_index()
                keys_to_remove = []
                for key, meta in index.items():
                    try:
                        Path(key).resolve().relative_to(output_dir.resolve())
                        keys_to_remove.append((key, meta))
                    except ValueError:
                        continue

                for key, meta in keys_to_remove:
                    vector_name = meta.get("vector")
                    if vector_name:
                        vector_path = VECTOR_DIR / vector_name
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

            record["completed_tasks"] = {task: False for task in TASK_TYPES}
            record["download_links"] = {}
            record["title_summary"] = ""

            save_upload_history(history)
            return True

    return False


def delete_file(file_identifier: str, file_type: str) -> tuple[bool, str]:
    """Delete a specific task artifact for a record and update metadata."""
    try:
        normalized_task = str(file_type or "").strip().lower()
        if normalized_task not in TASK_TYPES:
            return False, "지원하지 않는 항목입니다."

        file_path, record_id, resolved_task_type, _ = resolve_file_identifier(file_identifier)
        if not file_path:
            return False, "파일을 찾을 수 없습니다."

        expected_task_type = resolved_task_type
        if expected_task_type is None:
            suffix = file_path.name
            if suffix.endswith(".summary.md"):
                expected_task_type = "summary"

        if expected_task_type and expected_task_type != normalized_task:
            return False, "요청한 항목과 파일 유형이 일치하지 않습니다."

        if not record_id:
            return False, "연결된 기록을 찾을 수 없습니다."

        history = load_upload_history()
        record = next((item for item in history if item.get("id") == record_id), None)
        if not record:
            return False, "기록을 찾을 수 없습니다."
        if record.get("deleted"):
            return False, "삭제된 항목입니다."

        registry = load_file_registry()
        index = load_index()

        results, registry_changed, index_changed = reset_tasks_for_record(
            record,
            {normalized_task},
            registry,
            index,
        )

        if not results.get(normalized_task):
            return False, "삭제할 항목이 없습니다."

        if registry_changed:
            save_file_registry(registry)
        if index_changed:
            save_index(index)
        save_upload_history(history)

        return True, ""

    except Exception as e:
        print(f"Error deleting file: {e}")
        return False, f"삭제 중 오류가 발생했습니다: {str(e)}"


def _delete_single_record_assets(
    record: dict,
    registry: dict,
    index: dict,
    moved_vector_names: set[str],
) -> dict:
    """Move record assets to the deleted area and update metadata."""

    record_id = record.get("id")
    folder_name = record.get("folder_name")
    deleted_at = datetime.now().isoformat()

    upload_dir = (UPLOAD_DIR / folder_name).resolve() if folder_name else None
    deleted_upload_dir = (DELETED_UPLOAD_DIR / folder_name).resolve() if folder_name else None
    output_dir = (OUTPUT_DIR / folder_name).resolve() if folder_name else None
    deleted_output_dir = (DELETED_OUTPUT_DIR / folder_name).resolve() if folder_name else None
    vector_dir = VECTOR_DIR.resolve()
    deleted_vector_dir = DELETED_VECTOR_DIR.resolve()

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
            absolute_path = resolve_record_path(file_path_str)
        except Exception:
            absolute_path = None

        new_path: Path | None = None
        if absolute_path is not None:
            if upload_dir:
                try:
                    rel = absolute_path.relative_to(upload_dir)
                    new_path = deleted_upload_dir / rel
                except ValueError:
                    pass
            if new_path is None and output_dir:
                try:
                    rel = absolute_path.relative_to(output_dir)
                    new_path = deleted_output_dir / rel
                except ValueError:
                    pass
            if new_path is None:
                try:
                    rel = absolute_path.relative_to(vector_dir)
                    new_path = deleted_vector_dir / rel
                except ValueError:
                    pass

        if new_path is None:
            continue

        registry_updates.append((file_uuid, info, new_path))

    index_entries: list[tuple[str, dict, Path]] = []
    vector_names: set[str] = set()
    if output_dir:
        for key, meta in (index or {}).items():
            if not isinstance(meta, dict):
                continue
            try:
                rel = Path(key).resolve().relative_to(output_dir)
            except (ValueError, FileNotFoundError):
                continue

            deleted_path = (deleted_output_dir / rel) if deleted_output_dir else None
            if deleted_path is None:
                continue

            index_entries.append((key, meta, deleted_path))

            vector_name = meta.get("vector")
            if vector_name:
                vector_names.add(vector_name)

    if upload_dir and upload_dir.exists():
        deleted_upload_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(upload_dir), str(deleted_upload_dir))
        record_assets["uploads"] = to_record_path(deleted_upload_dir)

    if output_dir and output_dir.exists():
        deleted_output_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(output_dir), str(deleted_output_dir))
        record_assets["outputs"] = to_record_path(deleted_output_dir)

    for file_uuid, info, new_path in registry_updates:
        info["file_path"] = to_record_path(new_path)
        info["deleted"] = True
        info["deleted_at"] = deleted_at
        task_type = info.get("task_type")
        if task_type:
            files_assets.setdefault(task_type, []).append(info["file_path"])
        registry_changed = True

    if files_assets:
        record_assets["files"] = files_assets

    moved_vector_paths: list[str] = []
    for _, meta, deleted_path in index_entries:
        meta["deleted"] = True
        meta["deleted_at"] = deleted_at
        meta["deleted_path"] = str(deleted_path)
        vector_name = meta.get("vector")
        if vector_name:
            deleted_vector_path = DELETED_VECTOR_DIR / vector_name
            meta["vector_deleted_path"] = str(deleted_vector_path)
        index_changed = True

    for vector_name in vector_names:
        target_path = DELETED_VECTOR_DIR / vector_name
        if vector_name not in moved_vector_names:
            source_path = VECTOR_DIR / vector_name
            target_path.parent.mkdir(parents=True, exist_ok=True)
            if source_path.exists():
                shutil.move(str(source_path), str(target_path))
            moved_vector_names.add(vector_name)
        moved_vector_paths.append(to_record_path(target_path))

    if moved_vector_paths:
        record_assets["vectors"] = moved_vector_paths

    record["deleted"] = True
    record["deleted_at"] = deleted_at
    record["deleted_assets"] = record_assets

    return {"registry_changed": registry_changed, "index_changed": index_changed}


def delete_records(record_ids: list[str]) -> tuple[bool, dict[str, dict]]:
    """Delete multiple upload records by moving their assets to a deleted folder."""

    if not record_ids:
        return False, {}

    history = load_upload_history()
    registry = load_file_registry()
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
            results[record_id] = {"success": False, "error": "기록을 찾을 수 없습니다."}
            continue

        if record.get("deleted"):
            results[record_id] = {"success": False, "error": "이미 삭제된 항목입니다."}
            continue

        try:
            summary = _delete_single_record_assets(record, registry, index, moved_vector_names)
            history_changed = True
            registry_changed = registry_changed or summary.get("registry_changed", False)
            index_changed = index_changed or summary.get("index_changed", False)
            results[record_id] = {"success": True}
        except Exception as exc:
            results[record_id] = {"success": False, "error": str(exc)}

    if history_changed:
        save_upload_history(history)
    if registry_changed:
        save_file_registry(registry)
    if index_changed:
        save_index(index)

    overall_success = bool(results) and all(result.get("success") for result in results.values())
    return overall_success, results


def update_stt_text(file_identifier: str, new_text: str) -> tuple[bool, str, str | None]:
    """Update the contents of an STT result file."""

    if not file_identifier:
        return False, "파일 식별자가 필요합니다.", None

    file_path, record_id, task_type, _ = resolve_file_identifier(file_identifier)

    if not file_path or not file_path.exists():
        return False, "파일을 찾을 수 없습니다.", record_id

    if file_path.name.endswith(".summary.md"):
        return False, "요약 파일은 수정할 수 없습니다.", record_id

    if task_type and task_type not in ("stt", "embedding"):
        return False, "STT 파일만 수정할 수 있습니다.", record_id

    if file_path.suffix.lower() not in {".md", ".txt", ".text", ".markdown"}:
        return False, "지원하지 않는 파일 형식입니다.", record_id

    try:
        file_path.write_text(new_text, encoding="utf-8")
    except Exception as exc:
        print(f"Failed to write updated STT text: {exc}")
        return False, "텍스트를 저장하지 못했습니다.", record_id

    if not record_id:
        history = load_upload_history()
        resolved_path = file_path.resolve()
        for record in history:
            folder = record.get("folder_name")
            if not folder:
                continue
            output_dir = (OUTPUT_DIR / folder).resolve()
            try:
                resolved_path.relative_to(output_dir)
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

        file_path, _, _, resolved_identifier = resolve_file_identifier(link)

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
                    relative_path = to_record_path(file_path)
                except Exception:
                    relative_path = file_path.as_posix()

                normalized = normalize_record_path(relative_path)
                candidates = {relative_path, normalized}
                if normalized.startswith(f"{DB_ALIAS}/"):
                    candidates.add(normalized[len(DB_ALIAS) + 1 :])

                for key, info in list(registry.items()):
                    stored_path = normalize_record_path(info.get("file_path", ""))
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
            output_dir = OUTPUT_DIR / folder_name
            output_resolved = output_dir.resolve()
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
                        vector_path = VECTOR_DIR / vector_name
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
                source_path = resolve_record_path(original_path)
                output_dir = OUTPUT_DIR / folder_name
                if source_path and output_dir.exists():
                    stem = Path(source_path).stem
                    corrected_file = output_dir / f"{stem}.corrected.md"
                    if corrected_file.exists():
                        try:
                            corrected_file.unlink()
                        except Exception:
                            pass
        except Exception as exc:
            print(f"Failed to clean STT artifacts: {exc}")

    return results, registry_changed, index_changed


def reset_summary_and_embedding(record_id: str) -> tuple[bool, str]:
    """Reset summary and embedding artifacts for a record."""

    if not record_id:
        return False, "record_id가 필요합니다."

    history = load_upload_history()
    record = next((item for item in history if item.get("id") == record_id), None)

    if not record:
        return False, "기록을 찾을 수 없습니다."

    registry = load_file_registry()
    index = load_index()

    results, registry_changed, index_changed = reset_tasks_for_record(
        record,
        {"summary", "embedding"},
        registry,
        index,
    )

    if registry_changed:
        save_file_registry(registry)

    if index_changed:
        save_index(index)

    save_upload_history(history)

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


def reset_tasks_for_all_records(tasks: set[str]) -> tuple[bool, dict[str, int], str]:
    """Reset selected task artifacts for every record in history."""

    valid_tasks = set(TASK_TYPES)
    requested_tasks = {task for task in tasks if task in valid_tasks}

    if not requested_tasks:
        return False, {task: 0 for task in valid_tasks}, "유효한 초기화 항목을 선택해주세요."

    history = load_upload_history()
    if not history:
        return True, {task: 0 for task in valid_tasks}, "초기화할 기록이 없습니다."

    registry = load_file_registry()
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
        )

        if reg_changed:
            registry_changed = True
        if idx_changed:
            index_changed = True

        for task in requested_tasks:
            if results.get(task):
                reset_counts[task] += 1

    if registry_changed:
        save_file_registry(registry)

    if index_changed:
        save_index(index)

    save_upload_history(history)

    labels = {"stt": "STT", "embedding": "색인", "summary": "요약"}
    summary_parts = [
        f"{labels[task]} {reset_counts[task]}건" for task in requested_tasks if reset_counts.get(task)
    ]

    if not summary_parts:
        message = "초기화할 항목이 없습니다."
    else:
        message = ", ".join(summary_parts) + " 초기화되었습니다."

    return True, reset_counts, message
