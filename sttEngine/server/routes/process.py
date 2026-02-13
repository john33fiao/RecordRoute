from __future__ import annotations

from ..services.errors import send_error, send_json
from ..services.file_service import parse_process_payload
from ..tasks.queue import run_process_task


def handle(handler) -> None:
    try:
        task_request = parse_process_payload(handler)
        results = run_process_task(task_request)
        send_json(handler, 200, results)
    except Exception as exc:
        send_error(handler, exc)
