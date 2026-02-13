from __future__ import annotations

from pathlib import Path

from sttEngine.http_api.workflow import run_workflow


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
