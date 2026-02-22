from __future__ import annotations

import logging
import os
from pathlib import Path
from threading import Lock
from typing import Any, Dict, List, Optional, Sequence

import numpy as np
from huggingface_hub import hf_hub_download
from huggingface_hub.utils import HfHubHTTPError

from .base import ProviderConfigurationError, ProviderRequestError
from .embedding_provider import BaseEmbeddingProvider
from .llm_provider import BaseLLMProvider

LOGGER = logging.getLogger(__name__)
_DEFAULT_MODEL_PATH = "./models/default_model.gguf"
_DEFAULT_MODEL_CACHE_DIR = "./models"
_HTTP_ENV_KEYS = (
    "LLM_BASE_URL",
    "EMBEDDING_BASE_URL",
    "LLAMA_CPP_COMMAND",
    "LLM_TIMEOUT",
    "EMBEDDING_TIMEOUT",
    "LLAMA_CPP_TIMEOUT",
)
_WARNED_HTTP_ENV = False


def _resolve_hf_cache_dir() -> str:
    cache_dir = os.getenv("LLAMA_CPP_MODEL_CACHE_DIR", _DEFAULT_MODEL_CACHE_DIR)
    return os.path.abspath(cache_dir)


def _parse_hf_model_source(model: str) -> tuple[str, str, Optional[str]]:
    """Parse HF source from env or a model string.

    Supported model string formats:
    - hf://<repo_id>/<filename>.gguf
    - hf:<repo_id>:<filename>.gguf
    """
    model_value = (model or "").strip()
    revision = (os.getenv("HF_MODEL_REVISION") or "").strip() or None

    if model_value.startswith("hf://"):
        source = model_value[len("hf://") :]
        try:
            repo_id, filename = source.rsplit("/", 1)
        except ValueError as exc:
            raise ProviderConfigurationError(
                "hf:// 모델 지정 형식이 올바르지 않습니다. 예: hf://repo/file.gguf"
            ) from exc
        return repo_id.strip(), filename.strip(), revision

    if model_value.startswith("hf:"):
        parts = [part.strip() for part in model_value.split(":", 2)]
        if len(parts) != 3 or not parts[1] or not parts[2]:
            raise ProviderConfigurationError(
                "hf: 모델 지정 형식이 올바르지 않습니다. 예: hf:repo:file.gguf"
            )
        return parts[1], parts[2], revision

    repo_id = (os.getenv("HF_MODEL_REPO_ID") or "").strip()
    filename = (os.getenv("HF_MODEL_FILENAME") or "").strip()
    return repo_id, filename, revision


def _ensure_model_file(model_path: str, model: str) -> str:
    """Return absolute path to local GGUF model, downloading from HF if needed."""
    absolute_model_path = os.path.abspath(model_path)
    if os.path.exists(absolute_model_path):
        return absolute_model_path

    repo_id, filename, revision = _parse_hf_model_source(model)
    if not repo_id or not filename:
        raise ProviderConfigurationError(
            "llama.cpp 모델 파일을 찾을 수 없습니다. 로컬 파일을 준비하거나 "
            "HF_MODEL_REPO_ID/HF_MODEL_FILENAME(또는 hf://... 형식 model)를 설정하세요. "
            f"경로: {absolute_model_path}"
        )

    hf_token = os.environ.get("HF_TOKEN")
    if not hf_token:
        raise ProviderConfigurationError(
            "HF_TOKEN 환경 변수가 설정되지 않았습니다. Hugging Face 다운로드에 필요한 토큰을 설정하세요."
        )

    cache_dir = _resolve_hf_cache_dir()
    Path(cache_dir).mkdir(parents=True, exist_ok=True)
    LOGGER.info(
        "llama.cpp 모델이 없어 Hugging Face에서 다운로드를 시도합니다: repo=%s, file=%s, revision=%s, cache_dir=%s",
        repo_id,
        filename,
        revision or "<default>",
        cache_dir,
    )

    try:
        downloaded_path = hf_hub_download(
            repo_id=repo_id,
            filename=filename,
            revision=revision,
            token=hf_token,
            cache_dir=cache_dir,
            local_dir=cache_dir,
            local_dir_use_symlinks=False,
        )
    except HfHubHTTPError as exc:
        status_code = getattr(getattr(exc, "response", None), "status_code", None)
        if status_code == 403:
            LOGGER.error(
                "Hugging Face 모델 다운로드 403 Forbidden: 토큰 권한 부족 또는 gated 모델 라이선스 미동의 가능성이 큽니다. "
                "repo=%s file=%s",
                repo_id,
                filename,
            )
            raise RuntimeError(
                "Hugging Face 403 Forbidden: HF_TOKEN 권한 또는 모델 라이선스 동의를 확인하세요."
            ) from exc

        LOGGER.error(
            "Hugging Face 모델 다운로드 실패(HTTP %s): repo=%s file=%s error=%s",
            status_code,
            repo_id,
            filename,
            exc,
        )
        raise RuntimeError("Hugging Face 모델 다운로드에 실패했습니다.") from exc
    except Exception as exc:
        LOGGER.error(
            "Hugging Face 모델 다운로드 중 예외가 발생했습니다: repo=%s file=%s error=%s",
            repo_id,
            filename,
            exc,
        )
        raise RuntimeError("Hugging Face 모델 다운로드 중 예외가 발생했습니다.") from exc

    resolved_path = os.path.abspath(downloaded_path)
    LOGGER.info("Hugging Face 모델 다운로드 완료: %s", resolved_path)
    return resolved_path


class _LlamaInstanceManager:
    """Process-wide llama.cpp model cache keyed by absolute model path."""

    _instances: dict[str, Any] = {}
    _lock = Lock()

    @classmethod
    def get_or_create(cls, model_path: str, *, model: str = "") -> Any:
        normalized_path = _ensure_model_file(model_path, model)

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
        normalized_model = (model or "").strip()
        if normalized_model.startswith("hf://") or normalized_model.startswith("hf:"):
            _, filename, _ = _parse_hf_model_source(normalized_model)
            target_filename = filename or (os.getenv("HF_MODEL_FILENAME") or "model.gguf")
            return os.path.join(_resolve_hf_cache_dir(), target_filename)
        if normalized_model and (
            "/" in normalized_model
            or "\\" in normalized_model
            or normalized_model.endswith(".gguf")
        ):
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
        _warn_if_http_env_is_ignored()
        return _LlamaInstanceManager.get_or_create(model_path, model=model)


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

        repo_id, filename, _ = _parse_hf_model_source("")
        if repo_id and filename:
            return True, (
                "llama.cpp 모델 로컬 파일이 없지만 Hugging Face 자동 다운로드 설정이 확인되었습니다: "
                f"repo={repo_id}, file={filename}"
            )

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

        repo_id, filename, _ = _parse_hf_model_source("")
        if repo_id and filename:
            return True, (
                "llama.cpp embedding 모델 로컬 파일이 없지만 Hugging Face 자동 다운로드 설정이 확인되었습니다: "
                f"repo={repo_id}, file={filename}"
            )

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
