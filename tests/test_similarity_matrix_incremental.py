from __future__ import annotations

import numpy as np

from sttEngine import similarity_matrix


def _doc(doc_id: str, vector_path):
    return {"id": doc_id, "vector_path": vector_path}


def test_incremental_similarity_matrix_reuses_unchanged_pairs(tmp_path):
    similarity_matrix.invalidate_similarity_cache()

    vec_a = tmp_path / "a.npy"
    vec_b = tmp_path / "b.npy"
    vec_c = tmp_path / "c.npy"
    np.save(vec_a, np.array([1.0, 0.0, 0.0], dtype=float))
    np.save(vec_b, np.array([0.0, 1.0, 0.0], dtype=float))
    np.save(vec_c, np.array([0.0, 0.0, 1.0], dtype=float))

    docs = [_doc("a", vec_a), _doc("b", vec_b), _doc("c", vec_c)]
    vectors = [np.load(vec_a), np.load(vec_b), np.load(vec_c)]

    _, meta_first = similarity_matrix._build_similarity_matrix((150, "hybrid"), docs, vectors)
    assert meta_first["computed_pairs"] == 3
    assert meta_first["reused_pairs"] == 0

    _, meta_second = similarity_matrix._build_similarity_matrix((150, "hybrid"), docs, vectors)
    assert meta_second["computed_pairs"] == 0
    assert meta_second["reused_pairs"] == 3
    assert meta_second["added_docs"] == 0
    assert meta_second["removed_docs"] == 0
    assert meta_second["changed_docs"] == 0


def test_incremental_similarity_matrix_recomputes_changed_docs(tmp_path):
    similarity_matrix.invalidate_similarity_cache()

    vec_a = tmp_path / "a.npy"
    vec_b = tmp_path / "b.npy"
    vec_c = tmp_path / "c.npy"
    np.save(vec_a, np.array([1.0, 0.0, 0.0], dtype=float))
    np.save(vec_b, np.array([0.0, 1.0, 0.0], dtype=float))
    np.save(vec_c, np.array([0.0, 0.0, 1.0], dtype=float))

    docs = [_doc("a", vec_a), _doc("b", vec_b), _doc("c", vec_c)]
    vectors = [np.load(vec_a), np.load(vec_b), np.load(vec_c)]

    similarity_matrix._build_similarity_matrix((150, "hybrid"), docs, vectors)

    np.save(vec_b, np.array([1.0, 1.0, 0.0], dtype=float))
    updated_vectors = [np.load(vec_a), np.load(vec_b), np.load(vec_c)]

    _, meta = similarity_matrix._build_similarity_matrix((150, "hybrid"), docs, updated_vectors)
    assert meta["changed_docs"] == 1
    assert meta["computed_pairs"] == 2
    assert meta["reused_pairs"] == 1


def test_incremental_state_respects_max_entries(monkeypatch, tmp_path):
    similarity_matrix.invalidate_similarity_cache()
    monkeypatch.setattr(similarity_matrix, "INCREMENTAL_STATE_MAX_ENTRIES", 2)
    monkeypatch.setattr(similarity_matrix, "INCREMENTAL_STATE_TTL_SECONDS", 9999.0)

    vec_a = tmp_path / "a.npy"
    vec_b = tmp_path / "b.npy"
    np.save(vec_a, np.array([1.0, 0.0, 0.0], dtype=float))
    np.save(vec_b, np.array([0.0, 1.0, 0.0], dtype=float))

    docs = [_doc("a", vec_a), _doc("b", vec_b)]
    vectors = [np.load(vec_a), np.load(vec_b)]

    similarity_matrix._build_similarity_matrix((100, "hybrid"), docs, vectors)
    similarity_matrix._build_similarity_matrix((200, "hybrid"), docs, vectors)
    similarity_matrix._build_similarity_matrix((300, "hybrid"), docs, vectors)

    assert len(similarity_matrix._INCREMENTAL_STATE) == 2
    assert (100, "hybrid") not in similarity_matrix._INCREMENTAL_STATE


def test_similarity_graph_meta_contains_cache_diagnostics(monkeypatch, tmp_path):
    similarity_matrix.invalidate_similarity_cache()

    vec_a = tmp_path / "a.npy"
    vec_b = tmp_path / "b.npy"
    np.save(vec_a, np.array([1.0, 0.0, 0.0], dtype=float))
    np.save(vec_b, np.array([0.7, 0.7, 0.0], dtype=float))

    docs = [
        {"id": "a", "record_id": None, "file": "DB/a.md", "display_name": "A", "uploaded_at": None, "vector_path": vec_a},
        {"id": "b", "record_id": None, "file": "DB/b.md", "display_name": "B", "uploaded_at": None, "vector_path": vec_b},
    ]
    monkeypatch.setattr(similarity_matrix, "_build_doc_catalog", lambda: docs)

    first = similarity_matrix.get_similarity_graph(refresh=True)
    second = similarity_matrix.get_similarity_graph(refresh=False)

    assert first["meta"]["cache"]["hit"] is False
    assert first["meta"]["cache"]["refresh"] is True
    assert second["meta"]["cache"]["hit"] is True
    assert "graph_cache_ttl_seconds" in second["meta"]["cache"]


def test_similarity_graph_doc_subgraph_not_found_preserves_cache_meta(monkeypatch, tmp_path):
    similarity_matrix.invalidate_similarity_cache()

    vec_a = tmp_path / "a.npy"
    vec_b = tmp_path / "b.npy"
    np.save(vec_a, np.array([1.0, 0.0, 0.0], dtype=float))
    np.save(vec_b, np.array([0.7, 0.7, 0.0], dtype=float))

    docs = [
        {"id": "a", "record_id": None, "file": "DB/a.md", "display_name": "A", "uploaded_at": None, "vector_path": vec_a},
        {"id": "b", "record_id": None, "file": "DB/b.md", "display_name": "B", "uploaded_at": None, "vector_path": vec_b},
    ]
    monkeypatch.setattr(similarity_matrix, "_build_doc_catalog", lambda: docs)

    payload = similarity_matrix.get_similarity_graph(doc_id="missing-doc")

    assert payload["meta"]["found"] is False
    assert payload["meta"]["doc_id"] == "missing-doc"
    assert "cache" in payload["meta"]
    assert "hit" in payload["meta"]["cache"]
