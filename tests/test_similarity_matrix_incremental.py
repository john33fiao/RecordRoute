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


def test_lsh_candidate_reduction_reports_meta():
    docs = [{"id": f"doc-{idx}", "display_name": f"doc-{idx}", "file": f"DB/uploads/{idx}.txt", "record_id": None, "uploaded_at": None} for idx in range(64)]
    vectors = []
    rng = np.random.default_rng(0)
    for _ in range(64):
        vec = rng.normal(size=48)
        vectors.append(vec)

    edges, meta = similarity_matrix._build_edges_lsh(
        docs=docs,
        vectors=vectors,
        min_similarity=0.2,
        max_neighbors=8,
    )

    assert isinstance(edges, list)
    assert meta["strategy"] == "lsh"
    assert meta["candidate_pairs"] <= meta["total_pairs"]
    assert 0.0 <= meta["reduction_ratio"] <= 1.0
    assert 0.0 <= meta["estimated_recall_at_k"] <= 1.0


def test_auto_candidate_strategy_switches_by_size():
    assert similarity_matrix._resolve_candidate_strategy("auto", 100) == "exact"
    assert similarity_matrix._resolve_candidate_strategy("auto", 1000) == "lsh"
    assert similarity_matrix._resolve_candidate_strategy("exact", 1000) == "exact"
