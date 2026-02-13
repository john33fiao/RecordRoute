from __future__ import annotations

from sttEngine.server.services.errors import DependencyError
from sttEngine.server.tasks.queue import run_process_task


def test_run_process_task_success(monkeypatch):
    expected = {"summary": "/download/abc/summary.md"}
    monkeypatch.setattr("sttEngine.server.tasks.queue.run_workflow", lambda *_args, **_kwargs: expected)

    result = run_process_task(
        {
            "absolute_path": "/tmp/file.wav",
            "steps": ["summary"],
            "record_id": "r1",
            "task_id": "t1",
            "model_settings": {},
        }
    )

    assert result == expected


def test_run_process_task_maps_workflow_error(monkeypatch):
    def raise_error(*_args, **_kwargs):
        raise DependencyError(message="ollama missing", code="dependency_ollama", retryable=True)

    monkeypatch.setattr("sttEngine.server.tasks.queue.run_workflow", raise_error)

    result = run_process_task(
        {
            "absolute_path": "/tmp/file.wav",
            "steps": ["summary"],
            "record_id": "r1",
            "task_id": "t2",
            "model_settings": {},
        }
    )

    assert result == {
        "error": "ollama missing",
        "error_code": "dependency_ollama",
        "retryable": True,
        "failed_step": "workflow",
    }
