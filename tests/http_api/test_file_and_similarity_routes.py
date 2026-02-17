from __future__ import annotations

import io
import json
import threading
from http.server import ThreadingHTTPServer
from pathlib import Path
from urllib.request import Request, urlopen

import pytest

from sttEngine.http_api.handler import UploadHandler
from sttEngine.http_api.routes import similarity_routes
from sttEngine.http_api.routes.similar_documents import (
    SimilarDocumentNotFound,
    find_similar_documents,
)


@pytest.fixture
def file_server(tmp_path: Path, monkeypatch: pytest.MonkeyPatch):
    from sttEngine.http_api.routes import file_routes

    frontend_dist = tmp_path / "frontend" / "dist" / "assets"
    frontend_dist.mkdir(parents=True)
    (tmp_path / "frontend" / "dist" / "index.html").write_text("<html>ok</html>", encoding="utf-8")
    (frontend_dist / "app.js").write_text("console.log('ok');", encoding="utf-8")

    download_target = tmp_path / "sample.txt"
    download_target.write_text("hello", encoding="utf-8")

    monkeypatch.setattr(file_routes, "BASE_DIR", tmp_path)
    monkeypatch.setattr(file_routes, "resolve_record_path", lambda _p: download_target)

    httpd = ThreadingHTTPServer(("127.0.0.1", 0), UploadHandler)
    worker = threading.Thread(target=httpd.serve_forever, daemon=True)
    worker.start()

    base_url = f"http://127.0.0.1:{httpd.server_port}"
    try:
        yield base_url
    finally:
        httpd.shutdown()
        httpd.server_close()
        worker.join(timeout=2)


def test_assets_served_with_cache_header(file_server: str):
    with urlopen(f"{file_server}/assets/app.js") as response:
        body = response.read().decode("utf-8")
        assert response.status == 200
        assert "immutable" in (response.headers.get("Cache-Control") or "")
        assert "console.log" in body


def test_download_serves_binary_payload(file_server: str):
    with urlopen(f"{file_server}/download/DB/uploads/1/sample.txt") as response:
        assert response.status == 200
        assert response.read() == b"hello"
        assert "attachment" in (response.headers.get("Content-Disposition") or "")


def test_similar_get_returns_not_found(monkeypatch: pytest.MonkeyPatch):
    class DummyHandler:
        def __init__(self):
            self.path = "/similar/missing"
            self.headers = {}
            self.wfile = io.BytesIO()
            self.status = None
            self.response_headers = {}

        def send_response(self, code):
            self.status = code

        def send_header(self, key, value):
            self.response_headers[key] = value

        def end_headers(self):
            return None

    monkeypatch.setattr(
        similarity_routes,
        "find_similar_documents",
        lambda *_args, **_kwargs: (_ for _ in ()).throw(SimilarDocumentNotFound("파일을 찾을 수 없습니다.")),
    )

    handler = DummyHandler()
    assert similarity_routes.handle_get(handler) is True
    assert handler.status == 404
    payload = json.loads(handler.wfile.getvalue().decode("utf-8"))
    assert payload["error"] == "파일을 찾을 수 없습니다."


def test_find_similar_documents_deduplicates_record_ids(monkeypatch: pytest.MonkeyPatch, tmp_path: Path):
    source = tmp_path / "source.md"
    source.write_text("source", encoding="utf-8")

    monkeypatch.setattr("sttEngine.http_api.routes.similar_documents.resolve_target", lambda *_args, **_kwargs: type("T", (), {
        "file_path": "DB/uploads/a/source.md",
        "full_path": source,
        "current_file_name": "source.md",
        "current_record_id": "r1",
    })())
    monkeypatch.setattr("sttEngine.http_api.routes.similar_documents.resolve_text_file_for_similar", lambda _p: source)
    monkeypatch.setattr("sttEngine.http_api.routes.similar_documents.read_text_with_fallback", lambda _p: "source text")
    monkeypatch.setattr(
        "sttEngine.http_api.routes.similar_documents.search_vectors",
        lambda *_args, **_kwargs: [
            {"file": "DB/uploads/a/source.md", "score": 0.99},
            {"file": "DB/uploads/b/doc1.md", "score": 0.8},
            {"file": "DB/uploads/c/doc2.md", "score": 0.7},
        ],
    )
    monkeypatch.setattr(
        "sttEngine.http_api.routes.similar_documents.load_file_registry",
        lambda: {
            "u1": {"file_path": "DB/uploads/b/doc1.md", "record_id": "r2"},
            "u2": {"file_path": "DB/uploads/c/doc2.md", "record_id": "r2"},
        },
    )
    monkeypatch.setattr(
        "sttEngine.http_api.routes.similar_documents.load_upload_history",
        lambda: [{"id": "r2", "filename": "doc1", "title_summary": "title"}],
    )

    docs = find_similar_documents("dummy")
    assert len(docs) == 1
    assert docs[0]["record_id"] == "r2"
    assert docs[0]["display_name"] == "doc1"


def test_similarity_graph_route_forwards_candidate_strategy(monkeypatch: pytest.MonkeyPatch):
    captured = {}

    class DummyHandler:
        def __init__(self):
            self.path = '/api/similarity-graph?min_similarity=0.7&max_nodes=120&candidate_strategy=lsh'
            self.headers = {}
            self.wfile = io.BytesIO()
            self.status = None
            self.response_headers = {}

        def send_response(self, code):
            self.status = code

        def send_header(self, key, value):
            self.response_headers[key] = value

        def end_headers(self):
            return None

    def fake_get_similarity_graph(**kwargs):
        captured.update(kwargs)
        return {'nodes': [], 'edges': [], 'meta': {'ok': True}}

    monkeypatch.setattr(similarity_routes, 'get_similarity_graph', fake_get_similarity_graph)

    handler = DummyHandler()
    assert similarity_routes.handle_get(handler) is True
    assert handler.status == 200
    assert captured['candidate_strategy'] == 'lsh'


def test_similarity_graph_route_rejects_invalid_candidate_strategy():
    class DummyHandler:
        def __init__(self):
            self.path = '/api/similarity-graph?candidate_strategy=bogus'
            self.headers = {}
            self.wfile = io.BytesIO()
            self.status = None
            self.response_headers = {}

        def send_response(self, code):
            self.status = code

        def send_header(self, key, value):
            self.response_headers[key] = value

        def end_headers(self):
            return None

    handler = DummyHandler()
    assert similarity_routes.handle_get(handler) is True
    assert handler.status == 400
    payload = json.loads(handler.wfile.getvalue().decode('utf-8'))
    assert 'Invalid candidate_strategy' in payload['error']
