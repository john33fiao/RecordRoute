from __future__ import annotations

import json
from dataclasses import dataclass


LEGACY_ERROR_CODE_ALIASES: dict[str, str] = {
    "dependency_ollama": "dependency_llm_provider",
}

# Diarization step (`failed_step == "diarize"`) draft error codes.
DIARIZATION_MODEL_UNAVAILABLE = "diarization_model_unavailable"
DIARIZATION_TIMEOUT = "diarization_timeout"
DIARIZATION_INVALID_AUDIO = "diarization_invalid_audio"


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

    if default_step == "diarize":
        if "timeout" in lower_message or isinstance(error, TimeoutError):
            return TransientWorkflowError(
                message=message,
                code=DIARIZATION_TIMEOUT,
                retryable=True,
                failed_step=default_step,
            )

        if (
            "model unavailable" in lower_message
            or "model not found" in lower_message
            or "failed to load model" in lower_message
        ):
            return DependencyError(
                message=message,
                code=DIARIZATION_MODEL_UNAVAILABLE,
                retryable=True,
                failed_step=default_step,
            )

        if isinstance(error, ValueError) or "invalid" in lower_message or "unsupported audio" in lower_message:
            return InputValidationError(
                message=message,
                code=DIARIZATION_INVALID_AUDIO,
                retryable=False,
                failed_step=default_step,
            )

    if "ffmpeg" in lower_message or "ffprobe" in lower_message:
        return DependencyError(message=message, code="dependency_ffmpeg", retryable=False, failed_step=default_step)

    if isinstance(error, (FileNotFoundError, ValueError)) or "missing" in lower_message or "invalid" in lower_message:
        return InputValidationError(message=message, code="input_error", retryable=False, failed_step=default_step)

    llm_provider_keywords = (
        "ollama",
        "llama",
        "llm provider",
        "provider",
        "model server",
        "openai-compatible",
    )
    if any(keyword in lower_message for keyword in llm_provider_keywords):
        return DependencyError(
            message=message,
            code="dependency_llm_provider",
            retryable=True,
            failed_step=default_step,
        )

    if "timeout" in lower_message or isinstance(error, TimeoutError):
        return TransientWorkflowError(message=message, code="transient_timeout", retryable=True, failed_step=default_step)

    return FatalWorkflowError(message=message, code="fatal_error", retryable=False, failed_step=default_step)


def normalize_error_code(error_code: str | None) -> str:
    if not error_code:
        return "fatal_error"
    return LEGACY_ERROR_CODE_ALIASES.get(error_code, error_code)


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
