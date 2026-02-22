from __future__ import annotations

from sttEngine.server.services.errors import (
    LEGACY_ERROR_CODE_ALIASES,
    WorkflowError,
    map_workflow_exception,
    normalize_error_code,
)


def test_map_workflow_exception_maps_provider_dependency_to_generic_code() -> None:
    mapped = map_workflow_exception(RuntimeError("ollama server is not running"), "summary")

    assert mapped.code == "dependency_llm_provider"
    assert mapped.retryable is True
    assert mapped.failed_step == "summary"


def test_map_workflow_exception_keeps_ffmpeg_dependency_specific() -> None:
    mapped = map_workflow_exception(RuntimeError("ffmpeg executable missing"), "stt")

    assert mapped.code == "dependency_ffmpeg"
    assert mapped.retryable is False
    assert mapped.failed_step == "stt"


def test_normalize_error_code_keeps_backward_compatibility_aliases() -> None:
    assert LEGACY_ERROR_CODE_ALIASES["dependency_ollama"] == "dependency_llm_provider"
    assert normalize_error_code("dependency_ollama") == "dependency_llm_provider"


def test_normalize_error_code_with_empty_code_returns_fallback() -> None:
    assert normalize_error_code(None) == "fatal_error"


def test_normalize_error_code_preserves_current_codes() -> None:
    err = WorkflowError("boom", code="dependency_llm_provider")
    assert normalize_error_code(err.code) == "dependency_llm_provider"


def test_map_workflow_exception_preserves_existing_workflow_error_code() -> None:
    legacy = WorkflowError("legacy", code="dependency_ollama", retryable=True)
    mapped = map_workflow_exception(legacy, "workflow")

    assert mapped.code == "dependency_ollama"
    assert mapped.failed_step == "workflow"


def test_map_workflow_exception_maps_diarization_model_unavailable() -> None:
    mapped = map_workflow_exception(RuntimeError("diarization model unavailable on server"), "diarize")

    assert mapped.code == "diarization_model_unavailable"
    assert mapped.retryable is True
    assert mapped.failed_step == "diarize"


def test_map_workflow_exception_maps_diarization_timeout() -> None:
    mapped = map_workflow_exception(TimeoutError("diarization timeout after 30s"), "diarize")

    assert mapped.code == "diarization_timeout"
    assert mapped.retryable is True
    assert mapped.failed_step == "diarize"


def test_map_workflow_exception_maps_diarization_invalid_audio() -> None:
    mapped = map_workflow_exception(ValueError("invalid audio format for diarization"), "diarize")

    assert mapped.code == "diarization_invalid_audio"
    assert mapped.retryable is False
    assert mapped.failed_step == "diarize"
