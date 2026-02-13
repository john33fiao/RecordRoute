from __future__ import annotations

import json
from io import BytesIO
from pathlib import Path

import pytest


class DummyHandler:
    """Minimal request handler stub for route/service tests."""

    def __init__(self, payload: dict | None = None):
        raw = json.dumps(payload or {}).encode("utf-8")
        self.headers = {"Content-Length": str(len(raw))}
        self.rfile = BytesIO(raw)
        self.status_code = None
        self.headers_sent: list[tuple[str, str]] = []
        self.wfile = BytesIO()

    def send_response(self, status_code: int) -> None:
        self.status_code = status_code

    def send_header(self, key: str, value: str) -> None:
        self.headers_sent.append((key, value))

    def end_headers(self) -> None:
        return


@pytest.fixture
def dummy_handler_factory():
    def _factory(payload: dict | None = None) -> DummyHandler:
        return DummyHandler(payload)

    return _factory


@pytest.fixture
def temp_workflow_dirs(monkeypatch: pytest.MonkeyPatch, tmp_path: Path):
    """Isolate output directory writes from workflow tests."""
    output_dir = tmp_path / "whisper_output"
    output_dir.mkdir(parents=True, exist_ok=True)

    from sttEngine.http_api import workflow

    monkeypatch.setattr(workflow, "OUTPUT_DIR", output_dir)
    return output_dir


@pytest.fixture
def temp_vocab_path(tmp_path: Path) -> Path:
    return tmp_path / "vocab.json"
