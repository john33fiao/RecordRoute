from __future__ import annotations

import hashlib
import os
import threading
import time
from datetime import datetime, timezone
from pathlib import Path
import random
from typing import Any

import numpy as np

from .embedding_pipeline import VECTOR_DIR, load_index, resolve_index_path
from .http_api.history import get_active_history
from .http_api.paths import BASE_DIR, normalize_record_path
from .http_api.registry import load_file_registry

_CACHE_LOCK = threading.RLock()
_GRAPH_CACHE: dict[tuple[Any, ...], dict[str, Any]] = {}
_INCREMENTAL_STATE: dict[tuple[int, str], dict[str, Any]] = {}


def _env_float(name: str, default: float, *, minimum: float | None = None) -> float:
    raw = os.getenv(name)
    try:
        parsed = float(raw) if raw is not None else default
    except (TypeError, ValueError):
        parsed = default
    if minimum is not None:
        parsed = max(minimum, parsed)
    return parsed


def _env_int(name: str, default: int, *, minimum: int | None = None) -> int:
    raw = os.getenv(name)
    try:
        parsed = int(raw) if raw is not None else default
    except (TypeError, ValueError):
        parsed = default
    if minimum is not None:
        parsed = max(minimum, parsed)
    return parsed

DEFAULT_MIN_SIMILARITY = 0.65
DEFAULT_MAX_NEIGHBORS = 12
DEFAULT_MAX_NODES = 150
DEFAULT_SAMPLING_STRATEGY = "hybrid"
DEFAULT_NEIGHBOR_STRATEGY = "auto"
ALLOWED_SAMPLING_STRATEGIES = {"hybrid", "recent", "random"}
ALLOWED_NEIGHBOR_STRATEGIES = {"auto", "exact", "lsh"}
ALLOWED_DOC_TYPES = {"audio", "document", "other"}
ANN_AUTO_MIN_NODES = 220
LSH_TABLES = 6
LSH_BITS = 14
AUDIO_EXTENSIONS = {".mp3", ".wav", ".m4a", ".aac", ".ogg", ".flac", ".wma", ".opus"}
DOCUMENT_EXTENSIONS = {".txt", ".md", ".pdf", ".doc", ".docx", ".ppt", ".pptx", ".xls", ".xlsx", ".hwp"}
GRAPH_CACHE_TTL_SECONDS = _env_float("RECORDROUTE_SIMILARITY_GRAPH_CACHE_TTL_SECONDS", 300.0, minimum=0.0)
GRAPH_CACHE_MAX_ENTRIES = _env_int("RECORDROUTE_SIMILARITY_GRAPH_CACHE_MAX_ENTRIES", 24, minimum=1)
INCREMENTAL_STATE_TTL_SECONDS = _env_float("RECORDROUTE_SIMILARITY_INCREMENTAL_STATE_TTL_SECONDS", 900.0, minimum=0.0)
INCREMENTAL_STATE_MAX_ENTRIES = _env_int("RECORDROUTE_SIMILARITY_INCREMENTAL_STATE_MAX_ENTRIES", 8, minimum=1)


def _prune_graph_cache(now: float | None = None) -> None:
    now_ts = now if now is not None else time.time()
    expired_keys = [
        key
        for key, payload in _GRAPH_CACHE.items()
        if GRAPH_CACHE_TTL_SECONDS > 0 and (now_ts - float(payload.get("cached_at", now_ts))) > GRAPH_CACHE_TTL_SECONDS
    ]
    for key in expired_keys:
        _GRAPH_CACHE.pop(key, None)

    if len(_GRAPH_CACHE) <= GRAPH_CACHE_MAX_ENTRIES:
        return

    overflow = len(_GRAPH_CACHE) - GRAPH_CACHE_MAX_ENTRIES
    ordered_keys = sorted(_GRAPH_CACHE.keys(), key=lambda key: float(_GRAPH_CACHE[key].get("cached_at", 0.0)))
    for key in ordered_keys[:overflow]:
        _GRAPH_CACHE.pop(key, None)


