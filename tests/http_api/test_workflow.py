from __future__ import annotations

import sys
import types
from pathlib import Path

# Optional dependency guard for test environments without ollama installed.
sys.modules.setdefault("ollama", types.SimpleNamespace())

from sttEngine.http_api.workflow import _workflow_error_result, run_workflow


def test_workflow_correct_step_success_updates_progress(monkeypatch, tmp_path: Path, temp_workflow_dirs):
    upload_dir = tmp_path / "uploads" / "task-123"
    upload_dir.mkdir(parents=True)
    source = upload_dir / "note.txt"
    source.write_text("테스트 텍스트", encoding="utf-8")

    progress_messages: list[str] = []

    monkeypatch.setattr("sttEngine.http_api.workflow.correct_text_file", lambda **_kwargs: True)
    monkeypatch.setattr("sttEngine.http_api.workflow.update_task_progress", lambda _id, msg, **_k: progress_messages.append(msg))
    monkeypatch.setattr("sttEngine.http_api.workflow.clear_task_progress", lambda _id: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.is_task_cancelled", lambda _id: False)

    result = run_workflow(source, ["correct"], task_id="task-123")

    assert "correct" in result
    assert result["correct"].endswith("note.corrected.md")
    assert "교정 시작" in progress_messages


def test_workflow_correct_step_failure_returns_error_fields(monkeypatch, tmp_path: Path, temp_workflow_dirs):
    upload_dir = tmp_path / "uploads" / "task-999"
    upload_dir.mkdir(parents=True)
    source = upload_dir / "note.txt"
    source.write_text("테스트 텍스트", encoding="utf-8")

    monkeypatch.setattr(
        "sttEngine.http_api.workflow.correct_text_file",
        lambda **_kwargs: (_ for _ in ()).throw(RuntimeError("invalid prompt")),
    )
    monkeypatch.setattr("sttEngine.http_api.workflow.update_task_progress", lambda *_args, **_kwargs: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.clear_task_progress", lambda _id: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.is_task_cancelled", lambda _id: False)

    result = run_workflow(source, ["correct"], task_id="task-999")

    assert result["error"] == "invalid prompt"
    assert result["error_code"] == "input_error"
    assert result["retryable"] is False
    assert result["failed_step"] == "correct"


def test_workflow_passes_provider_to_correct_and_summary(monkeypatch, tmp_path: Path, temp_workflow_dirs):
    upload_dir = tmp_path / "uploads" / "task-provider"
    upload_dir.mkdir(parents=True)
    source = upload_dir / "note.txt"
    source.write_text("테스트 텍스트", encoding="utf-8")

    called: dict[str, dict] = {}

    def fake_correct_text_file(**kwargs):
        called["correct"] = kwargs
        kwargs["output_file"].write_text("교정 결과", encoding="utf-8")
        return True

    def fake_summarize_text_mapreduce(**kwargs):
        called["summary"] = kwargs
        return "요약 결과"

    monkeypatch.setattr("sttEngine.http_api.workflow.correct_text_file", fake_correct_text_file)
    monkeypatch.setattr("sttEngine.http_api.workflow.summarize_text_mapreduce", fake_summarize_text_mapreduce)
    monkeypatch.setattr("sttEngine.http_api.workflow.save_output", lambda content, path, as_json=False: path.write_text(content, encoding="utf-8"))
    monkeypatch.setattr("sttEngine.http_api.workflow.send_summary_to_obsidian_sync", lambda **_kwargs: {"success": False, "message": "disabled"})
    monkeypatch.setattr("sttEngine.http_api.workflow.update_task_progress", lambda *_args, **_kwargs: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.clear_task_progress", lambda _id: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.is_task_cancelled", lambda _id: False)

    model_settings = {"provider": "llamacpp", "correct": "model.gguf", "summarize": "model.gguf"}
    result = run_workflow(source, ["correct", "summary"], task_id="task-provider", model_settings=model_settings)

    assert result["correct"].endswith("note.corrected.md")
    assert result["summary"].endswith("note.corrected.summary.md")
    assert called["correct"]["provider_name"] == "llamacpp"
    assert called["summary"]["provider_name"] == "llamacpp"


def test_workflow_diarization_failure_contract_returns_standard_error_fields(monkeypatch):
    progress_payloads: list[dict[str, object]] = []

    monkeypatch.setattr(
        "sttEngine.http_api.workflow.update_task_progress",
        lambda _task_id, _message, **kwargs: progress_payloads.append(kwargs),
    )

    result = _workflow_error_result("task-diarize", RuntimeError("model unavailable for diarization"), "diarize")

    assert result["error"] == "model unavailable for diarization"
    assert result["error_code"] == "diarization_model_unavailable"
    assert result["retryable"] is True
    assert result["failed_step"] == "diarize"
    assert progress_payloads[-1]["error_code"] == "diarization_model_unavailable"
    assert progress_payloads[-1]["retryable"] is True
    assert progress_payloads[-1]["failed_step"] == "diarize"


def test_workflow_diarize_audio_returns_normalized_result(monkeypatch, tmp_path: Path, temp_workflow_dirs):
    upload_dir = tmp_path / "uploads" / "task-diarize-audio"
    upload_dir.mkdir(parents=True)
    source = upload_dir / "sample.wav"
    source.write_bytes(b"RIFF")

    monkeypatch.setattr("sttEngine.http_api.workflow.get_audio_duration", lambda _path: "00:10")
    monkeypatch.setattr("sttEngine.http_api.workflow.update_task_progress", lambda *_args, **_kwargs: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.clear_task_progress", lambda _id: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.is_task_cancelled", lambda _id: False)

    result = run_workflow(source, ["diarize"], task_id="task-diarize-audio")

    assert "diarize" in result
    assert result["diarize"]["status"] == "completed"
    assert result["diarize"]["input_file_type"] == "audio"
    assert result["diarize"]["segments"][0]["speaker"] == "SPEAKER_00"
    assert result["diarize"]["segments"][0]["start"] == 0.0
    assert result["diarize"]["segments"][0]["end"] == 10.0


def test_workflow_diarize_text_is_skipped_with_contract_payload(monkeypatch, tmp_path: Path, temp_workflow_dirs):
    upload_dir = tmp_path / "uploads" / "task-diarize-text"
    upload_dir.mkdir(parents=True)
    source = upload_dir / "note.txt"
    source.write_text("테스트 텍스트", encoding="utf-8")

    monkeypatch.setattr("sttEngine.http_api.workflow.update_task_progress", lambda *_args, **_kwargs: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.clear_task_progress", lambda _id: None)
    monkeypatch.setattr("sttEngine.http_api.workflow.is_task_cancelled", lambda _id: False)

    result = run_workflow(source, ["diarize"], task_id="task-diarize-text")

    assert "diarize" in result
    assert result["diarize"] == {
        "status": "skipped",
        "reason": "non_audio_input",
        "input_file_type": "text",
        "segments": [],
    }
