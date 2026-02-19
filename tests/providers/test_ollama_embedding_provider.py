from __future__ import annotations

import numpy as np
import pytest

from sttEngine.providers.base import ProviderRequestError
from sttEngine.providers.ollama_provider import OllamaEmbeddingProvider


class _FakeOllamaWithEmbed:
    @staticmethod
    def embed(*, model: str, input: str) -> dict:
        assert model == "bge-m3"
        assert input == "hello"
        return {"embeddings": [[0.11, 0.22, 0.33]]}


class _FakeOllamaLegacy:
    @staticmethod
    def embeddings(*, model: str, prompt: str) -> dict:
        assert model == "bge-m3"
        assert prompt == "hello"
        return {"embedding": [0.4, 0.5, 0.6]}


def test_ollama_embedding_provider_supports_embed_api(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = OllamaEmbeddingProvider()

    monkeypatch.setattr(provider, "_load_ollama", lambda: _FakeOllamaWithEmbed)
    monkeypatch.setattr(
        "sttEngine.providers.ollama_provider.safe_ollama_call",
        lambda func, *args, **kwargs: func(*args, **kwargs),
    )

    vector = provider.embed("hello", model="bge-m3")

    assert isinstance(vector, np.ndarray)
    assert vector.tolist() == pytest.approx([0.11, 0.22, 0.33])


def test_ollama_embedding_provider_falls_back_to_legacy_embeddings_api(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = OllamaEmbeddingProvider()

    monkeypatch.setattr(provider, "_load_ollama", lambda: _FakeOllamaLegacy)
    monkeypatch.setattr(
        "sttEngine.providers.ollama_provider.safe_ollama_call",
        lambda func, *args, **kwargs: func(*args, **kwargs),
    )

    vector = provider.embed("hello", model="bge-m3")

    assert isinstance(vector, np.ndarray)
    assert vector.tolist() == pytest.approx([0.4, 0.5, 0.6])




def test_ollama_embedding_provider_supports_flat_embeddings_shape(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = OllamaEmbeddingProvider()

    class _FakeOllamaFlat:
        @staticmethod
        def embed(*, model: str, input: str) -> dict:
            return {"embeddings": [0.7, 0.8, 0.9]}

    monkeypatch.setattr(provider, "_load_ollama", lambda: _FakeOllamaFlat)
    monkeypatch.setattr(
        "sttEngine.providers.ollama_provider.safe_ollama_call",
        lambda func, *args, **kwargs: func(*args, **kwargs),
    )

    vector = provider.embed("hello", model="bge-m3")

    assert isinstance(vector, np.ndarray)
    assert vector.tolist() == pytest.approx([0.7, 0.8, 0.9])


def test_ollama_embedding_provider_raises_when_embedding_api_missing(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = OllamaEmbeddingProvider()

    class _FakeOllamaNoEmbeddingApi:
        pass

    monkeypatch.setattr(provider, "_load_ollama", lambda: _FakeOllamaNoEmbeddingApi)

    with pytest.raises(ProviderRequestError, match="임베딩 API를 찾을 수 없습니다"):
        provider.embed("hello", model="bge-m3")

def test_ollama_embedding_provider_raises_for_empty_payload(monkeypatch: pytest.MonkeyPatch) -> None:
    provider = OllamaEmbeddingProvider()

    class _FakeOllamaEmpty:
        @staticmethod
        def embed(*, model: str, input: str) -> dict:
            return {"embeddings": []}

    monkeypatch.setattr(provider, "_load_ollama", lambda: _FakeOllamaEmpty)
    monkeypatch.setattr(
        "sttEngine.providers.ollama_provider.safe_ollama_call",
        lambda func, *args, **kwargs: func(*args, **kwargs),
    )

    with pytest.raises(ProviderRequestError, match="임베딩 응답이 비어 있습니다"):
        provider.embed("hello", model="bge-m3")
