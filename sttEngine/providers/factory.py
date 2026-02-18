from __future__ import annotations

import os

from .base import normalize_provider_name
from .embedding_provider import BaseEmbeddingProvider
from .llama_cpp_provider import LlamaCppEmbeddingProvider, LlamaCppLLMProvider
from .llm_provider import BaseLLMProvider
from .ollama_provider import OllamaEmbeddingProvider, OllamaLLMProvider


def get_llm_provider(provider_name: str | None = None) -> BaseLLMProvider:
    resolved = normalize_provider_name(
        provider_name,
        kind="LLM",
        default=os.getenv("LLM_PROVIDER", "ollama"),
        aliases={
            "llama.cpp": "llamacpp",
            "llama_cpp": "llamacpp",
            "llama-cpp": "llamacpp",
        },
        supported={"ollama", "llamacpp"},
    )

    if resolved == "ollama":
        return OllamaLLMProvider()
    return LlamaCppLLMProvider()


def get_embedding_provider(provider_name: str | None = None) -> BaseEmbeddingProvider:
    resolved = normalize_provider_name(
        provider_name,
        kind="embedding",
        default=os.getenv("EMBEDDING_PROVIDER", os.getenv("LLM_PROVIDER", "ollama")),
        aliases={
            "llama.cpp": "llamacpp",
            "llama_cpp": "llamacpp",
            "llama-cpp": "llamacpp",
        },
        supported={"ollama", "llamacpp"},
    )

    if resolved == "ollama":
        return OllamaEmbeddingProvider()
    return LlamaCppEmbeddingProvider()
