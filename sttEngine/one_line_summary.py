"""Utility for generating one-line summaries via configured LLM provider."""

from __future__ import annotations

import logging
from pathlib import Path
from typing import Any, Dict

from .llm_provider import chat_completion, map_llm_options, normalize_provider_name
from .workflow.summarize import DEFAULT_MODEL, read_text_with_fallback

LOGGER = logging.getLogger(__name__)
_MAX_SOURCE_CHARS = 4000


class OneLineSummaryError(RuntimeError):
    """Raised when the one-line summary could not be generated."""


def _extract_summary_line(response: Dict[str, Any]) -> str:
    """Extract first non-empty summary line from provider response."""
    if not isinstance(response, dict):
        return ""

    raw_text = response.get("response")
    if not isinstance(raw_text, str):
        message = response.get("message")
        if isinstance(message, dict):
            content = message.get("content")
            if isinstance(content, str):
                raw_text = content

    if not isinstance(raw_text, str):
        return ""

    for line in raw_text.strip().splitlines():
        stripped = line.strip().lstrip("-*•0123456789. ")
        if stripped and not stripped.lower().startswith("요약:"):
            return stripped
        if stripped:
            return stripped.removeprefix("요약:").strip()
    return ""


def build_one_line_summary_prompt(text: str) -> str:
    """Build a compact prompt tailored for list-title summaries."""
    source = (text or "").strip()
    if not source:
        raise OneLineSummaryError("한줄요약 입력 텍스트가 비어 있습니다.")

    return (
        "당신은 문서 목록에 표시할 제목형 한줄요약을 작성합니다.\n"
        "규칙:\n"
        "- 반드시 한국어 한 줄만 출력합니다.\n"
        "- 30~60자 내외로 작성합니다.\n"
        "- 불렛, 번호, 따옴표, 접두어(예: 요약:)를 쓰지 않습니다.\n"
        "- 핵심 주제를 제목처럼 간결하게 표현합니다.\n\n"
        "원문:\n"
        f"{source[:_MAX_SOURCE_CHARS]}"
    )



def generate_one_line_summary(file_path: Path, model: str | None = None, provider_name: str | None = None) -> str:
    """Generate a single-line Korean summary for the given text file.

    Args:
        file_path: Path to the text file to summarize.
        model: Optional model name to use. Defaults to the structured summary model.
        provider_name: Optional provider override (`ollama`/`llamacpp`).

    Returns:
        A one-line summary string.

    Raises:
        OneLineSummaryError: When the provider response is empty or malformed.
    """
    text = read_text_with_fallback(file_path)
    prompt = build_one_line_summary_prompt(text)

    resolved_provider = normalize_provider_name(provider_name)
    options = map_llm_options(
        {
            "temperature": 0,
            "max_tokens": 120,
        },
        provider_name=resolved_provider,
    )

    response = chat_completion(
        model=model or DEFAULT_MODEL,
        messages=[{"role": "user", "content": prompt}],
        options=options,
        provider_name=resolved_provider,
    )
    summary_line = _extract_summary_line(response)
    if not summary_line:
        LOGGER.warning("한줄요약 응답이 비어 있습니다: provider=%s model=%s", resolved_provider, model or DEFAULT_MODEL)
        raise OneLineSummaryError("한줄요약 응답이 비어 있습니다.")
    return summary_line
