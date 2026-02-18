from __future__ import annotations

import os
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from .providers.base import ProviderError
from .providers.factory import get_llm_provider


class LLMProviderError(RuntimeError):
    """Backward compatible alias for provider call failures."""


@dataclass
class ProviderResolution:
    name: str


def normalize_provider_name(provider_name: Optional[str] = None) -> str:
    value = (provider_name or os.getenv("LLM_PROVIDER") or "ollama").strip().lower()
    aliases = {
        "llama.cpp": "llamacpp",
        "llama_cpp": "llamacpp",
        "llama-cpp": "llamacpp",
    }
    normalized = aliases.get(value, value)
    if normalized not in {"ollama", "llamacpp"}:
        raise LLMProviderError(f"지원하지 않는 LLM provider 입니다: {provider_name or value}")
    return normalized


def check_model_available(model: str, provider_name: Optional[str] = None) -> tuple[bool, str]:
    provider = get_llm_provider(provider_name)
    ok, msg = provider.healthcheck()
    if not ok:
        return ok, msg

    model_list = provider.list_models()
    if not model_list:
        return True, "provider healthcheck 통과 (모델 목록이 비어 있습니다)"

    if model in model_list:
        return True, f"모델 확인 완료: {model}"

    normalized = normalize_provider_name(provider_name)
    if normalized == "llamacpp":
        if model and ("/" in model or "\\" in model or model.endswith(".gguf")):
            return True, f"llama.cpp 모델 경로 확인 완료: {model}"
    return False, f"모델을 찾을 수 없습니다: {model}"


def chat_completion(
    *,
    model: str,
    messages: List[Dict[str, str]],
    options: Optional[Dict[str, Any]] = None,
    provider_name: Optional[str] = None,
    timeout: Optional[int] = None,
) -> Dict[str, Any]:
    provider = get_llm_provider(provider_name)
    try:
        return provider.chat(model=model, messages=messages, options=options or {}, timeout=timeout)
    except ProviderError as exc:
        raise LLMProviderError(str(exc)) from exc
