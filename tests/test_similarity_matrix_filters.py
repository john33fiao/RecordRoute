from __future__ import annotations

from sttEngine import similarity_matrix


def test_filter_docs_supports_iso_uploaded_at_and_doc_types():
    docs = [
        {
            "id": "a",
            "display_name": "회의록 A",
            "file": "DB/uploads/a/audio.m4a",
            "uploaded_at": "2025-01-02T10:00:00Z",
        },
        {
            "id": "b",
            "display_name": "회의록 B",
            "file": "DB/uploads/b/notes.md",
            "uploaded_at": "2024-12-31T23:59:59Z",
        },
    ]

    filtered, meta = similarity_matrix._filter_docs(
        docs,
        doc_types={"audio"},
        start_date="2025-01-01",
        end_date="2025-12-31",
    )

    assert [doc["id"] for doc in filtered] == ["a"]
    assert meta["doc_types"] == ["audio"]
    assert meta["total_before_filtering"] == 2
    assert meta["total_after_filtering"] == 1


def test_safe_timestamp_parses_iso_string():
    ts = similarity_matrix._safe_timestamp("2025-01-01T00:00:00Z")
    assert ts > 0
