from __future__ import annotations

from ...http_api.state import get_task_progress
from ..services.errors import send_error, send_json


def handle(handler, task_id: str) -> None:
    try:
        progress = get_task_progress(task_id)
        send_json(handler, 200, progress)
    except Exception as exc:
        send_error(handler, exc)
