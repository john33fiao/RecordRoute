from __future__ import annotations

from collections.abc import Mapping
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
        response = self._request_embedding(ollama, text=text, model=model)
        embedding = self._extract_embedding(response)
        if not embedding:
            raise ProviderRequestError(
                f"Ollama 임베딩 응답이 비어 있습니다. 모델 '{model}'이 임베딩을 지원하는지 확인하세요."
            )
        return np.array(embedding, dtype=np.float32)

    def _request_embedding(self, ollama: Any, *, text: str, model: str) -> Any:
        if hasattr(ollama, "embed"):
            return safe_ollama_call(ollama.embed, model=model, input=text)
        if hasattr(ollama, "embeddings"):
            return safe_ollama_call(ollama.embeddings, model=model, prompt=text)
        raise ProviderRequestError("현재 ollama 클라이언트에서 임베딩 API를 찾을 수 없습니다.")

    @staticmethod
    def _extract_embedding(response: Any) -> list[float] | None:
        payload: Any = response
        if hasattr(response, "model_dump"):
            payload = response.model_dump()

        if isinstance(payload, Mapping):
            payload = dict(payload)
        elif not isinstance(payload, dict):
            payload = {
                key: getattr(payload, key)
                for key in ("embedding", "embeddings", "data")
                if hasattr(payload, key)
            }

        if not payload:
            return None

        direct = payload.get("embedding")
        if isinstance(direct, list) and direct:
            return direct

        many = payload.get("embeddings")
        if isinstance(many, list) and many:
            first = many[0]
            if isinstance(first, list) and first:
                return first
            if isinstance(first, (int, float)):
                return many

        data = payload.get("data")
        if isinstance(data, list) and data:
            first = data[0]
            if isinstance(first, dict):
                vec = first.get("embedding")
                if isinstance(vec, list) and vec:
                    return vec
        return None

    def embed_batch(self, texts: Sequence[str], *, model: str) -> list[np.ndarray]:
        return [self.embed(text, model=model) for text in texts]

    def healthcheck(self) -> tuple[bool, str]:
        return ensure_ollama_server()
