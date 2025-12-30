"""File utility functions for file type detection, hash computation, and metadata."""

import hashlib
import subprocess
import uuid
from pathlib import Path


def get_file_type(file_path: Path) -> str:
    """Determine if the file is audio or text.

    Returns:
        'audio' for audio files, 'text' for text files, 'pdf' for PDF files, 'unknown' for others.
    """
    audio_extensions = {'.flac', '.m4a', '.mp3', '.mp4', '.mpeg', '.mpga', '.oga', '.ogg', '.qta', '.wav', '.webm'}
    text_extensions = {'.md', '.txt', '.text', '.markdown'}
    pdf_extensions = {'.pdf'}

    suffix = file_path.suffix.lower()
    if suffix in audio_extensions:
        return 'audio'
    elif suffix in text_extensions:
        return 'text'
    elif suffix in pdf_extensions:
        return 'pdf'
    else:
        return 'unknown'


def get_audio_duration(file_path: Path) -> str | None:
    """Get audio file duration using ffprobe."""
    try:
        result = subprocess.run([
            'ffprobe', '-v', 'quiet', '-show_entries', 'format=duration',
            '-of', 'csv=p=0', str(file_path)
        ], capture_output=True, text=True, check=True)
        duration = float(result.stdout.strip())
        minutes = int(duration // 60)
        seconds = int(duration % 60)
        return f"{minutes:02d}:{seconds:02d}"
    except (subprocess.CalledProcessError, ValueError, FileNotFoundError):
        return None


def compute_file_hash(data: bytes) -> str:
    """Compute SHA256 hash for given file data."""
    return hashlib.sha256(data).hexdigest()


def file_hash(path: Path) -> str:
    """Return a stable SHA256 checksum for the given file."""
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()


def is_valid_uuid(value: str) -> bool:
    """Check whether a string is a valid UUID value."""
    if not value:
        return False
    try:
        uuid.UUID(str(value))
        return True
    except (ValueError, TypeError):
        return False
