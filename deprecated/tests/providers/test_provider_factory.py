from __future__ import annotations

import pytest

from sttEngine.providers.base import ProviderConfigurationError
from sttEngine.providers.factory import get_embedding_provider, get_llm_provider
from sttEngine.providers.llama_cpp_provider import LlamaCppEmbeddingProvider, LlamaCppLLMProvider
from sttEngine.providers.ollama_provider import OllamaEmbeddingProvider, OllamaLLMProvider


def test_get_llm_provider_ollama_alias() -> None:
    provider = get_llm_provider("ollama")
    assert isinstance(provider, OllamaLLMProvider)


@pytest.mark.parametrize("alias", ["llamacpp", "llama_cpp", "llama.cpp", "llama-cpp"])
def test_get_llm_provider_llamacpp_aliases(alias: str) -> None:
    provider = get_llm_provider(alias)
    assert isinstance(provider, LlamaCppLLMProvider)


def test_get_embedding_provider_defaults_to_llm_provider(monkeypatch: pytest.MonkeyPatch) -> None:
    monkeypatch.setenv("LLM_PROVIDER", "llama_cpp")
    monkeypatch.delenv("EMBEDDING_PROVIDER", raising=False)
    provider = get_embedding_provider()
    assert isinstance(provider, LlamaCppEmbeddingProvider)


def test_get_embedding_provider_ollama() -> None:
    provider = get_embedding_provider("ollama")
    assert isinstance(provider, OllamaEmbeddingProvider)


def test_get_llm_provider_invalid() -> None:
    with pytest.raises(ProviderConfigurationError):
        get_llm_provider("unknown")
