from pathlib import Path

from sttEngine.workflow import transcribe


def test_get_whisper_model_download_root_prefers_env(monkeypatch, tmp_path):
    env_dir = tmp_path / "custom_whisper_models"
    monkeypatch.setenv("WHISPER_MODEL_DIR", str(env_dir))
    monkeypatch.setattr(transcribe, "get_project_root", lambda: tmp_path / "project_root")

    resolved = transcribe.get_whisper_model_download_root()

    assert resolved == env_dir
    assert resolved.exists()
    assert resolved.is_dir()


def test_get_whisper_model_download_root_defaults_to_project_models(monkeypatch, tmp_path):
    project_root = tmp_path / "repo"
    monkeypatch.delenv("WHISPER_MODEL_DIR", raising=False)
    monkeypatch.setattr(transcribe, "get_project_root", lambda: project_root)

    resolved = transcribe.get_whisper_model_download_root()

    assert resolved == project_root / "models"
    assert resolved.exists()
    assert resolved.is_dir()


def test_transcribe_audio_files_passes_download_root_to_whisper_load_model(
    monkeypatch,
    tmp_path,
):
    input_dir = tmp_path / "input"
    output_dir = tmp_path / "output"
    input_dir.mkdir()
    media_file = input_dir / "sample.wav"
    media_file.write_bytes(b"RIFF")

    expected_download_root = tmp_path / "whisper_models"
    monkeypatch.setattr(
        transcribe,
        "get_whisper_model_download_root",
        lambda: expected_download_root,
    )
    monkeypatch.setattr(transcribe, "list_media_files", lambda *_args, **_kwargs: [media_file])
    monkeypatch.setattr(transcribe, "resolve_inference_device", lambda _requested: ("cpu", "ok"))

    captured = {}

    def fake_load_model(model_identifier, device=None, download_root=None):
        captured["model_identifier"] = model_identifier
        captured["device"] = device
        captured["download_root"] = download_root
        return object()

    monkeypatch.setattr(transcribe.whisper, "load_model", fake_load_model)
    monkeypatch.setattr(
        transcribe,
        "transcribe_single_file",
        lambda file_path, output_dir, *_args, **_kwargs: output_dir / f"{file_path.stem}.md",
    )

    transcribe.transcribe_audio_files(
        input_dir=str(input_dir),
        output_dir=str(output_dir),
        model_identifier="large-v3-turbo",
        language="ko",
        initial_prompt="",
        workers=1,
        recursive=False,
        filter_fillers=False,
        min_seg_length=2,
        normalize_punct=False,
        requested_device="cpu",
    )

    assert captured["model_identifier"] == "large-v3-turbo"
    assert captured["device"] == "cpu"
    assert captured["download_root"] == str(expected_download_root)
