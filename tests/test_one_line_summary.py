from __future__ import annotations

from pathlib import Path

import pytest

from sttEngine.one_line_summary import (
    OneLineSummaryError,
    _extract_summary_line,
    build_one_line_summary_prompt,
    generate_one_line_summary,
)


def test_extract_summary_line_supports_generate_response() -> None:
    assert _extract_summary_line({"response": "한 줄 요약\n두번째 줄"}) == "한 줄 요약"


def test_extract_summary_line_supports_chat_shape() -> None:
    assert _extract_summary_line({"message": {"content": "요약 결과"}}) == "요약 결과"


def test_extract_summary_line_normalizes_prefixes_and_bullets() -> None:
    assert _extract_summary_line({"message": {"content": "- 요약: 핵심 논의 정리"}}) == "핵심 논의 정리"


def test_build_one_line_summary_prompt_contains_ui_constraints() -> None:
    prompt = build_one_line_summary_prompt("긴 본문")

    assert "반드시 한국어 한 줄만 출력합니다" in prompt
    assert "30~60자 내외" in prompt
    assert prompt.endswith("긴 본문")


def test_generate_one_line_summary_uses_chat_completion_contract(monkeypatch, tmp_path: Path) -> None:
    source = tmp_path / "sample.txt"
    source.write_text("테스트 본문", encoding="utf-8")

    called: dict[str, object] = {}

    def fake_chat_completion(**kwargs):
        called.update(kwargs)
        return {"message": {"content": "요약 한 줄"}}

    monkeypatch.setattr("sttEngine.one_line_summary.chat_completion", fake_chat_completion)

    summary = generate_one_line_summary(source, model="demo", provider_name="llamacpp")

    assert summary == "요약 한 줄"
    assert called["model"] == "demo"
    assert called["provider_name"] == "llamacpp"
    assert called["options"]["temperature"] == 0
    assert called["options"]["num_predict"] == 120


def test_generate_one_line_summary_raises_for_empty_response(monkeypatch, tmp_path: Path) -> None:
    source = tmp_path / "sample.txt"
    source.write_text("테스트 본문", encoding="utf-8")

    monkeypatch.setattr(
        "sttEngine.one_line_summary.chat_completion",
        lambda **_kwargs: {"message": {"content": "   \n  "}},
    )

    with pytest.raises(OneLineSummaryError):
        generate_one_line_summary(source, model="demo", provider_name="ollama")
