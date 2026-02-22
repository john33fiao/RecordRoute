from __future__ import annotations

from ...http_api.history import get_active_history
from ..services.errors import send_error, send_json


def handle(handler) -> None:
    try:
        history = get_active_history()
        send_json(handler, 200, history)
    except Exception as exc:
        send_error(handler, exc)
