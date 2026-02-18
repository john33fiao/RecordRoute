from __future__ import annotations

import json
import uuid

from ...http_api.paths import normalize_record_path, resolve_record_path
from .errors import ApiError


def normalize_process_steps(raw_steps) -> list[str]:
    """Normalize process step names while preserving order and compatibility."""
    if not isinstance(raw_steps, list):
        return []

    step_aliases = {
        "summarize": "summary",
    }

    normalized: list[str] = []
    seen: set[str] = set()
    for step in raw_steps:
        if not isinstance(step, str):
            continue
        normalized_step = step_aliases.get(step.strip().lower(), step.strip().lower())
        if not normalized_step or normalized_step in seen:
            continue
        seen.add(normalized_step)
        normalized.append(normalized_step)
    return normalized


def parse_process_payload(handler) -> dict:
    length = int(handler.headers.get("Content-Length", 0))
    try:
        payload = json.loads(handler.rfile.read(length)) if length else {}
    except json.JSONDecodeError as exc:
        raise ApiError("Invalid JSON payload", 400, "invalid_json") from exc

    file_path = payload.get("file_path")
    if not file_path:
        raise ApiError("Missing file_path", 400, "missing_file_path")

    normalized_path = normalize_record_path(file_path)
    absolute_path = resolve_record_path(normalized_path)

    model_settings = _normalize_model_settings(payload.get("model_settings"))

    return {
        "absolute_path": absolute_path,
        "steps": normalize_process_steps(payload.get("steps", [])),
        "record_id": payload.get("record_id"),
        "task_id": payload.get("task_id") or str(uuid.uuid4()),
        "model_settings": model_settings,
        "retry_mode": payload.get("retry_mode") or "new_task",
        "retry_of_task_id": payload.get("retry_of_task_id"),
    }


def _normalize_model_settings(raw_model_settings) -> dict:
    if raw_model_settings is None:
        model_settings: dict = {}
    elif isinstance(raw_model_settings, dict):
        model_settings = dict(raw_model_settings)
    else:
        raise ApiError("model_settings must be an object", 400, "invalid_model_settings")

    # Default diarization provider when omitted.
    diarization_provider = model_settings.get("diarization_provider")
    if diarization_provider in (None, ""):
        model_settings["diarization_provider"] = "pyannote"
    elif not isinstance(diarization_provider, str):
        raise ApiError("model_settings.diarization_provider must be a string", 400, "invalid_model_settings")

    num_speakers = _validate_optional_positive_int(model_settings.get("num_speakers"), "num_speakers")
    min_speakers = _validate_optional_positive_int(model_settings.get("min_speakers"), "min_speakers")
    max_speakers = _validate_optional_positive_int(model_settings.get("max_speakers"), "max_speakers")

    if min_speakers is not None and max_speakers is not None and min_speakers > max_speakers:
        raise ApiError("model_settings.min_speakers must be <= model_settings.max_speakers", 400, "invalid_model_settings")

    return model_settings


def _validate_optional_positive_int(value, field_name: str, *, min_value: int = 1, max_value: int = 20) -> int | None:
    if value is None:
        return None
    if isinstance(value, bool) or not isinstance(value, int):
        raise ApiError(f"model_settings.{field_name} must be an integer", 400, "invalid_model_settings")
    if value < min_value or value > max_value:
        raise ApiError(
            f"model_settings.{field_name} must be between {min_value} and {max_value}",
            400,
            "invalid_model_settings",
        )
    return value
