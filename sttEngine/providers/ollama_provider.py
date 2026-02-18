from __future__ import annotations

from typing import Any, Dict, List, Optional, Sequence

import numpy as np

from ..ollama_utils import ensure_ollama_server, safe_ollama_call
from .base import ProviderRequestError
from .embedding_provider import BaseEmbeddingProvider
from .llm_provider import BaseLLMProvider


class OllamaLLMProvider(BaseLLMProvider):
    def _load_ollama(self):
        try:
            import ollama
        except ImportError as exc:
            raise ProviderRequestError("ollama 패키지가 설치되지 않았습니다.") from exc
        return ollama

    def chat(
        self,
        *,
        model: str,
        messages: List[Dict[str, str]],
        options: Optional[Dict[str, Any]] = None,
        timeout: Optional[int] = None,
    ) -> Dict[str, Any]:
        _ = timeout
        ollama = self._load_ollama()
        return safe_ollama_call(
            ollama.chat,
            model=model,
            messages=messages,
            options=options or {},
            stream=False,
        )

    def generate(
        self,
        *,
        model: str,
        prompt: str,
        options: Optional[Dict[str, Any]] = None,
        timeout: Optional[int] = None,
    ) -> Dict[str, Any]:
        _ = timeout
        ollama = self._load_ollama()
        return safe_ollama_call(
            ollama.generate,
            model=model,
            prompt=prompt,
            options=options or {},
            stream=False,
        )

    def list_models(self) -> List[str]:
        ollama = self._load_ollama()
        models = safe_ollama_call(ollama.list)
        items = models.get("models", []) if isinstance(models, dict) else []
        names: list[str] = []
        for model in items:
            if isinstance(model, dict):
                name = model.get("name")
                if name:
                    names.append(name)
        return names

    def healthcheck(self) -> tuple[bool, str]:
        return ensure_ollama_server()


class OllamaEmbeddingProvider(BaseEmbeddingProvider):
    def _load_ollama(self):
        try:
            import ollama
        except ImportError as exc:
            raise ProviderRequestError("ollama 패키지가 설치되지 않았습니다.") from exc
        return ollama

    def embed(self, text: str, *, model: str) -> np.ndarray:
        ollama = self._load_ollama()
        response = safe_ollama_call(ollama.embeddings, model=model, prompt=text)
        embedding = response.get("embedding") if isinstance(response, dict) else None
        if not embedding:
            raise ProviderRequestError("Ollama 임베딩 응답이 비어 있습니다.")
        return np.array(embedding, dtype=np.float32)

    def embed_batch(self, texts: Sequence[str], *, model: str) -> list[np.ndarray]:
        return [self.embed(text, model=model) for text in texts]

    def healthcheck(self) -> tuple[bool, str]:
        return ensure_ollama_server()
