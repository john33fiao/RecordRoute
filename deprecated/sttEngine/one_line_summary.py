"""Utility for generating one-line summaries using llama.cpp."""

from pathlib import Path
from workflow.summarize import read_text_with_fallback, DEFAULT_MODEL
from llamacpp_utils import generate_text


def generate_one_line_summary(file_path: Path, model: str = None) -> str:
    """Generate a single-line Korean summary for the given text file.

    Args:
        file_path: Path to the text file to summarize.
        model: Optional GGUF model filename to use. Defaults to the
            structured summary model when not provided.

    Returns:
        A one-line summary string.
    """
    text = read_text_with_fallback(file_path)
    prompt = "다음 텍스트를 한 줄로 한국어로 요약해 주세요:\n" + text[:4000]
    response = generate_text(
        model_filename=model or DEFAULT_MODEL,
        prompt=prompt,
        temperature=0.0,
        max_tokens=100,
        stream=False
    )
    return response.strip().splitlines()[0]