def _prune_incremental_state(now: float | None = None) -> None:
    now_ts = now if now is not None else time.time()
    expired_keys = [
        key
        for key, payload in _INCREMENTAL_STATE.items()
        if INCREMENTAL_STATE_TTL_SECONDS > 0 and (now_ts - float(payload.get("updated_at", now_ts))) > INCREMENTAL_STATE_TTL_SECONDS
    ]
    for key in expired_keys:
        _INCREMENTAL_STATE.pop(key, None)

    if len(_INCREMENTAL_STATE) <= INCREMENTAL_STATE_MAX_ENTRIES:
        return

    overflow = len(_INCREMENTAL_STATE) - INCREMENTAL_STATE_MAX_ENTRIES
    ordered_keys = sorted(_INCREMENTAL_STATE.keys(), key=lambda key: float(_INCREMENTAL_STATE[key].get("updated_at", 0.0)))
    for key in ordered_keys[:overflow]:
        _INCREMENTAL_STATE.pop(key, None)


def _cache_policy_meta() -> dict[str, Any]:
    return {
        "graph_cache": {
            "ttl_seconds": GRAPH_CACHE_TTL_SECONDS,
            "max_entries": GRAPH_CACHE_MAX_ENTRIES,
            "entries": len(_GRAPH_CACHE),
        },
        "incremental_state": {
            "ttl_seconds": INCREMENTAL_STATE_TTL_SECONDS,
            "max_entries": INCREMENTAL_STATE_MAX_ENTRIES,
            "entries": len(_INCREMENTAL_STATE),
        },
    }


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
    if value is None:
        return 0.0

    if isinstance(value, (int, float)):
        return float(value)

    if isinstance(value, str):
        raw = value.strip()
        if not raw:
            return 0.0
        try:
            return float(raw)
        except ValueError:
            parsed = _parse_datetime(raw)
            return parsed if parsed is not None else 0.0

    return 0.0


def _resolve_doc_type(file_path: str | None) -> str:
    if not file_path:
        return "other"
    suffix = Path(file_path).suffix.lower()
    if suffix in AUDIO_EXTENSIONS:
        return "audio"
    if suffix in DOCUMENT_EXTENSIONS:
        return "document"
    return "other"


def _parse_datetime(value: str | None) -> float | None:
    if not value:
        return None
    raw = value.strip()
    if not raw:
        return None

    try:
        return float(raw)
    except ValueError:
        pass

    normalized = raw.replace("Z", "+00:00")
    try:
        parsed = datetime.fromisoformat(normalized)
    except ValueError:
        return None

    if parsed.tzinfo is None:
        parsed = parsed.replace(tzinfo=timezone.utc)

    return parsed.timestamp()


