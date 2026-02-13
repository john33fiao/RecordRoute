from __future__ import annotations

import json
import uuid

from ...http_api.paths import normalize_record_path, resolve_record_path
from .errors import ApiError


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

    return {
        "absolute_path": absolute_path,
        "steps": payload.get("steps", []),
        "record_id": payload.get("record_id"),
        "task_id": payload.get("task_id") or str(uuid.uuid4()),
        "model_settings": payload.get("model_settings", {}),
        "retry_mode": payload.get("retry_mode") or "new_task",
        "retry_of_task_id": payload.get("retry_of_task_id"),
    }
