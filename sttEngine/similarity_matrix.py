from __future__ import annotations

import hashlib
import os
import threading
import time
from pathlib import Path
import random
from typing import Any

import numpy as np

from .embedding_pipeline import VECTOR_DIR, load_index, resolve_index_path
from .http_api.history import get_active_history
from .http_api.paths import BASE_DIR, normalize_record_path
from .http_api.registry import load_file_registry

_CACHE_LOCK = threading.RLock()
_GRAPH_CACHE: dict[tuple[float, int, int, str], dict[str, Any]] = {}
_INCREMENTAL_STATE: dict[tuple[int, str], dict[str, Any]] = {}

DEFAULT_MIN_SIMILARITY = 0.65
DEFAULT_MAX_NEIGHBORS = 12
DEFAULT_MAX_NODES = 150
DEFAULT_SAMPLING_STRATEGY = "hybrid"
ALLOWED_SAMPLING_STRATEGIES = {"hybrid", "recent", "random"}


def _read_env_float(name: str, default: float) -> float:
    raw = os.getenv(name)
    if raw is None:
        return default
    try:
        return float(raw)
    except Exception:
        return default


def _read_env_int(name: str, default: int) -> int:
    raw = os.getenv(name)
    if raw is None:
        return default
    try:
        return int(raw)
    except Exception:
        return default


GRAPH_CACHE_TTL_SECONDS = max(0.0, _read_env_float("RECORDROUTE_SIMILARITY_GRAPH_CACHE_TTL_SECONDS", 300.0))
INCREMENTAL_STATE_TTL_SECONDS = max(0.0, _read_env_float("RECORDROUTE_SIMILARITY_INCREMENTAL_TTL_SECONDS", 900.0))
INCREMENTAL_STATE_MAX_ENTRIES = max(1, _read_env_int("RECORDROUTE_SIMILARITY_INCREMENTAL_MAX_ENTRIES", 6))


def _prune_caches_locked(now: float) -> None:
    if GRAPH_CACHE_TTL_SECONDS > 0:
        expired_graph_keys = [
            key
            for key, entry in _GRAPH_CACHE.items()
            if now - float(entry.get("created_at", 0.0)) > GRAPH_CACHE_TTL_SECONDS
        ]
        for key in expired_graph_keys:
            _GRAPH_CACHE.pop(key, None)

    if INCREMENTAL_STATE_TTL_SECONDS > 0:
        expired_state_keys = [
            key
            for key, entry in _INCREMENTAL_STATE.items()
            if now - float(entry.get("updated_at", 0.0)) > INCREMENTAL_STATE_TTL_SECONDS
        ]
        for key in expired_state_keys:
            _INCREMENTAL_STATE.pop(key, None)

    overflow = len(_INCREMENTAL_STATE) - INCREMENTAL_STATE_MAX_ENTRIES
    if overflow > 0:
        ordered_keys = sorted(
            _INCREMENTAL_STATE.keys(),
            key=lambda key: float(_INCREMENTAL_STATE[key].get("updated_at", 0.0)),
        )
        for key in ordered_keys[:overflow]:
            _INCREMENTAL_STATE.pop(key, None)


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


def _safe_timestamp(value: Any) -> float:
    try:
        if value is None:
            return 0.0
        return float(value)
    except Exception:
        return 0.0


def _sample_docs(docs: list[dict[str, Any]], max_nodes: int, strategy: str) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    total_docs = len(docs)
    if max_nodes <= 0 or total_docs <= max_nodes:
        return docs, {
            "strategy": strategy,
            "total_before_sampling": total_docs,
            "sampled": False,
            "max_nodes": max_nodes,
        }

    if strategy == "recent":
        sampled = sorted(docs, key=lambda item: _safe_timestamp(item.get("uploaded_at")), reverse=True)[:max_nodes]
    elif strategy == "random":
        rng = random.Random(42)
        sampled = rng.sample(docs, max_nodes)
    else:
        recent_count = max(1, int(max_nodes * 0.7))
        random_count = max_nodes - recent_count
        sorted_recent = sorted(docs, key=lambda item: _safe_timestamp(item.get("uploaded_at")), reverse=True)
        sampled = sorted_recent[:recent_count]
        if random_count > 0:
            remaining = sorted_recent[recent_count:]
            if remaining:
                rng = random.Random(42)
                sampled.extend(rng.sample(remaining, min(random_count, len(remaining))))

    sampled.sort(key=lambda item: item.get("display_name") or item["id"])
    return sampled, {
        "strategy": strategy,
        "total_before_sampling": total_docs,
        "sampled": True,
        "max_nodes": max_nodes,
        "total_after_sampling": len(sampled),
    }


