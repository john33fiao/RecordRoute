from __future__ import annotations

import json
from dataclasses import dataclass


@dataclass
class ApiError(Exception):
    message: str
    status_code: int = 400
    code: str = "bad_request"


def send_json(handler, status_code: int, payload: dict) -> None:
    handler.send_response(status_code)
    handler.send_header("Content-Type", "application/json")
    handler.end_headers()
    handler.wfile.write(json.dumps(payload, ensure_ascii=False).encode())


def send_error(handler, error: Exception) -> None:
    if isinstance(error, ApiError):
        send_json(
            handler,
            error.status_code,
            {"success": False, "error": {"code": error.code, "message": error.message}},
        )
        return

    send_json(
        handler,
        500,
        {
            "success": False,
            "error": {"code": "internal_error", "message": "Internal server error"},
        },
    )
