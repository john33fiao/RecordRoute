from __future__ import annotations

import sys
from pathlib import Path

from ..config import (
    DB_ALIAS,
    get_db_base_path,
    normalize_db_record_path,
    resolve_db_path,
    to_db_record_path,
)


# NOTE: server.py previously lived at sttEngine/server.py (repo root is parent.parent).
# This module lives at sttEngine/http_api/paths.py (repo root is parents[2]).
_DEFAULT_BASE_DIR = Path(__file__).resolve().parents[2]
BASE_DIR = Path(getattr(sys, "_MEIPASS", _DEFAULT_BASE_DIR)).resolve()

DB_BASE_PATH = get_db_base_path(BASE_DIR)
UPLOAD_DIR = DB_BASE_PATH / "uploads"
OUTPUT_DIR = DB_BASE_PATH / "whisper_output"
VECTOR_DIR = DB_BASE_PATH / "vector_store"
HISTORY_FILE = DB_BASE_PATH / "upload_history.json"
FILE_REGISTRY_FILE = DB_BASE_PATH / "file_registry.json"
DELETED_DIR = DB_BASE_PATH / "deleted"
DELETED_UPLOAD_DIR = DELETED_DIR / "uploads"
DELETED_OUTPUT_DIR = DELETED_DIR / "whisper_output"
DELETED_VECTOR_DIR = DELETED_DIR / "vector_store"

SEARCHABLE_SUFFIXES = {".md", ".txt", ".text", ".markdown"}
TASK_TYPES = ("stt", "embedding", "summary")


def normalize_record_path(path_str: str) -> str:
    """Normalize stored record paths using the configured DB base path."""
    return normalize_db_record_path(path_str, BASE_DIR)


def to_record_path(path: Path) -> str:
    """Convert an absolute path to a stored record path."""
    return to_db_record_path(path, BASE_DIR)


def resolve_record_path(path_str: str) -> Path:
    """Resolve a stored record path to an absolute filesystem path."""
    return resolve_db_path(path_str, BASE_DIR)
