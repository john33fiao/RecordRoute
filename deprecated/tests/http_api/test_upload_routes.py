from __future__ import annotations

import io
import json
from pathlib import Path

import pytest

from sttEngine.http_api.routes import upload_routes


class DummyUploadHandler:
    def __init__(self, body: bytes, content_type: str):
        self.path = '/upload'
        self.headers = {
            'Content-Type': content_type,
            'Content-Length': str(len(body)),
        }
        self.rfile = io.BytesIO(body)
        self.wfile = io.BytesIO()
        self.status = None
        self.response_headers = {}

    def send_response(self, code):
        self.status = code

    def send_header(self, key, value):
        self.response_headers[key] = value

    def end_headers(self):
        return None


def _multipart_body(boundary: str, filename: str, content: bytes, field_name: str = 'files') -> bytes:
    lines = [
        f'--{boundary}',
        f'Content-Disposition: form-data; name="{field_name}"; filename="{filename}"',
        'Content-Type: application/octet-stream',
        '',
    ]
    head = '\r\n'.join(lines).encode('utf-8') + b'\r\n'
    tail = f'\r\n--{boundary}--\r\n'.encode('utf-8')
    return head + content + tail


def test_upload_rejects_invalid_content_type():
    handler = DummyUploadHandler(body=b'{}', content_type='application/json')

    assert upload_routes.handle_post(handler) is True
    assert handler.status == 400
    assert handler.wfile.getvalue() == b'Invalid content type'


def test_upload_saves_file_and_returns_record(monkeypatch: pytest.MonkeyPatch, tmp_path: Path):
    uploads_dir = tmp_path / 'uploads'
    monkeypatch.setattr(upload_routes, 'UPLOAD_DIR', uploads_dir)
    monkeypatch.setattr(upload_routes, 'load_upload_history', lambda: [])
    monkeypatch.setattr(upload_routes, 'get_file_type', lambda _path: 'audio')
    monkeypatch.setattr(upload_routes, 'get_audio_duration', lambda _path: '00:05')

    captured = {}

    def fake_add_upload_record(file_path, file_type, duration, file_hash):
        captured['file_path'] = file_path
        captured['file_type'] = file_type
        captured['duration'] = duration
        captured['file_hash'] = file_hash
        return {'id': 'rec-1', 'file_hash': file_hash}

    monkeypatch.setattr(upload_routes, 'add_upload_record', fake_add_upload_record)

    boundary = '----recordroute-boundary'
    body = _multipart_body(boundary, 'hello.wav', b'abc123')
    handler = DummyUploadHandler(body=body, content_type=f'multipart/form-data; boundary={boundary}')

    assert upload_routes.handle_post(handler) is True
    assert handler.status == 200

    payload = json.loads(handler.wfile.getvalue().decode('utf-8'))
    assert payload[0]['record_id'] == 'rec-1'
    assert payload[0]['file_type'] == 'audio'
    assert payload[0]['file_path'].endswith('/hello.wav')
    assert captured['file_type'] == 'audio'
    assert captured['duration'] == '00:05'
    assert captured['file_path'].exists()


def test_upload_returns_duplicate_for_existing_hash(monkeypatch: pytest.MonkeyPatch):
    file_bytes = b'duplicate-content'
    existing_hash = upload_routes.compute_file_hash(file_bytes)
    monkeypatch.setattr(upload_routes, 'load_upload_history', lambda: [{'id': 'rec-existing', 'file_hash': existing_hash}])

    boundary = '----recordroute-boundary'
    body = _multipart_body(boundary, 'dup.txt', file_bytes)
    handler = DummyUploadHandler(body=body, content_type=f'multipart/form-data; boundary={boundary}')

    assert upload_routes.handle_post(handler) is True
    assert handler.status == 200
    payload = json.loads(handler.wfile.getvalue().decode('utf-8'))
    assert payload == [{'duplicate': True, 'original_record_id': 'rec-existing', 'filename': 'dup.txt'}]
