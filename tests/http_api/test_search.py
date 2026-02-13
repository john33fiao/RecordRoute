from __future__ import annotations

import json
import threading
from http.server import ThreadingHTTPServer
from urllib.request import urlopen

import pytest

from sttEngine.http_api.handler import UploadHandler


@pytest.fixture
def search_server(monkeypatch: pytest.MonkeyPatch):
    from sttEngine.http_api import handler

    monkeypatch.setattr(handler, "collect_searchable_documents", lambda: ([], {}))
    monkeypatch.setattr(handler, "get_active_history", lambda: [])
    monkeypatch.setattr(handler, "collect_keyword_matches", lambda *args, **kwargs: [])

    payloads = {
        "cache-hit": {
            "results": [{"file": "docs/a.md", "score": 0.9}],
            "timing": {"total_ms": 12},
            "cache_hit": True,
        },
        "cache-miss": {
            "results": [{"file": "docs/a.md", "score": 0.8}],
            "timing": {"total_ms": 24},
            "cache_hit": False,
        },
        "default": [{"file": "docs/a.md", "score": 0.7}],
    }

    def fake_search(query, *_args, **_kwargs):
        if "cache-hit" in query:
            return payloads["cache-hit"]
        if "cache-miss" in query:
            return payloads["cache-miss"]
        return payloads["default"]

    monkeypatch.setattr(handler, "search_vectors", fake_search)

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


def _get_json(url: str) -> dict:
    with urlopen(url) as response:
        return json.loads(response.read().decode("utf-8"))


def test_search_parses_query_and_pagination_defaults(search_server: str):
    payload = _get_json(
        f"{search_server}/search?query=hello&sort_by=UPLOADED_AT&sort_order=ASC"
        "&page=oops&page_size=-5&min_score=bad"
    )

    assert payload["query"] == "hello"
    assert payload["sort"] == {"by": "uploaded_at", "order": "asc"}
    assert payload["filters"]["min_score"] is None
    assert payload["pagination"]["page"] == 1
    assert payload["pagination"]["pageSize"] == 1
    assert payload["limit"] == 10


def test_search_reports_cache_hit(search_server: str):
    payload = _get_json(f"{search_server}/search?query=cache-hit&include_timing=true")

    assert payload["cache"] == {"hit": True}
    assert payload["timing"]["total_ms"] == 12


def test_search_reports_cache_miss(search_server: str):
    payload = _get_json(f"{search_server}/search?query=cache-miss&include_timing=true")

    assert payload["cache"] == {"hit": False}
    assert payload["timing"]["total_ms"] == 24
