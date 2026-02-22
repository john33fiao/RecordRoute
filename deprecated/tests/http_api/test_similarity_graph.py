from __future__ import annotations

import json
import threading
from http.server import ThreadingHTTPServer
from urllib.parse import quote
from urllib.request import urlopen

import pytest

from sttEngine.http_api.handler import UploadHandler


@pytest.fixture
def server(monkeypatch: pytest.MonkeyPatch):
    from sttEngine.http_api.routes import similarity_routes

    captured: dict[str, object] = {}

    def fake_graph(**kwargs):
        captured.update(kwargs)
        return {"nodes": [], "edges": [], "meta": {"ok": True}}

    monkeypatch.setattr(similarity_routes, "get_similarity_graph", fake_graph)

    httpd = ThreadingHTTPServer(("127.0.0.1", 0), UploadHandler)
    worker = threading.Thread(target=httpd.serve_forever, daemon=True)
    worker.start()

    base_url = f"http://127.0.0.1:{httpd.server_port}"
    try:
        yield base_url, captured
    finally:
        httpd.shutdown()
        httpd.server_close()
        worker.join(timeout=2)


def test_similarity_graph_accepts_filter_and_sampling_query(server):
    base_url, captured = server
    with urlopen(
        (
            f"{base_url}/api/similarity-graph?min_similarity=0.7&max_neighbors=8&max_nodes=90"
            "&sampling=recent&doc_types=audio,document&start_date=2025-01-01"
            f"&end_date=2025-12-31&keyword={quote('회의')}"
        )
    ) as response:
        payload = json.loads(response.read().decode("utf-8"))

    assert payload["meta"]["ok"] is True
    assert captured["min_similarity"] == 0.7
    assert captured["max_neighbors"] == 8
    assert captured["max_nodes"] == 90
    assert captured["sampling_strategy"] == "recent"
    assert captured["doc_types"] == ["audio", "document"]
    assert captured["start_date"] == "2025-01-01"
    assert captured["end_date"] == "2025-12-31"
    assert captured["keyword"] == "회의"


def test_similarity_graph_normalizes_invalid_params(server):
    base_url, captured = server
    with urlopen(
        f"{base_url}/api/similarity-graph?min_similarity=bad&max_neighbors=0&max_nodes=-2&refresh=YES"
    ) as response:
        payload = json.loads(response.read().decode("utf-8"))

    assert payload["meta"]["ok"] is True
    assert captured["min_similarity"] == 0.65
    assert captured["max_neighbors"] == 1
    assert captured["max_nodes"] == 1
    assert captured["refresh"] is True


def test_similarity_graph_accepts_start_end_alias(server):
    base_url, captured = server
    with urlopen(f"{base_url}/api/similarity-graph?start=2025-01-01&end=2025-12-31") as response:
        payload = json.loads(response.read().decode("utf-8"))

    assert payload["meta"]["ok"] is True
    assert captured["start_date"] == "2025-01-01"
    assert captured["end_date"] == "2025-12-31"


def test_similarity_graph_accepts_neighbor_strategy(server):
    base_url, captured = server
    with urlopen(f"{base_url}/api/similarity-graph?neighbor_strategy=lsh") as response:
        payload = json.loads(response.read().decode("utf-8"))

    assert payload["meta"]["ok"] is True
    assert captured["neighbor_strategy"] == "lsh"
