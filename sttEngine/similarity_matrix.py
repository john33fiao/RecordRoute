from __future__ import annotations

import threading
import time
from pathlib import Path
from typing import Any

import numpy as np

from .embedding_pipeline import VECTOR_DIR, load_index, resolve_index_path
from .http_api.history import get_active_history
from .http_api.paths import BASE_DIR, normalize_record_path
from .http_api.registry import load_file_registry

_CACHE_LOCK = threading.Lock()
_GRAPH_CACHE: dict[tuple[float, int], dict[str, Any]] = {}
_SUBGRAPH_CACHE: dict[tuple[str, float, int], dict[str, Any]] = {}


def _cosine_similarity(vec_a: np.ndarray, vec_b: np.ndarray) -> float:
    denom = float(np.linalg.norm(vec_a) * np.linalg.norm(vec_b))
    if denom == 0.0:
        return 0.0
    return float(np.dot(vec_a, vec_b) / denom)


def _build_doc_catalog() -> list[dict[str, Any]]:
    registry = load_file_registry()
    history = {record.get("id"): record for record in get_active_history()}
    index = load_index()

    registry_by_path: dict[str, tuple[str, dict[str, Any]]] = {}
    for file_uuid, info in registry.items():
        if not isinstance(info, dict) or info.get("deleted"):
            continue
        rel_path = normalize_record_path(info.get("file_path", ""))
        if rel_path:
            registry_by_path[rel_path] = (file_uuid, info)

    docs: list[dict[str, Any]] = []
    for path_key, meta in index.items():
        if not isinstance(meta, dict) or meta.get("deleted"):
            continue

        vector_name = meta.get("vector")
        if not vector_name:
            continue

        vector_path = VECTOR_DIR / vector_name
        if not vector_path.exists():
            continue

        try:
            resolved = resolve_index_path(path_key, meta)
        except Exception:
            resolved = Path(path_key).resolve()

        rel_path = normalize_record_path(str(resolved))
        registry_match = registry_by_path.get(rel_path)

        file_uuid = registry_match[0] if registry_match else rel_path
        file_info = registry_match[1] if registry_match else {}
        record = history.get(file_info.get("record_id"), {}) if file_info else {}

        docs.append(
            {
                "id": file_uuid,
                "record_id": file_info.get("record_id") if file_info else None,
                "file": rel_path,
                "display_name": file_info.get("original_filename")
                or resolved.name,
                "uploaded_at": record.get("timestamp") or meta.get("timestamp"),
                "vector_path": vector_path,
            }
        )

    docs.sort(key=lambda item: item.get("display_name") or item["id"])
    return docs


def _compute_graph(threshold: float = 0.65, max_neighbors: int = 12) -> dict[str, Any]:
    docs = _build_doc_catalog()
    if not docs:
        return {"nodes": [], "edges": [], "meta": {"threshold": threshold, "count": 0, "generated_at": time.time()}}

    vectors: list[np.ndarray] = []
    valid_docs: list[dict[str, Any]] = []
    for doc in docs:
        try:
            vectors.append(np.load(doc["vector_path"]))
            valid_docs.append(doc)
        except Exception:
            continue

    node_count = len(valid_docs)
    if node_count == 0:
        return {"nodes": [], "edges": [], "meta": {"threshold": threshold, "count": 0, "generated_at": time.time()}}

    matrix = np.zeros((node_count, node_count), dtype=float)
    for i in range(node_count):
        matrix[i, i] = 1.0
        for j in range(i + 1, node_count):
            score = _cosine_similarity(vectors[i], vectors[j])
            matrix[i, j] = score
            matrix[j, i] = score

    nodes = [
        {
            "id": doc["id"],
            "label": doc["display_name"],
            "file": doc["file"],
            "record_id": doc.get("record_id"),
            "uploaded_at": doc.get("uploaded_at"),
        }
        for doc in valid_docs
    ]

    edges: list[dict[str, Any]] = []
    for i in range(node_count):
        row = matrix[i]
        candidate_idx = [j for j in np.argsort(row)[::-1] if j != i and row[j] >= threshold][:max_neighbors]
        for j in candidate_idx:
            if i < j:
                edges.append(
                    {
                        "source": valid_docs[i]["id"],
                        "target": valid_docs[j]["id"],
                        "score": float(row[j]),
                    }
                )

    return {
        "nodes": nodes,
        "edges": edges,
        "meta": {
            "threshold": threshold,
            "count": node_count,
            "generated_at": time.time(),
        },
    }


def invalidate_similarity_cache() -> None:
    with _CACHE_LOCK:
        _GRAPH_CACHE.clear()
        _SUBGRAPH_CACHE.clear()


def get_similarity_graph(threshold: float = 0.65, max_neighbors: int = 12, refresh: bool = False) -> dict[str, Any]:
    key = (round(float(threshold), 4), int(max_neighbors))
    with _CACHE_LOCK:
        if not refresh and key in _GRAPH_CACHE:
            return _GRAPH_CACHE[key]

    graph = _compute_graph(threshold=threshold, max_neighbors=max_neighbors)
    with _CACHE_LOCK:
        _GRAPH_CACHE[key] = graph
    return graph


def get_similarity_subgraph(doc_id: str, threshold: float = 0.65, max_neighbors: int = 12, refresh: bool = False) -> dict[str, Any]:
    cache_key = (doc_id, round(float(threshold), 4), int(max_neighbors))
    with _CACHE_LOCK:
        if not refresh and cache_key in _SUBGRAPH_CACHE:
            return _SUBGRAPH_CACHE[cache_key]

    graph = get_similarity_graph(threshold=threshold, max_neighbors=max_neighbors, refresh=refresh)
    node_ids = {node["id"] for node in graph["nodes"]}
    if doc_id not in node_ids:
        return {"nodes": [], "edges": [], "meta": {"doc_id": doc_id, "found": False}}

    kept_ids = {doc_id}
    for edge in graph["edges"]:
        if edge["source"] == doc_id:
            kept_ids.add(edge["target"])
        elif edge["target"] == doc_id:
            kept_ids.add(edge["source"])

    sub_nodes = [node for node in graph["nodes"] if node["id"] in kept_ids]
    sub_edges = [
        edge
        for edge in graph["edges"]
        if edge["source"] in kept_ids and edge["target"] in kept_ids
    ]

    payload = {
        "nodes": sub_nodes,
        "edges": sub_edges,
        "meta": {
            "doc_id": doc_id,
            "found": True,
            "threshold": threshold,
            "count": len(sub_nodes),
        },
    }
    with _CACHE_LOCK:
        _SUBGRAPH_CACHE[cache_key] = payload
    return payload


def get_documents_metadata() -> dict[str, Any]:
    docs = _build_doc_catalog()
    return {
        "documents": [
            {
                "id": doc["id"],
                "file": doc["file"],
                "display_name": doc["display_name"],
                "record_id": doc.get("record_id"),
                "uploaded_at": doc.get("uploaded_at"),
            }
            for doc in docs
        ],
        "meta": {
            "count": len(docs),
            "base_dir": str(BASE_DIR),
        },
    }
