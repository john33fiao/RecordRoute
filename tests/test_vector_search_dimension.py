from __future__ import annotations

from pathlib import Path

import numpy as np

from sttEngine import vector_search


def test_search_skips_dimension_mismatch(tmp_path: Path, monkeypatch) -> None:
    base_dir = tmp_path / "DB"
    vector_dir = base_dir / "vector_store"
    vector_dir.mkdir(parents=True)

    np.save(vector_dir / "a.npy", np.array([0.1, 0.2, 0.3], dtype=np.float32))

    index = {
        "doc-a": {
            "vector": "a.npy",
            "timestamp": "2024-01-01T00:00:00",
            "vector_dimension": 3,
        }
    }

    monkeypatch.setattr(vector_search, "VECTOR_DIR", vector_dir)
    monkeypatch.setattr(vector_search, "load_index", lambda: index)
    monkeypatch.setattr(vector_search, "resolve_index_path", lambda *_args, **_kwargs: base_dir / "doc-a.md")
    monkeypatch.setattr(vector_search, "get_model_for_task", lambda *_args, **_kwargs: "bge-m3")
    monkeypatch.setattr(vector_search, "get_default_model", lambda *_args, **_kwargs: "bge-m3")
    monkeypatch.setattr(vector_search, "embed_text", lambda *_args, **_kwargs: np.array([1.0, 2.0], dtype=np.float32))
    monkeypatch.setattr(vector_search, "get_cached_search_result", lambda *_args, **_kwargs: None)
    monkeypatch.setattr(vector_search, "cache_search_result", lambda *_args, **_kwargs: None)

    results = vector_search.search("hello", base_dir=base_dir, include_timing=False)

    assert results == []
