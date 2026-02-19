from sttEngine.workflow.speaker_utils import (
    align_diarization_to_stt_segments,
    format_speaker_label,
    normalize_speaker_label,
)


def test_format_and_normalize_speaker_label():
    assert format_speaker_label(0) == "SPEAKER_00"
    assert normalize_speaker_label("speaker_1") == "SPEAKER_01"
    assert normalize_speaker_label("SPEAKER_09") == "SPEAKER_09"
    assert normalize_speaker_label("spk2") == "SPEAKER_02"


def test_align_diarization_to_stt_segments_assigns_by_overlap():
    stt_segments = [
        {"start": 0.0, "end": 1.0, "text": "a"},
        {"start": 1.0, "end": 2.0, "text": "b"},
    ]
    diarization_segments = [
        {"start": 0.0, "end": 1.4, "speaker": "speaker_0"},
        {"start": 1.4, "end": 2.0, "speaker": "1"},
    ]

    aligned = align_diarization_to_stt_segments(stt_segments, diarization_segments)

    assert aligned[0]["speaker"] == "SPEAKER_00"
    assert aligned[1]["speaker"] == "SPEAKER_01"


def test_align_diarization_to_stt_segments_default_speaker_policy():
    stt_segments = [{"start": 4.0, "end": 5.0, "text": "x"}]

    aligned = align_diarization_to_stt_segments(stt_segments, [], allow_null_speaker=False)
    assert aligned[0]["speaker"] == "SPEAKER_00"

    aligned_null = align_diarization_to_stt_segments(stt_segments, [], allow_null_speaker=True)
    assert aligned_null[0]["speaker"] is None
