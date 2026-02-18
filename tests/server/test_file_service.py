from __future__ import annotations

from sttEngine.server.services.errors import ApiError
from sttEngine.server.services.file_service import normalize_process_steps, parse_process_payload


def test_normalize_process_steps_lowercases_deduplicates_and_preserves_order() -> None:
    assert normalize_process_steps([" STT ", "Diarize", "summary", "stt", "", 1]) == [
        "stt",
        "diarize",
        "summary",
    ]


def test_normalize_process_steps_maps_summarize_alias_to_summary() -> None:
    assert normalize_process_steps(["summarize", "summary", "Summarize"]) == ["summary"]


def test_parse_process_payload_normalizes_steps(dummy_handler_factory, monkeypatch) -> None:
    monkeypatch.setattr("sttEngine.server.services.file_service.resolve_record_path", lambda _path: "/tmp/file.wav")

    handler = dummy_handler_factory(
        {
            "file_path": "DB/uploads/uuid/sample.wav",
            "steps": [" STT ", "Diarize", "summary", "stt"],
        }
    )

    parsed = parse_process_payload(handler)

    assert parsed["steps"] == ["stt", "diarize", "summary"]


def test_parse_process_payload_sets_default_diarization_provider(dummy_handler_factory, monkeypatch) -> None:
    monkeypatch.setattr("sttEngine.server.services.file_service.resolve_record_path", lambda _path: "/tmp/file.wav")

    handler = dummy_handler_factory(
        {
            "file_path": "DB/uploads/uuid/sample.wav",
            "steps": ["diarize"],
            "model_settings": {},
        }
    )

    parsed = parse_process_payload(handler)

    assert parsed["model_settings"]["diarization_provider"] == "pyannote"


def test_parse_process_payload_validates_diarization_speaker_ranges(dummy_handler_factory, monkeypatch) -> None:
    monkeypatch.setattr("sttEngine.server.services.file_service.resolve_record_path", lambda _path: "/tmp/file.wav")

    handler = dummy_handler_factory(
        {
            "file_path": "DB/uploads/uuid/sample.wav",
            "steps": ["diarize"],
            "model_settings": {"min_speakers": 3, "max_speakers": 2},
        }
    )

    try:
        parse_process_payload(handler)
    except ApiError as exc:
        assert exc.code == "invalid_model_settings"
        assert "min_speakers" in exc.message
    else:
        raise AssertionError("Expected ApiError for invalid min/max speaker constraints")


def test_parse_process_payload_rejects_non_integer_num_speakers(dummy_handler_factory, monkeypatch) -> None:
    monkeypatch.setattr("sttEngine.server.services.file_service.resolve_record_path", lambda _path: "/tmp/file.wav")

    handler = dummy_handler_factory(
        {
            "file_path": "DB/uploads/uuid/sample.wav",
            "steps": ["diarize"],
            "model_settings": {"num_speakers": "2"},
        }
    )

    try:
        parse_process_payload(handler)
    except ApiError as exc:
        assert exc.code == "invalid_model_settings"
        assert "num_speakers" in exc.message
    else:
        raise AssertionError("Expected ApiError for non-integer num_speakers")
