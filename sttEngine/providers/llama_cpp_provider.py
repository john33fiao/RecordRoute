from __future__ import annotations

import os
import subprocess
from typing import Any, Dict, List, Optional, Sequence

import numpy as np

from .base import ProviderConfigurationError, ProviderRequestError
from .embedding_provider import BaseEmbeddingProvider
from .llm_provider import BaseLLMProvider


class LlamaCppLLMProvider(BaseLLMProvider):
    def _resolve_model_path(self, model: str) -> str:
        if model and ("/" in model or "\\" in model or model.endswith(".gguf")):
            return model
        return os.getenv("LLAMA_CPP_MODEL_PATH", "")

    def _messages_to_prompt(self, messages: List[Dict[str, str]]) -> str:
        ordered = []
        for message in messages:
            role = (message.get("role") or "user").upper()
            content = message.get("content") or ""
            ordered.append(f"[{role}]\n{content}")
        return "\n\n".join(ordered).strip()

    def _run_cli(self, *, model: str, prompt: str, options: Dict[str, Any], timeout: Optional[int]) -> str:
        command = os.getenv("LLAMA_CPP_COMMAND", "llama-cli")
        model_path = self._resolve_model_path(model)
        if not model_path:
            raise ProviderConfigurationError(
                "llama.cpp provider는 모델 경로(LLAMA_CPP_MODEL_PATH 또는 model 경로)가 필요합니다."
            )

        args = [command, "-m", model_path, "-p", prompt, "--no-display-prompt"]
        if options.get("num_ctx"):
            args.extend(["--ctx-size", str(options["num_ctx"])])
        if options.get("temperature") is not None:
            args.extend(["--temp", str(options["temperature"])])
        if options.get("num_predict"):
            args.extend(["--n-predict", str(options["num_predict"])])

        timeout_seconds = timeout if timeout is not None else int(os.getenv("LLAMA_CPP_TIMEOUT", "300"))

        try:
            result = subprocess.run(args, capture_output=True, text=True, check=False, timeout=timeout_seconds)
        except FileNotFoundError as exc:
            raise ProviderRequestError(f"llama.cpp 실행 파일을 찾을 수 없습니다: {command}") from exc
        except subprocess.TimeoutExpired as exc:
            raise ProviderRequestError(f"llama.cpp 호출 타임아웃 ({timeout_seconds}초)") from exc

        if result.returncode != 0:
            raise ProviderRequestError(
                f"llama.cpp 호출 실패(returncode={result.returncode}): {(result.stderr or '').strip()}"
            )

        content = (result.stdout or "").strip() or (result.stderr or "").strip()
        if not content:
            raise ProviderRequestError("llama.cpp 응답이 비어 있습니다.")
        return content

    def chat(
        self,
        *,
        model: str,
        messages: List[Dict[str, str]],
        options: Optional[Dict[str, Any]] = None,
        timeout: Optional[int] = None,
    ) -> Dict[str, Any]:
        prompt = self._messages_to_prompt(messages)
        if not prompt:
            raise ProviderRequestError("llama.cpp 호출용 prompt가 비어 있습니다.")
        content = self._run_cli(model=model, prompt=prompt, options=options or {}, timeout=timeout)
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
        content = self._run_cli(model=model, prompt=prompt, options=options or {}, timeout=timeout)
        return {"response": content}

    def list_models(self) -> List[str]:
        model_path = os.getenv("LLAMA_CPP_MODEL_PATH", "")
        return [model_path] if model_path else []

    def healthcheck(self) -> tuple[bool, str]:
        command = os.getenv("LLAMA_CPP_COMMAND", "llama-cli")
        result = subprocess.run(["bash", "-lc", f"command -v {command}"], capture_output=True, text=True, check=False)
        if result.returncode != 0:
            return False, f"llama.cpp 실행 파일을 찾을 수 없습니다: {command}"
        return True, f"llama.cpp 실행 파일 확인 완료: {command}"


class LlamaCppEmbeddingProvider(BaseEmbeddingProvider):
    def embed(self, text: str, *, model: str) -> np.ndarray:
        _ = text
        _ = model
        raise ProviderRequestError("llama.cpp embedding provider는 아직 구현되지 않았습니다. (Phase 1 skeleton)")

    def embed_batch(self, texts: Sequence[str], *, model: str) -> list[np.ndarray]:
        return [self.embed(text, model=model) for text in texts]

    def healthcheck(self) -> tuple[bool, str]:
        return LlamaCppLLMProvider().healthcheck()
