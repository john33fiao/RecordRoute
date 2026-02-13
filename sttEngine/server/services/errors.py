from __future__ import annotations

import json
from dataclasses import dataclass


@dataclass
class ApiError(Exception):
    message: str
    status_code: int = 400
    code: str = "bad_request"


@dataclass
class WorkflowError(Exception):
    message: str
    code: str = "workflow_error"
    retryable: bool = False
    failed_step: str | None = None


class InputValidationError(WorkflowError):
    pass


class DependencyError(WorkflowError):
    pass


class TransientWorkflowError(WorkflowError):
    pass


class FatalWorkflowError(WorkflowError):
    pass


def map_workflow_exception(error: Exception, default_step: str | None = None) -> WorkflowError:
    if isinstance(error, WorkflowError):
        if default_step and not error.failed_step:
            error.failed_step = default_step
        return error

    message = str(error) or "Unknown workflow error"
    lower_message = message.lower()

    if isinstance(error, (FileNotFoundError, ValueError)) or "missing" in lower_message or "invalid" in lower_message:
        return InputValidationError(message=message, code="input_error", retryable=False, failed_step=default_step)

    if "ollama" in lower_message:
        return DependencyError(message=message, code="dependency_ollama", retryable=True, failed_step=default_step)

    if "ffmpeg" in lower_message or "ffprobe" in lower_message:
        return DependencyError(message=message, code="dependency_ffmpeg", retryable=False, failed_step=default_step)

    if "timeout" in lower_message or isinstance(error, TimeoutError):
        return TransientWorkflowError(message=message, code="transient_timeout", retryable=True, failed_step=default_step)

    return FatalWorkflowError(message=message, code="fatal_error", retryable=False, failed_step=default_step)


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
