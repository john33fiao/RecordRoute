from __future__ import annotations

from pathlib import Path

from sttEngine.one_line_summary import _extract_summary_line, generate_one_line_summary


class _DummyProvider:
    def __init__(self, payload):
        self.payload = payload
        self.calls = []

    def generate(self, *, model: str, prompt: str, options: dict):
        self.calls.append({"model": model, "prompt": prompt, "options": options})
        return self.payload


def test_extract_summary_line_supports_generate_response() -> None:
    assert _extract_summary_line({"response": "한 줄 요약\n두번째 줄"}) == "한 줄 요약"


def test_extract_summary_line_supports_chat_shape() -> None:
    assert _extract_summary_line({"message": {"content": "요약 결과"}}) == "요약 결과"


def test_generate_one_line_summary_uses_provider_contract(monkeypatch, tmp_path: Path) -> None:
    source = tmp_path / "sample.txt"
    source.write_text("테스트 본문", encoding="utf-8")

    provider = _DummyProvider({"response": "요약 한 줄"})
    monkeypatch.setattr("sttEngine.one_line_summary.get_llm_provider", lambda *_args, **_kwargs: provider)

    summary = generate_one_line_summary(source, model="demo", provider_name="llamacpp")

    assert summary == "요약 한 줄"
    assert provider.calls
    assert provider.calls[0]["model"] == "demo"
    assert provider.calls[0]["options"]["temperature"] == 0
