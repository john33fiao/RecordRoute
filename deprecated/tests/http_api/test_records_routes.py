from __future__ import annotations

import json
from pathlib import Path

from sttEngine.http_api.routes import records_routes


def _body(handler):
    handler.wfile.seek(0)
    raw = handler.wfile.read().decode('utf-8')
    return json.loads(raw) if raw else []


def test_segments_route_returns_stt_segments(dummy_handler_factory, monkeypatch, tmp_path: Path):
    stt_file = tmp_path / 'sample.md'
    stt_file.write_text('hello', encoding='utf-8')
    stt_file.with_suffix('.segments.json').write_text(
        json.dumps({'segments': [{'start': 0.0, 'end': 1.2, 'text': '안녕하세요', 'speaker': 'SPEAKER_00'}]}),
        encoding='utf-8',
    )

    monkeypatch.setattr(records_routes, 'resolve_file_identifier', lambda _x: (stt_file, 'r1', 'stt', 'id-1'))

    handler = dummy_handler_factory()
    handler.path = '/segments/id-1'

    handled = records_routes.handle_get(handler)

    assert handled is True
    assert handler.status_code == 200
    payload = _body(handler)
    assert payload[0]['speaker'] == 'SPEAKER_00'
    assert payload[0]['text'] == '안녕하세요'


def test_segments_route_returns_empty_array_when_sidecar_missing(dummy_handler_factory, monkeypatch, tmp_path: Path):
    stt_file = tmp_path / 'sample.md'
    stt_file.write_text('hello', encoding='utf-8')
    monkeypatch.setattr(records_routes, 'resolve_file_identifier', lambda _x: (stt_file, 'r1', 'stt', 'id-1'))

    handler = dummy_handler_factory()
    handler.path = '/segments/id-1'

    handled = records_routes.handle_get(handler)

    assert handled is True
    assert handler.status_code == 200
    assert _body(handler) == []


def test_segments_route_decodes_encoded_identifier(dummy_handler_factory, monkeypatch, tmp_path: Path):
    stt_file = tmp_path / 'sample.md'
    stt_file.write_text('hello', encoding='utf-8')

    called = {}

    def _resolve(identifier: str):
        called['identifier'] = identifier
        return (stt_file, 'r1', 'stt', identifier)

    monkeypatch.setattr(records_routes, 'resolve_file_identifier', _resolve)

    handler = dummy_handler_factory()
    handler.path = '/segments/DB%2Fwhisper%2Fdemo%2Fsample.md'

    handled = records_routes.handle_get(handler)

    assert handled is True
    assert handler.status_code == 200
    assert called['identifier'] == 'DB/whisper/demo/sample.md'


def test_segments_route_returns_not_found_when_unknown(dummy_handler_factory, monkeypatch):
    monkeypatch.setattr(records_routes, 'resolve_file_identifier', lambda _x: (None, None, None, 'id-1'))

    handler = dummy_handler_factory()
    handler.path = '/segments/id-1'

    handled = records_routes.handle_get(handler)

    assert handled is True
    assert handler.status_code == 404
