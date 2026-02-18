from __future__ import annotations

import pytest

from sttEngine.llm_provider import map_llm_options
from sttEngine.workflow import correct, summarize


def test_map_llm_options_maps_common_keys_for_llamacpp():
    mapped = map_llm_options(
        {"temperature": 0.1, "context_window": 4096, "max_tokens": 256},
        provider_name="llamacpp",
    )

    assert mapped["temperature"] == 0.1
    assert mapped["num_ctx"] == 4096
    assert mapped["num_predict"] == 256


def test_call_llm_with_retry_success(monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setattr(
        summarize,
        "call_llm_with_timeout",
        lambda *_args, **_kwargs: {"message": {"content": "요약 결과"}},
    )

    result = summarize.call_llm_with_retry(model="m", prompt="p", provider_name="llamacpp")

    assert result == "요약 결과"


def test_call_llm_with_retry_failure(monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setattr(summarize, "MAX_RETRIES", 2)

    def _raise(*_args, **_kwargs):
        raise RuntimeError("timeout")

    monkeypatch.setattr(summarize, "call_llm_with_timeout", _raise)

    with pytest.raises(Exception) as exc_info:
        summarize.call_llm_with_retry(model="m", prompt="p", provider_name="llamacpp")

    assert "모든 재시도 실패" in getattr(exc_info.value, "message", "")


def test_validate_model_uses_provider_healthcheck_and_list_models(monkeypatch: pytest.MonkeyPatch):
    class DummyProvider:
        def healthcheck(self):
            return True, "ok"

        def list_models(self):
            return ["model.gguf"]

    monkeypatch.setattr("sttEngine.providers.factory.get_llm_provider", lambda *_args, **_kwargs: DummyProvider())

    assert summarize.validate_model("model.gguf", provider_name="llamacpp") is True
    assert summarize.validate_model("missing-model", provider_name="llamacpp") is False


def test_chat_once_uses_provider_chat(monkeypatch: pytest.MonkeyPatch):
    called = {}

    def _fake_chat_completion(**kwargs):
        called.update(kwargs)
        return {"message": {"content": "교정 결과"}}

    monkeypatch.setattr(correct, "chat_completion", _fake_chat_completion)

    result = correct.chat_once(
        model="model.gguf",
        system="sys",
        user="usr",
        provider_name="llamacpp",
    )

    assert result == "교정 결과"
    assert called["provider_name"] == "llamacpp"
    assert called["options"]["context_window"] == 8192


def test_chat_once_failure_raises_mapped_error(monkeypatch: pytest.MonkeyPatch):
    monkeypatch.setattr(correct, "chat_completion", lambda **_kwargs: (_ for _ in ()).throw(RuntimeError("bad")))

    with pytest.raises(Exception) as exc_info:
        correct.chat_once(model="m", system="s", user="u", retries=2, provider_name="llamacpp")

    assert "모델 통신 2회 시도 모두 실패" in getattr(exc_info.value, "message", "")