def _compute_graph(
    min_similarity: float = DEFAULT_MIN_SIMILARITY,
    max_neighbors: int = DEFAULT_MAX_NEIGHBORS,
    max_nodes: int = DEFAULT_MAX_NODES,
    sampling_strategy: str = DEFAULT_SAMPLING_STRATEGY,
) -> dict[str, Any]:
    docs = _build_doc_catalog()
    docs, sampling_meta = _sample_docs(docs, max_nodes=max_nodes, strategy=sampling_strategy)

    if not docs:
        return {
            "nodes": [],
            "edges": [],
            "meta": {
                "min_similarity": min_similarity,
                "count": 0,
                "generated_at": time.time(),
                "sampling": sampling_meta,
            },
        }

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
        return {
            "nodes": [],
            "edges": [],
            "meta": {
                "min_similarity": min_similarity,
                "count": 0,
                "generated_at": time.time(),
                "sampling": sampling_meta,
            },
        }

    state_key = (max_nodes, sampling_strategy)
    matrix, incremental_meta = _build_similarity_matrix(
        state_key=state_key,
        docs=valid_docs,
        vectors=vectors,
    )

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
        candidate_idx = [j for j in np.argsort(row)[::-1] if j != i and row[j] >= min_similarity][:max_neighbors]
        for j in candidate_idx:
            if i < j:
                edges.append(
                    {
                        "source": valid_docs[i]["id"],
                        "target": valid_docs[j]["id"],
                        "weight": float(row[j]),
                    }
                )

    return {
        "nodes": nodes,
        "edges": edges,
        "meta": {
            "node_schema": {"id": "string", "label": "string", "record_id": "string|null", "file": "string"},
            "edge_schema": {"source": "string", "target": "string", "weight": "number"},
            "min_similarity": min_similarity,
            "count": node_count,
            "max_neighbors": max_neighbors,
            "max_nodes": max_nodes,
            "sampling": sampling_meta,
            "incremental": incremental_meta,
            "generated_at": time.time(),
        },
    }


def _vector_fingerprint(vector: np.ndarray) -> tuple[str, tuple[int, ...], str]:
    contiguous = np.ascontiguousarray(vector)
    digest = hashlib.sha1(contiguous.view(np.uint8).tobytes()).hexdigest()
    return digest, tuple(contiguous.shape), str(contiguous.dtype)


def _build_similarity_matrix(
    state_key: tuple[int, str],
    docs: list[dict[str, Any]],
    vectors: list[np.ndarray],
) -> tuple[np.ndarray, dict[str, Any]]:
    node_count = len(docs)
    matrix = np.zeros((node_count, node_count), dtype=float)
    if node_count == 0:
        return matrix, {
            "reused_pairs": 0,
            "computed_pairs": 0,
            "added_docs": 0,
            "removed_docs": 0,
            "changed_docs": 0,
            "strategy": "incremental",
        }

    current_ids = [str(doc["id"]) for doc in docs]
    current_fingerprints = {
        doc_id: _vector_fingerprint(vector)
        for doc_id, vector in zip(current_ids, vectors)
    }

    with _CACHE_LOCK:
        _prune_caches_locked(time.time())
        prev_state = _INCREMENTAL_STATE.get(state_key)

    previous_index: dict[str, int] = {}
    previous_fingerprints: dict[str, tuple[str, tuple[int, ...], str]] = {}
    previous_matrix: np.ndarray | None = None
    if prev_state:
        previous_index = prev_state.get("id_index", {})
        previous_fingerprints = prev_state.get("fingerprints", {})
        previous_matrix = prev_state.get("matrix")

    unchanged_ids = {
        doc_id
        for doc_id in current_ids
        if doc_id in previous_index and current_fingerprints.get(doc_id) == previous_fingerprints.get(doc_id)
    }

    computed_pairs = 0
    reused_pairs = 0

    for i in range(node_count):
        matrix[i, i] = 1.0
        for j in range(i + 1, node_count):
            source_id = current_ids[i]
            target_id = current_ids[j]
            can_reuse = (
                previous_matrix is not None
                and source_id in unchanged_ids
                and target_id in unchanged_ids
            )
            if can_reuse:
                prev_i = previous_index[source_id]
                prev_j = previous_index[target_id]
                score = float(previous_matrix[prev_i, prev_j])
                reused_pairs += 1
            else:
                score = _cosine_similarity(vectors[i], vectors[j])
                computed_pairs += 1
            matrix[i, j] = score
            matrix[j, i] = score

    with _CACHE_LOCK:
        _INCREMENTAL_STATE[state_key] = {
            "id_index": {doc_id: idx for idx, doc_id in enumerate(current_ids)},
            "fingerprints": current_fingerprints,
            "matrix": matrix,
            "updated_at": time.time(),
        }
        _prune_caches_locked(time.time())

    previous_ids = set(previous_index.keys())
    current_ids_set = set(current_ids)
    added_docs = len(current_ids_set - previous_ids)
    removed_docs = len(previous_ids - current_ids_set)
    changed_docs = len(current_ids_set & previous_ids) - len(unchanged_ids)

    return matrix, {
        "reused_pairs": reused_pairs,
        "computed_pairs": computed_pairs,
        "added_docs": max(0, added_docs),
        "removed_docs": max(0, removed_docs),
        "changed_docs": max(0, changed_docs),
        "strategy": "incremental",
    }


