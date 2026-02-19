from __future__ import annotations

import re
from typing import Any


def format_speaker_label(index: int) -> str:
    """Return canonical speaker label format (e.g. SPEAKER_00)."""
    safe_index = max(0, int(index))
    return f"SPEAKER_{safe_index:02d}"


def normalize_speaker_label(raw_speaker: Any, default_index: int = 0) -> str:
    """Normalize various provider speaker formats into canonical SPEAKER_XX."""
    if isinstance(raw_speaker, int):
        return format_speaker_label(raw_speaker)

    if isinstance(raw_speaker, str):
        text = raw_speaker.strip()
        if not text:
            return format_speaker_label(default_index)

        matched = re.search(r"(\d+)$", text)
        if matched:
            return format_speaker_label(int(matched.group(1)))

        upper = text.upper()
        if upper.startswith("SPEAKER_"):
            suffix = upper.split("SPEAKER_", 1)[1]
            if suffix.isdigit():
                return format_speaker_label(int(suffix))

    return format_speaker_label(default_index)


def get_overlap_seconds(start_a: float, end_a: float, start_b: float, end_b: float) -> float:
    """Calculate overlap length (seconds) for two timeline windows."""
    return max(0.0, min(end_a, end_b) - max(start_a, start_b))


def align_diarization_to_stt_segments(
    stt_segments: list[dict[str, Any]],
    diarization_segments: list[dict[str, Any]],
    *,
    default_speaker: str | None = None,
    allow_null_speaker: bool = False,
) -> list[dict[str, Any]]:
    """Assign speaker labels to STT segments by maximum overlap with diarization segments."""
    aligned: list[dict[str, Any]] = []
    normalized_default = normalize_speaker_label(default_speaker) if default_speaker else format_speaker_label(0)

    normalized_diarization: list[dict[str, Any]] = []
    for diarization in diarization_segments:
        try:
            start = float(diarization.get("start", 0.0))
            end = float(diarization.get("end", 0.0))
        except (TypeError, ValueError):
            continue
        if end <= start:
            continue
        normalized_diarization.append(
            {
                "start": start,
                "end": end,
                "speaker": normalize_speaker_label(diarization.get("speaker"), default_index=0),
            }
        )

    for segment in stt_segments:
        mapped = dict(segment)
        try:
            start = float(mapped.get("start", 0.0))
            end = float(mapped.get("end", 0.0))
        except (TypeError, ValueError):
            start, end = 0.0, 0.0

        speaker: str | None = None
        best_overlap = 0.0

        for diarization in normalized_diarization:
            overlap = get_overlap_seconds(start, end, diarization["start"], diarization["end"])
            if overlap > best_overlap:
                best_overlap = overlap
                speaker = diarization["speaker"]

        if speaker is None:
            speaker = None if allow_null_speaker else normalized_default

        mapped["speaker"] = speaker
        aligned.append(mapped)

    return aligned
