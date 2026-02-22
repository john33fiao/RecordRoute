from __future__ import annotations

import json
import os
import re
import traceback
import uuid

from ..history import add_upload_record, compute_file_hash, load_upload_history
from ..paths import UPLOAD_DIR, to_record_path
from ..workflow import get_audio_duration, get_file_type


def _parse_multipart(data: bytes, boundary: str) -> dict[str, list[dict[str, bytes | str]]]:
    parts = data.split(f'--{boundary}'.encode())
    files: dict[str, list[dict[str, bytes | str]]] = {}

    for part in parts[1:-1]:
        if b'Content-Disposition' not in part:
            continue

        headers, body = part.split(b'\r\n\r\n', 1)
        headers_text = headers.decode('utf-8')
        body = body.rstrip(b'\r\n')

        if 'filename=' not in headers_text:
            continue

        filename_match = re.search(r'filename="([^"]*)"', headers_text)
        name_match = re.search(r'name="([^"]*)"', headers_text)
        if not filename_match or not name_match:
            continue

        files.setdefault(name_match.group(1), []).append({'filename': filename_match.group(1), 'data': body})

    return files


def _save_upload(file_info: dict[str, bytes | str], history: list[dict]) -> dict:
    filename = str(file_info['filename'])
    data = file_info['data']
    if not isinstance(data, bytes):
        raise ValueError('Invalid multipart payload')

    file_hash = compute_file_hash(data)
    existing = next((record for record in history if record.get('file_hash') == file_hash), None)
    if existing:
        return {
            'duplicate': True,
            'original_record_id': existing['id'],
            'filename': filename,
        }

    uid = uuid.uuid4().hex
    save_dir = UPLOAD_DIR / uid
    save_dir.mkdir(parents=True, exist_ok=True)
    file_path = save_dir / os.path.basename(filename)

    with open(file_path, 'wb') as output_file:
        output_file.write(data)

    print(f'File saved successfully: {file_path}')

    file_type = get_file_type(file_path)
    duration = get_audio_duration(file_path) if file_type == 'audio' else None
    record = add_upload_record(file_path, file_type, duration, file_hash)
    history.insert(0, record)

    return {
        'file_path': to_record_path(file_path),
        'file_type': file_type,
        'record_id': record['id'],
    }


def handle_post(handler) -> bool:
    if handler.path != '/upload':
        return False

    try:
        print(f"Upload request received - Content-Length: {handler.headers.get('Content-Length')}")
        print(f"Content-Type: {handler.headers.get('Content-Type')}")

        content_type = handler.headers.get('Content-Type', '')
        if not content_type.startswith('multipart/form-data'):
            print('Upload failed: Not multipart/form-data')
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Invalid content type')
            return True

        boundary_match = re.search(r'boundary=([^;]+)', content_type)
        if not boundary_match:
            print('Upload failed: No boundary found')
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'No boundary found')
            return True

        boundary = boundary_match.group(1).strip()
        content_length = int(handler.headers.get('Content-Length', 0))
        data = handler.rfile.read(content_length)

        files = _parse_multipart(data, boundary)
        print(f"Parsed fields: {list(files.keys())}")

        file_entries = files.get('files') or files.get('file')
        if not file_entries:
            print('Upload failed: No files provided')
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'No file uploaded')
            return True

        history = load_upload_history()
        uploaded_files = []
        for file_info in file_entries:
            if not file_info.get('filename'):
                continue
            uploaded_files.append(_save_upload(file_info, history))

        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(uploaded_files).encode())
        return True
    except Exception as e:
        print(f'Upload error: {str(e)}')
        print(f'Exception type: {type(e).__name__}')
        traceback.print_exc()
        handler.send_response(500)
        handler.end_headers()
        handler.wfile.write(f'Upload error: {str(e)}'.encode())
        return True
