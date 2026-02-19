from sttEngine.workflow.transcribe import merge_segments


def test_merge_segments_does_not_merge_different_speakers():
    segments = [
        {"start": 0.0, "end": 1.0, "text": "안녕하세요", "speaker": "SPEAKER_00"},
        {"start": 1.0, "end": 2.0, "text": "안녕하세요", "speaker": "SPEAKER_01"},
    ]

    merged = merge_segments(segments, max_gap=0.2)

    assert len(merged) == 2
    assert merged[0]["speaker"] == "SPEAKER_00"
    assert merged[1]["speaker"] == "SPEAKER_01"


def test_merge_segments_merges_same_text_same_speaker_contiguous():
    segments = [
        {"start": 0.0, "end": 1.0, "text": "테스트", "speaker": "SPEAKER_00"},
        {"start": 1.1, "end": 2.0, "text": "테스트", "speaker": "SPEAKER_00"},
    ]

    merged = merge_segments(segments, max_gap=0.2)

    assert len(merged) == 1
    assert merged[0]["end"] == 2.0