def _filter_docs(
    docs: list[dict[str, Any]],
    *,
    doc_types: set[str] | None = None,
    start_date: str | None = None,
    end_date: str | None = None,
    keyword: str | None = None,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    normalized_types = {item.lower() for item in (doc_types or set()) if item.lower() in ALLOWED_DOC_TYPES}
    keyword_norm = (keyword or "").strip().lower()
    start_ts = _parse_datetime(start_date)
    end_ts = _parse_datetime(end_date)

    filtered: list[dict[str, Any]] = []
    for doc in docs:
        current_type = _resolve_doc_type(doc.get("file"))
        if normalized_types and current_type not in normalized_types:
            continue

        uploaded_ts = _safe_timestamp(doc.get("uploaded_at"))
        if start_ts is not None and uploaded_ts < start_ts:
            continue
        if end_ts is not None and uploaded_ts > end_ts:
            continue

        if keyword_norm:
            haystack = " ".join(
                str(part or "")
                for part in (doc.get("display_name"), doc.get("file"), doc.get("id"))
            ).lower()
            if keyword_norm not in haystack:
                continue

        filtered.append(doc)

    return filtered, {
        "doc_types": sorted(normalized_types),
        "start_date": start_date,
        "end_date": end_date,
        "keyword": keyword_norm or None,
        "total_before_filtering": len(docs),
        "total_after_filtering": len(filtered),
    }


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
    doc_types: set[str] | None = None,
    start_date: str | None = None,
    end_date: str | None = None,
    keyword: str | None = None,
    neighbor_strategy: str = DEFAULT_NEIGHBOR_STRATEGY,
) -> dict[str, Any]:
    docs = _build_doc_catalog()
    docs, filter_meta = _filter_docs(
        docs,
        doc_types=doc_types,
        start_date=start_date,
        end_date=end_date,
        keyword=keyword,
    )
    docs, sampling_meta = _sample_docs(docs, max_nodes=max_nodes, strategy=sampling_strategy)

    if not docs:
        return {
            "nodes": [],
            "edges": [],
            "meta": {
                "min_similarity": min_similarity,
                "count": 0,
                "generated_at": time.time(),
                "filters": filter_meta,
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
                "filters": filter_meta,
                "sampling": sampling_meta,
            },
        }

    state_key = (max_nodes, sampling_strategy)
    effective_strategy = _resolve_neighbor_strategy(neighbor_strategy, node_count)

    nodes = [
        {
            "id": doc["id"],
            "label": doc["display_name"],
            "file": doc["file"],
            "file_type": _resolve_doc_type(doc.get("file")),
            "record_id": doc.get("record_id"),
            "uploaded_at": doc.get("uploaded_at"),
        }
        for doc in valid_docs
    ]

    if effective_strategy == "lsh":
        edges, incremental_meta = _build_similarity_edges_lsh(
            docs=valid_docs,
            vectors=vectors,
            min_similarity=min_similarity,
            max_neighbors=max_neighbors,
        )
    else:
        matrix, incremental_meta = _build_similarity_matrix(
            state_key=state_key,
            docs=valid_docs,
            vectors=vectors,
        )
        edges = _build_similarity_edges_from_matrix(
            matrix=matrix,
            docs=valid_docs,
            min_similarity=min_similarity,
            max_neighbors=max_neighbors,
        )

    return {
        "nodes": nodes,
        "edges": edges,
        "meta": {
            "node_schema": {"id": "string", "label": "string", "record_id": "string|null", "file": "string", "file_type": "audio|document|other"},
            "edge_schema": {"source": "string", "target": "string", "weight": "number"},
            "min_similarity": min_similarity,
            "count": node_count,
            "max_neighbors": max_neighbors,
            "max_nodes": max_nodes,
            "filters": filter_meta,
            "sampling": sampling_meta,
            "neighbor_strategy": {
                "requested": neighbor_strategy,
                "effective": effective_strategy,
            },
            "incremental": incremental_meta,
            "generated_at": time.time(),
        },
    }


def _resolve_neighbor_strategy(strategy: str, node_count: int) -> str:
    normalized = (strategy or DEFAULT_NEIGHBOR_STRATEGY).strip().lower()
    if normalized not in ALLOWED_NEIGHBOR_STRATEGIES:
        normalized = DEFAULT_NEIGHBOR_STRATEGY
    if normalized == "auto":
        return "lsh" if node_count >= ANN_AUTO_MIN_NODES else "exact"
    return normalized


