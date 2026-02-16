from __future__ import annotations

import logging
import os
import subprocess
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from .ollama_utils import check_ollama_model_available, safe_ollama_call


class LLMProviderError(RuntimeError):
    """Raised when an LLM provider call fails."""


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
    provider = normalize_provider_name(provider_name)

    if provider == "ollama":
        return check_ollama_model_available(model)

    model_path = _resolve_llamacpp_model_path(model)
    if not model_path:
        return False, "llama.cpp provider는 모델 경로(LLAMA_CPP_MODEL_PATH 또는 --model 경로)가 필요합니다."
    if not os.path.exists(model_path):
        return False, f"llama.cpp 모델 파일을 찾을 수 없습니다: {model_path}"
    return True, f"llama.cpp 모델 확인 완료: {model_path}"


def chat_completion(
    *,
    model: str,
    messages: List[Dict[str, str]],
    options: Optional[Dict[str, Any]] = None,
    provider_name: Optional[str] = None,
    timeout: Optional[int] = None,
) -> Dict[str, Any]:
    provider = normalize_provider_name(provider_name)
    options = options or {}

    if provider == "ollama":
        return _chat_with_ollama(model=model, messages=messages, options=options)
    return _chat_with_llamacpp(model=model, messages=messages, options=options, timeout=timeout)


def _chat_with_ollama(*, model: str, messages: List[Dict[str, str]], options: Dict[str, Any]) -> Dict[str, Any]:
    try:
        import ollama
    except ImportError as exc:
        raise LLMProviderError("ollama 패키지가 설치되어 있지 않습니다.") from exc

    return safe_ollama_call(
        ollama.chat,
        model=model,
        messages=messages,
        options=options,
        stream=False,
    )


def _resolve_llamacpp_model_path(model: str) -> str:
    if model and ("/" in model or "\\" in model or model.endswith(".gguf")):
        return model
    return os.getenv("LLAMA_CPP_MODEL_PATH", "")


def _messages_to_prompt(messages: List[Dict[str, str]]) -> str:
    ordered = []
    for message in messages:
        role = (message.get("role") or "user").upper()
        content = message.get("content") or ""
        ordered.append(f"[{role}]\n{content}")
    return "\n\n".join(ordered).strip()


def _chat_with_llamacpp(
    *, model: str, messages: List[Dict[str, str]], options: Dict[str, Any], timeout: Optional[int]
) -> Dict[str, Any]:
    command = os.getenv("LLAMA_CPP_COMMAND", "llama-cli")
    model_path = _resolve_llamacpp_model_path(model)
    if not model_path:
        raise LLMProviderError("llama.cpp provider는 모델 경로(LLAMA_CPP_MODEL_PATH 또는 model 경로)가 필요합니다.")

    prompt = _messages_to_prompt(messages)
    if not prompt:
        raise LLMProviderError("llama.cpp 호출용 prompt가 비어 있습니다.")

    args = [command, "-m", model_path, "-p", prompt, "--no-display-prompt"]

    num_ctx = options.get("num_ctx")
    if num_ctx:
        args.extend(["--ctx-size", str(num_ctx)])

    temperature = options.get("temperature")
    if temperature is not None:
        args.extend(["--temp", str(temperature)])

    num_predict = options.get("num_predict")
    if num_predict:
        args.extend(["--n-predict", str(num_predict)])

    timeout_seconds = timeout if timeout is not None else int(os.getenv("LLAMA_CPP_TIMEOUT", "300"))

    try:
        result = subprocess.run(args, capture_output=True, text=True, check=False, timeout=timeout_seconds)
    except FileNotFoundError as exc:
        raise LLMProviderError(f"llama.cpp 실행 파일을 찾을 수 없습니다: {command}") from exc
    except subprocess.TimeoutExpired as exc:
        raise LLMProviderError(f"llama.cpp 호출 타임아웃 ({timeout_seconds}초)") from exc

    if result.returncode != 0:
        stderr = (result.stderr or "").strip()
        raise LLMProviderError(f"llama.cpp 호출 실패(returncode={result.returncode}): {stderr}")

    content = (result.stdout or "").strip()
    if not content:
        logging.warning("llama.cpp 응답(stdout)이 비어 있어 stderr를 사용합니다.")
        content = (result.stderr or "").strip()

    if not content:
        raise LLMProviderError("llama.cpp 응답이 비어 있습니다.")

    return {"message": {"content": content}}
