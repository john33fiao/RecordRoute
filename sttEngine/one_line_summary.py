"""Utility for generating one-line summaries using Ollama."""

from pathlib import Path
import ollama
from sttEngine.workflow.summarize import read_text_with_fallback, DEFAULT_MODEL
from ollama_utils import safe_ollama_call


def generate_one_line_summary(file_path: Path) -> str:
    """Generate a single-line Korean summary for the given text file.

    Args:
        file_path: Path to the text file to summarize.

    Returns:
        A one-line summary string.
    """
    text = read_text_with_fallback(file_path)
    prompt = "다음 텍스트를 한 줄로 한국어로 요약해 주세요:\n" + text[:4000]
    response = safe_ollama_call(
        ollama.generate,
        model=DEFAULT_MODEL,
        prompt=prompt,
        options={"temperature": 0},
    )
    return response.get("response", "").strip().splitlines()[0]
