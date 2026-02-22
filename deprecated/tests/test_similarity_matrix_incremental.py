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


def test_resolve_neighbor_strategy_auto_threshold():
    assert similarity_matrix._resolve_neighbor_strategy('auto', 40) == 'exact'
    assert similarity_matrix._resolve_neighbor_strategy('auto', 300) == 'lsh'
    assert similarity_matrix._resolve_neighbor_strategy('invalid', 300) == 'lsh'


def test_compute_graph_uses_lsh_neighbor_strategy(monkeypatch, tmp_path):
    similarity_matrix.invalidate_similarity_cache()

    doc_count = 230
    docs = []
    vectors = []
    for idx in range(doc_count):
        vector = np.array([float((idx * 3) % 11), float((idx * 5) % 13), float((idx * 7) % 17)], dtype=float)
        if np.linalg.norm(vector) == 0:
            vector = np.array([1.0, 0.0, 0.0], dtype=float)
        vec_path = tmp_path / f"{idx}.npy"
        np.save(vec_path, vector)
        vectors.append(vector)
        docs.append({
            'id': f'doc-{idx}',
            'display_name': f'Doc {idx}',
            'file': f'DB/uploads/{idx}.txt',
            'vector_path': vec_path,
            'record_id': None,
            'uploaded_at': '2025-01-01T00:00:00Z',
        })

    monkeypatch.setattr(similarity_matrix, '_build_doc_catalog', lambda: docs)

    graph = similarity_matrix.get_similarity_graph(
        min_similarity=0.3,
        max_neighbors=4,
        max_nodes=300,
        sampling_strategy='hybrid',
        neighbor_strategy='auto',
        refresh=True,
    )

    meta = graph['meta']
    assert meta['neighbor_strategy']['effective'] == 'lsh'
    assert meta['incremental']['strategy'] == 'lsh'
    assert meta['incremental']['full_pairs'] > meta['incremental']['candidate_pairs']


def test_similarity_graph_cache_hit_and_policy_meta(monkeypatch, tmp_path):
    similarity_matrix.invalidate_similarity_cache()

    docs = []
    for idx in range(3):
        vec_path = tmp_path / f"{idx}.npy"
        np.save(vec_path, np.array([1.0, float(idx + 1), 0.0], dtype=float))
        docs.append({
            'id': f'doc-{idx}',
            'display_name': f'Doc {idx}',
            'file': f'DB/uploads/{idx}.txt',
            'vector_path': vec_path,
            'record_id': None,
            'uploaded_at': '2025-01-01T00:00:00Z',
        })

    monkeypatch.setattr(similarity_matrix, '_build_doc_catalog', lambda: docs)

    first = similarity_matrix.get_similarity_graph(max_nodes=10, sampling_strategy='hybrid', refresh=False)
    first_hit = first['meta']['cache']['hit']
    second = similarity_matrix.get_similarity_graph(max_nodes=10, sampling_strategy='hybrid', refresh=False)

    assert first_hit is False
    assert second['meta']['cache']['hit'] is True
    assert second['meta']['cache']['policy']['graph_cache']['entries'] >= 1


def test_prune_incremental_state_by_ttl(monkeypatch):
    now = 1000.0
    monkeypatch.setattr(similarity_matrix, 'INCREMENTAL_STATE_TTL_SECONDS', 5.0)
    monkeypatch.setattr(similarity_matrix, 'INCREMENTAL_STATE_MAX_ENTRIES', 10)

    similarity_matrix._INCREMENTAL_STATE.clear()
    similarity_matrix._INCREMENTAL_STATE[(1, 'hybrid')] = {'updated_at': now - 20.0}
    similarity_matrix._INCREMENTAL_STATE[(2, 'hybrid')] = {'updated_at': now - 2.0}

    similarity_matrix._prune_incremental_state(now)

    assert (1, 'hybrid') not in similarity_matrix._INCREMENTAL_STATE
    assert (2, 'hybrid') in similarity_matrix._INCREMENTAL_STATE


def test_prune_graph_cache_by_entry_limit(monkeypatch):
    monkeypatch.setattr(similarity_matrix, 'GRAPH_CACHE_TTL_SECONDS', 1000.0)
    monkeypatch.setattr(similarity_matrix, 'GRAPH_CACHE_MAX_ENTRIES', 2)

    similarity_matrix._GRAPH_CACHE.clear()
    similarity_matrix._GRAPH_CACHE[(1,)] = {'graph': {'nodes': []}, 'cached_at': 10.0}
    similarity_matrix._GRAPH_CACHE[(2,)] = {'graph': {'nodes': []}, 'cached_at': 20.0}
    similarity_matrix._GRAPH_CACHE[(3,)] = {'graph': {'nodes': []}, 'cached_at': 30.0}

    similarity_matrix._prune_graph_cache(now=40.0)

    assert (1,) not in similarity_matrix._GRAPH_CACHE
    assert (2,) in similarity_matrix._GRAPH_CACHE
    assert (3,) in similarity_matrix._GRAPH_CACHE
