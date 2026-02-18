from __future__ import annotations

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
