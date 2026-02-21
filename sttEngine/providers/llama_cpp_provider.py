from __future__ import annotations

import logging
import os
from threading import Lock
from typing import Any, Dict, List, Optional, Sequence

import numpy as np

from .base import ProviderConfigurationError, ProviderRequestError
from .embedding_provider import BaseEmbeddingProvider
from .llm_provider import BaseLLMProvider

LOGGER = logging.getLogger(__name__)
_DEFAULT_MODEL_PATH = "./models/default_model.gguf"
_HTTP_ENV_KEYS = (
    "LLM_BASE_URL",
    "EMBEDDING_BASE_URL",
    "LLAMA_CPP_COMMAND",
    "LLM_TIMEOUT",
    "EMBEDDING_TIMEOUT",
    "LLAMA_CPP_TIMEOUT",
)
_WARNED_HTTP_ENV = False


class _LlamaInstanceManager:
    """Process-wide llama.cpp model cache keyed by absolute model path."""

    _instances: dict[str, Any] = {}
    _lock = Lock()

    @classmethod
    def get_or_create(cls, model_path: str) -> Any:
        normalized_path = os.path.abspath(model_path)

        with cls._lock:
            if normalized_path in cls._instances:
                return cls._instances[normalized_path]

            llama_cls = cls._resolve_llama_class()
            kwargs = cls._build_llama_kwargs()
            try:
                instance = llama_cls(model_path=normalized_path, embedding=True, **kwargs)
            except Exception as exc:  # pragma: no cover - runtime backend errors
                raise ProviderRequestError(
                    f"llama.cpp 모델 로드에 실패했습니다: {normalized_path} ({exc})"
                ) from exc

            cls._instances[normalized_path] = instance
            LOGGER.info("llama.cpp 모델 로드 완료(in-process): %s", normalized_path)
            return instance

    @classmethod
    def is_loaded(cls, model_path: str) -> bool:
        normalized_path = os.path.abspath(model_path)
        with cls._lock:
            return normalized_path in cls._instances

    @staticmethod
    def _resolve_llama_class() -> Any:
        try:
            from llama_cpp import Llama
        except ImportError as exc:
            raise ProviderConfigurationError(
                "llama-cpp-python 패키지가 설치되어 있지 않습니다. requirements.txt를 확인하세요."
            ) from exc
        return Llama

    @staticmethod
    def _build_llama_kwargs() -> Dict[str, Any]:
        kwargs: Dict[str, Any] = {}

        def _apply_int(env_key: str, target_key: str) -> None:
            value = os.getenv(env_key)
            if value in {None, ""}:
                return
            try:
                kwargs[target_key] = int(value)
            except ValueError as exc:
                raise ProviderConfigurationError(
                    f"환경 변수 {env_key}는 정수여야 합니다: {value}"
                ) from exc

        _apply_int("LLAMA_CPP_N_CTX", "n_ctx")
        _apply_int("LLAMA_CPP_N_THREADS", "n_threads")
        _apply_int("LLAMA_CPP_N_BATCH", "n_batch")
        _apply_int("LLAMA_CPP_N_GPU_LAYERS", "n_gpu_layers")

        chat_format = os.getenv("LLAMA_CPP_CHAT_FORMAT")
        if chat_format:
            kwargs["chat_format"] = chat_format

        return kwargs


def _warn_if_http_env_is_ignored() -> None:
    global _WARNED_HTTP_ENV
    if _WARNED_HTTP_ENV:
        return

    configured_keys = [key for key in _HTTP_ENV_KEYS if os.getenv(key)]
    if configured_keys:
        LOGGER.warning(
            "llama.cpp in-process 모드에서는 HTTP 환경 변수를 무시합니다: %s",
            ", ".join(configured_keys),
        )

    _WARNED_HTTP_ENV = True


class _LlamaCppProviderMixin:
    def _resolve_model_path(self, model: str) -> str:
        if model and ("/" in model or "\\" in model or model.endswith(".gguf")):
            return model
        return os.getenv("LLAMA_CPP_MODEL_PATH", _DEFAULT_MODEL_PATH)

    def _get_model_path_or_raise(self, model: str) -> str:
        model_path = (self._resolve_model_path(model) or "").strip()
        if not model_path:
            raise ProviderConfigurationError(
                "llama.cpp provider는 모델 경로(LLAMA_CPP_MODEL_PATH 또는 model 경로)가 필요합니다."
            )
        return model_path

    def _get_llama_or_raise(self, model: str) -> Any:
        model_path = self._get_model_path_or_raise(model)
        if not os.path.exists(model_path):
            raise ProviderConfigurationError(f"llama.cpp 모델 파일을 찾을 수 없습니다: {model_path}")
        _warn_if_http_env_is_ignored()
        return _LlamaInstanceManager.get_or_create(model_path)