def _build_similarity_edges_from_matrix(
    *,
    matrix: np.ndarray,
    docs: list[dict[str, Any]],
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


def _normalize_vectors(vectors: list[np.ndarray]) -> np.ndarray:
    matrix = np.asarray(vectors, dtype=float)
    if matrix.ndim == 1:
        matrix = matrix.reshape(1, -1)
    norms = np.linalg.norm(matrix, axis=1, keepdims=True)
    safe_norms = np.where(norms == 0.0, 1.0, norms)
    return matrix / safe_norms


def _build_similarity_edges_lsh(
    *,
    docs: list[dict[str, Any]],
    vectors: list[np.ndarray],
    min_similarity: float,
    max_neighbors: int,
) -> tuple[list[dict[str, Any]], dict[str, Any]]:
    node_count = len(docs)
    full_pairs = node_count * (node_count - 1) // 2
    if node_count <= 1:
        return [], {
            "strategy": "lsh",
            "full_pairs": full_pairs,
            "candidate_pairs": 0,
            "computed_pairs": 0,
            "pruned_pairs": 0,
            "reused_pairs": 0,
            "added_docs": 0,
            "removed_docs": 0,
            "changed_docs": 0,
            "tables": LSH_TABLES,
            "bits": LSH_BITS,
        }

    normalized = _normalize_vectors(vectors)
    dimensions = normalized.shape[1]
    candidate_sets: list[set[int]] = [set() for _ in range(node_count)]

    rng = np.random.default_rng(42)
    for _table_idx in range(LSH_TABLES):
        hyperplanes = rng.standard_normal((LSH_BITS, dimensions))
        projections = normalized @ hyperplanes.T
        signatures = projections > 0
        buckets: dict[tuple[bool, ...], list[int]] = {}
        for idx in range(node_count):
            key = tuple(bool(value) for value in signatures[idx])
            buckets.setdefault(key, []).append(idx)
        for members in buckets.values():
            if len(members) <= 1:
                continue
            for i in members:
                for j in members:
                    if i != j:
                        candidate_sets[i].add(j)

    candidate_pairs = sum(len(item) for item in candidate_sets) // 2
    edges: list[dict[str, Any]] = []

    for i in range(node_count):
        candidates = sorted(candidate_sets[i])
        if not candidates:
            continue

        scored: list[tuple[int, float]] = []
        row = normalized[i]
        for j in candidates:
            score = float(np.dot(row, normalized[j]))
            if score >= min_similarity:
                scored.append((j, score))

        scored.sort(key=lambda item: item[1], reverse=True)
        for j, score in scored[:max_neighbors]:
            if i < j:
                edges.append(
                    {
                        "source": docs[i]["id"],
                        "target": docs[j]["id"],
                        "weight": score,
                    }
                )

    return edges, {
        "strategy": "lsh",
        "full_pairs": full_pairs,
        "candidate_pairs": candidate_pairs,
        "computed_pairs": candidate_pairs,
        "pruned_pairs": max(0, full_pairs - candidate_pairs),
        "reused_pairs": 0,
        "added_docs": 0,
        "removed_docs": 0,
        "changed_docs": 0,
        "tables": LSH_TABLES,
        "bits": LSH_BITS,
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
        _prune_incremental_state()
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
        _prune_incremental_state()

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
    doc_types: list[str] | None = None,
    start_date: str | None = None,
    end_date: str | None = None,
    keyword: str | None = None,
    neighbor_strategy: str = DEFAULT_NEIGHBOR_STRATEGY,
    refresh: bool = False,
) -> dict[str, Any]:
    strategy = sampling_strategy if sampling_strategy in ALLOWED_SAMPLING_STRATEGIES else DEFAULT_SAMPLING_STRATEGY
    normalized_doc_types = tuple(sorted({item.lower() for item in (doc_types or []) if item.lower() in ALLOWED_DOC_TYPES}))
    normalized_keyword = (keyword or "").strip().lower()
    normalized_neighbor_strategy = (neighbor_strategy or DEFAULT_NEIGHBOR_STRATEGY).strip().lower()
    if normalized_neighbor_strategy not in ALLOWED_NEIGHBOR_STRATEGIES:
        normalized_neighbor_strategy = DEFAULT_NEIGHBOR_STRATEGY
    key = (
        round(float(min_similarity), 4),
        int(max_neighbors),
        int(max_nodes),
        strategy,
        normalized_neighbor_strategy,
        normalized_doc_types,
        (start_date or "").strip(),
        (end_date or "").strip(),
        normalized_keyword,
    )
    with _CACHE_LOCK:
        _prune_graph_cache()
        cached_entry = _GRAPH_CACHE.get(key)
        cache_hit = bool(not refresh and cached_entry is not None)
        if cache_hit:
            graph = cached_entry["graph"]
        else:
            graph = _compute_graph(
                min_similarity=min_similarity,
                max_neighbors=max_neighbors,
                max_nodes=max_nodes,
                sampling_strategy=strategy,
                doc_types=set(normalized_doc_types),
                start_date=start_date,
                end_date=end_date,
                keyword=normalized_keyword or None,
                neighbor_strategy=normalized_neighbor_strategy,
            )
            _GRAPH_CACHE[key] = {"graph": graph, "cached_at": time.time()}
            _prune_graph_cache()

    graph_meta = graph.get("meta") if isinstance(graph, dict) else None
    if isinstance(graph_meta, dict):
        graph_meta["cache"] = {
            "hit": cache_hit,
            "refresh_requested": bool(refresh),
            "policy": _cache_policy_meta(),
        }

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
                "file_type": _resolve_doc_type(doc.get("file")),
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
