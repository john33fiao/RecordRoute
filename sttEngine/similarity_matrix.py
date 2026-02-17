from __future__ import annotations

import hashlib
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
_GRAPH_CACHE: dict[tuple[float, int, int, str, str], dict[str, Any]] = {}
_INCREMENTAL_STATE: dict[tuple[int, str], dict[str, Any]] = {}

DEFAULT_MIN_SIMILARITY = 0.65
DEFAULT_MAX_NEIGHBORS = 12
DEFAULT_MAX_NODES = 150
DEFAULT_SAMPLING_STRATEGY = "hybrid"
ALLOWED_SAMPLING_STRATEGIES = {"hybrid", "recent", "random"}
DEFAULT_CANDIDATE_STRATEGY = "auto"
ALLOWED_CANDIDATE_STRATEGIES = {"auto", "exact", "lsh"}
LSH_AUTO_MIN_DOCS = 320
LSH_NUM_TABLES = 6
LSH_NUM_PLANES = 14
LSH_RECALL_SAMPLE_LIMIT = 24
LSH_MAX_BUCKET_SIZE = 80


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
    candidate_strategy: str = DEFAULT_CANDIDATE_STRATEGY,
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

    effective_candidate_strategy = _resolve_candidate_strategy(candidate_strategy, node_count)
    incremental_meta: dict[str, Any]
    candidate_meta: dict[str, Any]
    if effective_candidate_strategy == "exact":
        state_key = (max_nodes, sampling_strategy)
        matrix, incremental_meta = _build_similarity_matrix(
            state_key=state_key,
            docs=valid_docs,
            vectors=vectors,
        )
        edges = _build_edges_from_matrix(
            docs=valid_docs,
            matrix=matrix,
            min_similarity=min_similarity,
            max_neighbors=max_neighbors,
        )
        total_pairs = (node_count * (node_count - 1)) // 2
        candidate_meta = {
            "strategy": "exact",
            "total_pairs": total_pairs,
            "candidate_pairs": total_pairs,
            "reduction_ratio": 0.0,
            "estimated_recall_at_k": 1.0,
        }
    else:
        edges, ann_meta = _build_edges_lsh(
            docs=valid_docs,
            vectors=vectors,
            min_similarity=min_similarity,
            max_neighbors=max_neighbors,
        )
        incremental_meta = {
            "reused_pairs": 0,
            "computed_pairs": ann_meta["computed_pairs"],
            "added_docs": node_count,
            "removed_docs": 0,
            "changed_docs": 0,
            "strategy": "ann_lsh",
        }
        candidate_meta = {
            "strategy": "lsh",
            "total_pairs": ann_meta["total_pairs"],
            "candidate_pairs": ann_meta["candidate_pairs"],
            "computed_pairs": ann_meta["computed_pairs"],
            "reduction_ratio": ann_meta["reduction_ratio"],
            "estimated_recall_at_k": ann_meta["estimated_recall_at_k"],
            "num_tables": ann_meta["num_tables"],
            "num_planes": ann_meta["num_planes"],
        }

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
            "candidate_reduction": candidate_meta,
            "generated_at": time.time(),
        },
    }


def _resolve_candidate_strategy(strategy: str, node_count: int) -> str:
    if strategy == "exact":
        return "exact"
    if strategy == "lsh":
        return "lsh"
    return "lsh" if node_count >= LSH_AUTO_MIN_DOCS else "exact"


def _build_edges_from_matrix(
    docs: list[dict[str, Any]],
    matrix: np.ndarray,
    min_similarity: float,
    max_neighbors: int,
) -> list[dict[str, Any]]:
    node_count = len(docs)
    edges: list[dict[str, Any]] = []
    for i in range(node_count):
        row = matrix[i]
        candidate_idx = [j for j in np.argsort(row)[::-1] if j != i and row[j] >= min_similarity][:max_neighbors]
        for j in candidate_idx:
            if i < j:
                edges.append(
                    {
                        "source": docs[i]["id"],
                        "target": docs[j]["id"],
                        "weight": float(row[j]),
                    }
                )
    return edges


