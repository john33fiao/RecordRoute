from __future__ import annotations

import os
from dataclasses import dataclass
from pathlib import Path

from ...search_cache import delete_cache_record
from ...vector_search import search as search_vectors
from ..history import load_upload_history
from ..paths import BASE_DIR, OUTPUT_DIR, normalize_record_path, resolve_record_path, to_record_path
from ..registry import get_file_by_uuid, load_file_registry


class SimilarDocumentError(Exception):
    pass


class SimilarDocumentNotFound(SimilarDocumentError):
    pass


@dataclass
class ResolvedTarget:
    file_path: str
    full_path: Path
    current_file_name: str
    current_record_id: str | None


def _is_uuid(value: str) -> bool:
    import uuid

    try:
        uuid.UUID(value)
        return True
    except ValueError:
        return False


def resolve_target(file_identifier: str, *, user_filename: str | None = None) -> ResolvedTarget:
    if _is_uuid(file_identifier):
        file_info = get_file_by_uuid(file_identifier)
        if not file_info:
            raise SimilarDocumentNotFound("파일을 찾을 수 없습니다.")
        file_path = normalize_record_path(file_info["file_path"])
        full_path = resolve_record_path(file_path)
        return ResolvedTarget(
            file_path=file_path,
            full_path=full_path,
            current_file_name=user_filename or file_info["original_filename"],
            current_record_id=file_info.get("record_id"),
        )

    file_path = normalize_record_path(file_identifier)
    full_path = resolve_record_path(file_path)
    current_file_name = user_filename or os.path.basename(file_identifier) or full_path.name
    return ResolvedTarget(
        file_path=file_path,
        full_path=full_path,
        current_file_name=current_file_name,
        current_record_id=None,
    )


def resolve_text_file_for_similar(full_path: Path):
    text_suffixes = {".md", ".txt", ".text", ".markdown"}
    if full_path.suffix.lower() in text_suffixes:
        return full_path

    upload_uuid = full_path.parent.name
    stem = full_path.stem
    stt_output_dir = OUTPUT_DIR / upload_uuid

    if stt_output_dir.exists():
        candidates = [
            stt_output_dir / f"{stem}.summary.md",
            stt_output_dir / f"{stem}.corrected.md",
            stt_output_dir / f"{stem}.md",
        ]
        for candidate in candidates:
            if candidate.exists():
                print(f"[DEBUG] 유사문서 검색용 텍스트 파일 찾음: {candidate}")
                return candidate

        md_files = list(stt_output_dir.glob("*.md"))
        if md_files:
            chosen = md_files[0]
            print(f"[DEBUG] 유사문서 검색용 대체 텍스트 파일: {chosen}")
            return chosen

    print(f"[DEBUG] 유사문서 검색용 텍스트 파일을 찾지 못함: {full_path}")
    return None


def read_text_with_fallback(text_path: Path):
    content = None
    for enc in ["utf-8", "utf-8-sig", "cp949", "euc-kr", "utf-16"]:
        try:
            with open(text_path, "r", encoding=enc) as f:
                content = f.read()
            break
        except UnicodeDecodeError:
            continue
    if content is None:
        raise ValueError(f"텍스트 파일을 읽을 수 없습니다: {text_path}")
    return content


def _build_registry_path_map(registry: dict):
    registry_path_map: dict[str, tuple[str, dict]] = {}
    for uuid_key, info in registry.items():
        if not isinstance(info, dict):
            continue
        stored_norm = os.path.normpath(normalize_record_path(info.get("file_path", "")))
        if stored_norm:
            registry_path_map[stored_norm] = (uuid_key, info)
    return registry_path_map


def _resolve_current_record_id(history: list[dict], current_path_norm: str, current_record_id: str | None):
    if current_record_id:
        return current_record_id
    for record in history:
        try:
            record_path_norm = os.path.normpath(normalize_record_path(record.get("file_path", "")))
        except Exception:
            continue
        if record_path_norm == current_path_norm:
            return record.get("id")
    return None


def find_similar_documents(file_identifier: str, *, user_filename: str | None = None, refresh: bool = False):
    target = resolve_target(file_identifier, user_filename=user_filename)
    if not target.full_path.exists():
        raise SimilarDocumentNotFound("파일을 찾을 수 없습니다.")

    text_path = resolve_text_file_for_similar(target.full_path)
    if text_path is None:
        raise ValueError(f"검색에 사용할 텍스트 파일을 찾을 수 없습니다: {target.full_path}")

    content = read_text_with_fallback(text_path)
    if refresh:
        delete_cache_record(content, 20)

    hits = search_vectors(content, BASE_DIR, top_k=20)
    registry = load_file_registry()
    history = load_upload_history()

    history_by_id = {str(record.get("id")): record for record in history if record.get("id")}
    registry_path_map = _build_registry_path_map(registry)

    current_path_norm = os.path.normpath(normalize_record_path(target.file_path))
    current_text_path_norm = os.path.normpath(normalize_record_path(to_record_path(text_path)))
    current_record_id = _resolve_current_record_id(history, current_path_norm, target.current_record_id)

    seen_record_ids: set[str] = set()
    seen_paths: set[str] = set()
    similar_docs = []

    for hit in hits:
        normalized_hit = normalize_record_path(hit["file"])
        hit_path_norm = os.path.normpath(normalized_hit)
        if hit_path_norm in {current_path_norm, current_text_path_norm}:
            continue

        uuid_and_info = registry_path_map.get(hit_path_norm)
        file_uuid = uuid_and_info[0] if uuid_and_info else None
        hit_info = uuid_and_info[1] if uuid_and_info else None
        record_id = hit_info.get("record_id") if hit_info else None

        if current_record_id and record_id and record_id == current_record_id:
            continue
        if record_id and record_id in seen_record_ids:
            continue
        if hit_path_norm in seen_paths:
            continue

        user_filename_found = None
        title_summary = ""
        if record_id:
            record = history_by_id.get(str(record_id))
            if record:
                user_filename_found = record.get("filename")
                title_summary = (record.get("title_summary") or "").strip()

        display_filename = user_filename_found or os.path.basename(normalized_hit)
        download_link = f"/download/{file_uuid}" if file_uuid else f"/download/{normalized_hit}"

        similar_docs.append(
            {
                "file": normalized_hit,
                "score": hit["score"],
                "link": download_link,
                "display_name": display_filename,
                "title_summary": title_summary,
                "record_id": record_id,
            }
        )

        if record_id:
            seen_record_ids.add(record_id)
        seen_paths.add(hit_path_norm)

        if len(similar_docs) >= 5:
            break

    return similar_docs