class LlamaCppLLMProvider(_LlamaCppProviderMixin, BaseLLMProvider):
    def chat(
        self,
        *,
        model: str,
        messages: List[Dict[str, str]],
        options: Optional[Dict[str, Any]] = None,
        timeout: Optional[int] = None,
    ) -> Dict[str, Any]:
        if not messages:
            raise ProviderRequestError("llama.cpp 호출용 messages가 비어 있습니다.")

        if timeout is not None:
            LOGGER.warning("llama.cpp in-process 모드에서는 timeout 인자를 무시합니다.")

        llama = self._get_llama_or_raise(model)
        opts = options or {}

        try:
            response = llama.create_chat_completion(
                messages=messages,
                temperature=opts.get("temperature", 0.0),
                max_tokens=opts.get("num_predict"),
                top_p=opts.get("top_p"),
            )
        except Exception as exc:  # pragma: no cover - runtime backend errors
            raise ProviderRequestError(f"llama.cpp chat 호출 실패: {exc}") from exc

        content = self._extract_chat_content(response)
        if not content:
            raise ProviderRequestError("llama.cpp chat 응답이 비어 있습니다.")
        return {"message": {"content": content}}

    def generate(
        self,
        *,
        model: str,
        prompt: str,
        options: Optional[Dict[str, Any]] = None,
        timeout: Optional[int] = None,
    ) -> Dict[str, Any]:
        if not prompt.strip():
            raise ProviderRequestError("llama.cpp 호출용 prompt가 비어 있습니다.")

        if timeout is not None:
            LOGGER.warning("llama.cpp in-process 모드에서는 timeout 인자를 무시합니다.")

        llama = self._get_llama_or_raise(model)
        opts = options or {}

        try:
            response = llama(
                prompt,
                max_tokens=opts.get("num_predict"),
                temperature=opts.get("temperature", 0.0),
                top_p=opts.get("top_p"),
            )
        except Exception as exc:  # pragma: no cover - runtime backend errors
            raise ProviderRequestError(f"llama.cpp generate 호출 실패: {exc}") from exc

        content = self._extract_generate_content(response)
        if not content:
            raise ProviderRequestError("llama.cpp generate 응답이 비어 있습니다.")
        return {"response": content}

    def list_models(self) -> List[str]:
        return [self._get_model_path_or_raise("")]

    def healthcheck(self) -> tuple[bool, str]:
        model_path = self._get_model_path_or_raise("")
        if _LlamaInstanceManager.is_loaded(model_path):
            return True, f"llama.cpp 모델 로드됨(in-process): {model_path}"
        if os.path.exists(model_path):
            return True, f"llama.cpp 모델 파일 확인됨: {model_path}"
        return False, f"llama.cpp 모델 파일이 없습니다: {model_path}"

    @staticmethod
    def _extract_chat_content(response: Any) -> str:
        choices = response.get("choices") if isinstance(response, dict) else None
        if not isinstance(choices, list) or not choices:
            return ""
        message = choices[0].get("message") if isinstance(choices[0], dict) else None
        if isinstance(message, dict):
            return str(message.get("content") or "").strip()
        return ""

    @staticmethod
    def _extract_generate_content(response: Any) -> str:
        choices = response.get("choices") if isinstance(response, dict) else None
        if not isinstance(choices, list) or not choices:
            return ""
        text = choices[0].get("text") if isinstance(choices[0], dict) else None
        return str(text or "").strip()


class LlamaCppEmbeddingProvider(_LlamaCppProviderMixin, BaseEmbeddingProvider):
    def embed(self, text: str, *, model: str) -> np.ndarray:
        prompt = (text or "").strip()
        if not prompt:
            raise ProviderRequestError("llama.cpp embedding 호출용 텍스트가 비어 있습니다.")

        llama = self._get_llama_or_raise(model)

        try:
            response = llama.create_embedding(prompt)
        except Exception as exc:  # pragma: no cover - runtime backend errors
            raise ProviderRequestError(f"llama.cpp embedding 호출 실패: {exc}") from exc

        vector = self._extract_embedding(response)
        if vector.size == 0:
            raise ProviderRequestError("llama.cpp embedding 벡터가 비어 있습니다.")
        return vector

    def embed_batch(self, texts: Sequence[str], *, model: str) -> list[np.ndarray]:
        return [self.embed(text, model=model) for text in texts]

    def healthcheck(self) -> tuple[bool, str]:
        model_path = self._get_model_path_or_raise("")
        if _LlamaInstanceManager.is_loaded(model_path):
            return True, f"llama.cpp embedding 모델 로드됨(in-process): {model_path}"
        if os.path.exists(model_path):
            return True, f"llama.cpp embedding 모델 파일 확인됨: {model_path}"
        return False, f"llama.cpp embedding 모델 파일이 없습니다: {model_path}"

    @staticmethod
    def _extract_embedding(response: Any) -> np.ndarray:
        if isinstance(response, dict):
            data = response.get("data")
            if isinstance(data, list) and data and isinstance(data[0], dict):
                embedding = data[0].get("embedding")
                if isinstance(embedding, list):
                    return np.array(embedding, dtype=np.float32)

        if isinstance(response, list):
            return np.array(response, dtype=np.float32)

        return np.array([], dtype=np.float32)
