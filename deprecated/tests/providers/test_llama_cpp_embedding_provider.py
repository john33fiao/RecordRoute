from __future__ import annotations

import numpy as np
import pytest

from sttEngine.providers.base import ProviderConfigurationError, ProviderRequestError
from sttEngine.providers.llama_cpp_provider import LlamaCppEmbeddingProvider


class _Response:
    def __init__(self, status_code: int, payload: dict | None = None, text: str = "") -> None:
        self.status_code = status_code
        self._payload = payload or {}
        self.text = text
        self.content = b"{}"

    def raise_for_status(self) -> None:
        if self.status_code >= 400:
            import requests

            raise requests.HTTPError(f"status={self.status_code}")

    def json(self) -> dict:
        return self._payload


def test_llamacpp_embedding_provider_calls_openai_compatible_endpoint(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = LlamaCppEmbeddingProvider()

    monkeypatch.setenv("EMBEDDING_BASE_URL", "http://127.0.0.1:8081")

    def fake_post(url: str, json: dict, timeout: int):
        assert url == "http://127.0.0.1:8081/v1/embeddings"
        assert json["model"] == "bge-m3"
        assert json["input"] == "hello"
        assert timeout > 0
        return _Response(200, payload={"data": [{"embedding": [0.1, 0.2, 0.3]}]})

    monkeypatch.setattr("requests.post", fake_post)

    vector = provider.embed("hello", model="bge-m3")

    assert isinstance(vector, np.ndarray)
    assert vector.tolist() == pytest.approx([0.1, 0.2, 0.3])


def test_llamacpp_embedding_provider_requires_model() -> None:
    provider = LlamaCppEmbeddingProvider()
    with pytest.raises(ProviderConfigurationError):
        provider.embed("hello", model="")


def test_llamacpp_embedding_provider_raises_for_invalid_payload(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = LlamaCppEmbeddingProvider()

    def fake_post(url: str, json: dict, timeout: int):
        return _Response(200, payload={"data": []})

    monkeypatch.setattr("requests.post", fake_post)

    with pytest.raises(ProviderRequestError):
        provider.embed("hello", model="bge-m3")