def invalidate_similarity_cache() -> None:
    with _CACHE_LOCK:
        _GRAPH_CACHE.clear()
        _INCREMENTAL_STATE.clear()


def get_similarity_graph(
    min_similarity: float = DEFAULT_MIN_SIMILARITY,
    max_neighbors: int = DEFAULT_MAX_NEIGHBORS,
    max_nodes: int = DEFAULT_MAX_NODES,
    sampling_strategy: str = DEFAULT_SAMPLING_STRATEGY,
    doc_id: str | None = None,
    refresh: bool = False,
) -> dict[str, Any]:
    strategy = sampling_strategy if sampling_strategy in ALLOWED_SAMPLING_STRATEGIES else DEFAULT_SAMPLING_STRATEGY
    key = (round(float(min_similarity), 4), int(max_neighbors), int(max_nodes), strategy)
    cache_hit = False
    with _CACHE_LOCK:
        now = time.time()
        _prune_caches_locked(now)
        cached_entry = _GRAPH_CACHE.get(key)
        if not refresh and cached_entry is not None:
            graph = cached_entry["graph"]
            cache_hit = True
        else:
            graph = _compute_graph(
                min_similarity=min_similarity,
                max_neighbors=max_neighbors,
                max_nodes=max_nodes,
                sampling_strategy=strategy,
            )
            _GRAPH_CACHE[key] = {
                "graph": graph,
                "created_at": time.time(),
            }

    payload = {
        "nodes": graph.get("nodes", []),
        "edges": graph.get("edges", []),
        "meta": dict(graph.get("meta", {})),
    }
    payload["meta"]["cache"] = {
        "hit": cache_hit,
        "refresh": bool(refresh),
        "graph_cache_ttl_seconds": GRAPH_CACHE_TTL_SECONDS,
        "incremental_state_ttl_seconds": INCREMENTAL_STATE_TTL_SECONDS,
        "incremental_state_max_entries": INCREMENTAL_STATE_MAX_ENTRIES,
    }

    if not doc_id:
        return payload

    node_ids = {node["id"] for node in payload["nodes"]}
    if doc_id not in node_ids:
        return {
            "nodes": [],
            "edges": [],
            "meta": {
                "doc_id": doc_id,
                "found": False,
                "min_similarity": min_similarity,
                "cache": payload["meta"].get("cache"),
            },
        }

    kept_ids = {doc_id}
    for edge in payload["edges"]:
        if edge["source"] == doc_id:
            kept_ids.add(edge["target"])
        elif edge["target"] == doc_id:
            kept_ids.add(edge["source"])

    sub_nodes = [node for node in payload["nodes"] if node["id"] in kept_ids]
    sub_edges = [
        edge
        for edge in payload["edges"]
        if edge["source"] in kept_ids and edge["target"] in kept_ids
    ]

    payload = {
        "nodes": sub_nodes,
        "edges": sub_edges,
        "meta": {
            "doc_id": doc_id,
            "found": True,
            "min_similarity": min_similarity,
            "count": len(sub_nodes),
            "node_schema": payload["meta"].get("node_schema"),
            "edge_schema": payload["meta"].get("edge_schema"),
            "cache": payload["meta"].get("cache"),
        },
    }
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
