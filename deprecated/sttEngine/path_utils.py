"""Path utility functions for managing file paths."""

from pathlib import Path

try:
    from .config import (
        normalize_db_record_path,
        resolve_db_path,
        to_db_record_path,
    )
except ImportError:
    from config import (
        normalize_db_record_path,
        resolve_db_path,
        to_db_record_path,
    )


def normalize_record_path(path_str: str, base_dir: Path) -> str:
    """Normalize stored record paths using the configured DB base path."""
    return normalize_db_record_path(path_str, base_dir)


def to_record_path(path: Path, base_dir: Path) -> str:
    """Convert an absolute path to a stored record path."""
    return to_db_record_path(path, base_dir)


def resolve_record_path(path_str: str, base_dir: Path) -> Path:
    """Resolve a stored record path to an absolute filesystem path."""
    return resolve_db_path(path_str, base_dir)
