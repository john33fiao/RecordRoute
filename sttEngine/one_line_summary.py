"""Utility for generating one-line summaries via configured LLM provider."""

from __future__ import annotations

from pathlib import Path
from typing import Any, Dict

from .llm_provider import map_llm_options, normalize_provider_name
from .providers.factory import get_llm_provider
from .workflow.summarize import DEFAULT_MODEL, read_text_with_fallback


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
        stripped = line.strip()
        if stripped:
            return stripped
    return ""


def generate_one_line_summary(file_path: Path, model: str | None = None, provider_name: str | None = None) -> str:
    """Generate a single-line Korean summary for the given text file.

    Args:
        file_path: Path to the text file to summarize.
        model: Optional model name to use. Defaults to the structured summary model.
        provider_name: Optional provider override (`ollama`/`llamacpp`).

    Returns:
        A one-line summary string. Returns empty string when provider response is empty.
    """
    text = read_text_with_fallback(file_path)
    prompt = "다음 텍스트를 한 줄로 한국어로 요약해 주세요:\n" + text[:4000]

    resolved_provider = normalize_provider_name(provider_name)
    provider = get_llm_provider(resolved_provider)
    options = map_llm_options({"temperature": 0}, provider_name=resolved_provider)

    response = provider.generate(
        model=model or DEFAULT_MODEL,
        prompt=prompt,
        options=options,
    )
    return _extract_summary_line(response)