def _build_edges_lsh(
    docs: list[dict[str, Any]],
    vectors: list[np.ndarray],
    min_similarity: float,
    max_neighbors: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    node_count = len(docs)
    normalized = np.vstack([_normalize_vector(vec) for vec in vectors])
    candidate_pairs = _collect_lsh_candidate_pairs(normalized)

    scores_by_node: list[dict[int, float]] = [dict() for _ in range(node_count)]
    computed_pairs = 0
    for i, j in candidate_pairs:
        score = float(np.dot(normalized[i], normalized[j]))
        computed_pairs += 1
        if score < min_similarity:
            continue
        scores_by_node[i][j] = score
        scores_by_node[j][i] = score

    edges = _build_edges_from_candidates(docs, scores_by_node, max_neighbors)
    total_pairs = (node_count * (node_count - 1)) // 2
    recall = _estimate_lsh_recall(
        normalized=normalized,
        scores_by_node=scores_by_node,
        max_neighbors=max_neighbors,
    )
    return edges, {
        "strategy": "lsh",
        "total_pairs": total_pairs,
        "candidate_pairs": len(candidate_pairs),
        "computed_pairs": computed_pairs,
        "reduction_ratio": 0.0 if total_pairs == 0 else 1.0 - (len(candidate_pairs) / total_pairs),
        "estimated_recall_at_k": recall,
        "num_tables": LSH_NUM_TABLES,
        "num_planes": LSH_NUM_PLANES,
    }


def _normalize_vector(vector: np.ndarray) -> np.ndarray:
    v = np.asarray(vector, dtype=float).reshape(-1)
    norm = float(np.linalg.norm(v))
    if norm == 0.0:
        return np.zeros_like(v)
    return v / norm


def _collect_lsh_candidate_pairs(vectors: np.ndarray) -> set[tuple[int, int]]:
    _, dim = vectors.shape
    rng = np.random.default_rng(42)
    pairs: set[tuple[int, int]] = set()

    for _ in range(LSH_NUM_TABLES):
        hyperplanes = rng.normal(size=(LSH_NUM_PLANES, dim))
        projections = vectors @ hyperplanes.T
        signatures = projections >= 0
        buckets: dict[bytes, list[int]] = {}
        for idx, signature in enumerate(signatures):
            key = signature.tobytes()
            buckets.setdefault(key, []).append(idx)
        for bucket in buckets.values():
            if len(bucket) < 2:
                continue
            if len(bucket) > LSH_MAX_BUCKET_SIZE:
                continue
            for i_pos in range(len(bucket)):
                for j_pos in range(i_pos + 1, len(bucket)):
                    i = bucket[i_pos]
                    j = bucket[j_pos]
                    pairs.add((i, j) if i < j else (j, i))

    return pairs


def _build_edges_from_candidates(
    docs: list[dict[str, Any]],
    scores_by_node: list[dict[int, float]],
    max_neighbors: int,
) -> list[dict[str, Any]]:
    edges: list[dict[str, Any]] = []
    for i, neighbors in enumerate(scores_by_node):
        sorted_neighbors = sorted(neighbors.items(), key=lambda item: item[1], reverse=True)[:max_neighbors]
        for j, score in sorted_neighbors:
            if i < j:
                edges.append(
                    {
                        "source": docs[i]["id"],
                        "target": docs[j]["id"],
                        "weight": float(score),
                    }
                )
    return edges


def _estimate_lsh_recall(
    normalized: np.ndarray,
    scores_by_node: list[dict[int, float]],
    max_neighbors: int,
) -> float:
    node_count = normalized.shape[0]
    if node_count <= 1 or max_neighbors <= 0:
        return 1.0

    sample_count = min(node_count, LSH_RECALL_SAMPLE_LIMIT)
    rng = random.Random(42)
    sampled_indices = list(range(node_count))
    if sample_count < node_count:
        sampled_indices = rng.sample(sampled_indices, sample_count)

    recalls: list[float] = []
    for idx in sampled_indices:
        exact_scores = normalized @ normalized[idx]
        exact_order = [
            i for i in np.argsort(exact_scores)[::-1]
            if i != idx
        ]
        exact_top = set(exact_order[:max_neighbors])
        if not exact_top:
            continue
        approx_top = {
            neighbor_idx
            for neighbor_idx, _ in sorted(scores_by_node[idx].items(), key=lambda item: item[1], reverse=True)[:max_neighbors]
        }
        recalls.append(len(exact_top & approx_top) / len(exact_top))

    if not recalls:
        return 1.0
    return float(sum(recalls) / len(recalls))


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
        }

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
    candidate_strategy: str = DEFAULT_CANDIDATE_STRATEGY,
    doc_id: str | None = None,
    refresh: bool = False,
) -> dict[str, Any]:
    strategy = sampling_strategy if sampling_strategy in ALLOWED_SAMPLING_STRATEGIES else DEFAULT_SAMPLING_STRATEGY
    candidate = candidate_strategy if candidate_strategy in ALLOWED_CANDIDATE_STRATEGIES else DEFAULT_CANDIDATE_STRATEGY
    key = (round(float(min_similarity), 4), int(max_neighbors), int(max_nodes), strategy, candidate)
    with _CACHE_LOCK:
        if not refresh and key in _GRAPH_CACHE:
            graph = _GRAPH_CACHE[key]
        else:
            graph = _compute_graph(
                min_similarity=min_similarity,
                max_neighbors=max_neighbors,
                max_nodes=max_nodes,
                sampling_strategy=strategy,
                candidate_strategy=candidate,
            )
            _GRAPH_CACHE[key] = graph

    if not doc_id:
        return graph

    node_ids = {node["id"] for node in graph["nodes"]}
    if doc_id not in node_ids:
        return {"nodes": [], "edges": [], "meta": {"doc_id": doc_id, "found": False, "min_similarity": min_similarity}}

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
            "min_similarity": min_similarity,
            "count": len(sub_nodes),
            "node_schema": graph["meta"].get("node_schema"),
            "edge_schema": graph["meta"].get("edge_schema"),
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
