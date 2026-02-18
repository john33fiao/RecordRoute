from __future__ import annotations

import json
from io import BytesIO

from sttEngine.http_api.routes import admin_routes


class _DummyModelsHandler:
    def __init__(self):
        self.status_code: int | None = None
        self.headers_sent: list[tuple[str, str]] = []
        self.wfile = BytesIO()

    def send_response(self, status_code: int) -> None:
        self.status_code = status_code

    def send_header(self, key: str, value: str) -> None:
        self.headers_sent.append((key, value))

    def end_headers(self) -> None:
        return


class _FakeProvider:
    def __init__(self, models: list[str], ok: bool = True, message: str = "ok"):
        self._models = models
        self._ok = ok
        self._message = message

    def healthcheck(self):
        return self._ok, self._message

    def list_models(self):
        return self._models


def test_models_endpoint_uses_provider_contract(monkeypatch):
    handler = _DummyModelsHandler()

    providers = {
        "ollama": _FakeProvider(["gpt-oss:20b"]),
        "llamacpp": _FakeProvider(["/models/llama-3.1.gguf"]),
    }

    monkeypatch.setenv("LLM_PROVIDER", "llama_cpp")
    monkeypatch.setattr("sttEngine.http_api.routes.admin_routes.get_llm_provider", lambda name: providers[name])

    admin_routes._serve_available_models(handler)

    assert handler.status_code == 200
    payload = json.loads(handler.wfile.getvalue().decode("utf-8"))
    assert payload["models"] == ["/models/llama-3.1.gguf"]
    assert payload["models_by_provider"]["ollama"] == []
    assert payload["models_by_provider"]["llamacpp"] == ["/models/llama-3.1.gguf"]
    assert payload["default"]["provider"] == "llamacpp"
    assert payload["provider_status"]["ollama"]["message"] == "not_checked"


def test_models_endpoint_respects_explicit_provider_query(monkeypatch):
    handler = _DummyModelsHandler()

    providers = {
        "ollama": _FakeProvider(["gpt-oss:20b"]),
        "llamacpp": _FakeProvider(["/models/llama-3.1.gguf"]),
    }

    monkeypatch.setenv("LLM_PROVIDER", "llama_cpp")
    monkeypatch.setattr("sttEngine.http_api.routes.admin_routes.get_llm_provider", lambda name: providers[name])

    admin_routes._serve_available_models(handler, provider_name="ollama")

    assert handler.status_code == 200
    payload = json.loads(handler.wfile.getvalue().decode("utf-8"))
    assert payload["models"] == ["gpt-oss:20b"]
    assert payload["default"]["provider"] == "ollama"
    assert payload["provider_status"]["ollama"]["ok"] is True
    assert payload["provider_status"]["llamacpp"]["message"] == "not_checked"
