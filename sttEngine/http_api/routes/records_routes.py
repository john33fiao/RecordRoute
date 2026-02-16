from __future__ import annotations

import json

from ..destructive_guard import check_destructive_api_access, reject_destructive_api_request
from ..paths import normalize_record_path, resolve_record_path, to_record_path
from ..records import delete_file, delete_records, reset_summary_and_embedding, update_stt_text
from ..registry import update_filename
from ..workflow import find_existing_stt_file


def _read_json_payload(handler):
    length = int(handler.headers.get('Content-Length', 0))
    return json.loads(handler.rfile.read(length)) if length else {}


def handle_post(handler) -> bool:
    if handler.path == '/update_filename':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Invalid JSON payload')
            return True

        record_id = payload.get('record_id')
        new_filename = payload.get('filename')

        if not record_id or not new_filename:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Missing record_id or filename')
            return True

        update_filename(record_id, new_filename)
        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps({'success': True}).encode())
        return True

    if handler.path == '/check_existing_stt':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Invalid JSON payload')
            return True

        file_path = payload.get('file_path')
        if not file_path:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Missing file_path')
            return True

        try:
            normalized_path = normalize_record_path(file_path)
            original_file = resolve_record_path(normalized_path)
            existing_stt = find_existing_stt_file(original_file)

            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(
                json.dumps(
                    {
                        'has_stt': existing_stt is not None,
                        'stt_file': to_record_path(existing_stt) if existing_stt else None,
                    }
                ).encode()
            )
        except Exception as e:
            handler.send_response(500)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'has_stt': False, 'error': str(e)}).encode())
        return True

    if handler.path == '/update_stt_text':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': False, 'error': 'Invalid JSON payload'}).encode())
            return True

        file_identifier = payload.get('file_identifier')
        content = payload.get('content', '')
        if not isinstance(content, str):
            content = str(content)

        success, message, record_id = update_stt_text(file_identifier, content)
        if success:
            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': True, 'record_id': record_id}).encode())
        else:
            handler.send_response(400)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': False, 'error': message, 'record_id': record_id}).encode())
        return True

    if handler.path == '/reset_summary_embedding':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': False, 'error': 'Invalid JSON payload'}).encode())
            return True

        allowed, message = check_destructive_api_access(handler, payload)
        if not allowed:
            reject_destructive_api_request(handler, message or 'Forbidden')
            return True

        record_id = payload.get('record_id')
        success, message = reset_summary_and_embedding(record_id)
        status_code = 200 if success else 400
        handler.send_response(status_code)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps({'success': success, 'message': message}).encode())
        return True

    if handler.path == '/delete':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Invalid JSON payload')
            return True

        allowed, message = check_destructive_api_access(handler, payload)
        if not allowed:
            reject_destructive_api_request(handler, message or 'Forbidden')
            return True

        file_identifier = payload.get('file_identifier')
        file_type = payload.get('file_type')
        if not file_identifier or not file_type:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Missing file_identifier or file_type')
            return True

        success, error_msg = delete_file(file_identifier, file_type)
        if success:
            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': True}).encode())
        else:
            handler.send_response(400)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'error': error_msg}).encode())
        return True

    if handler.path == '/delete_records':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Invalid JSON payload')
            return True

        allowed, message = check_destructive_api_access(handler, payload)
        if not allowed:
            reject_destructive_api_request(handler, message or 'Forbidden')
            return True

        record_ids = payload.get('record_ids')
        if not isinstance(record_ids, list):
            handler.send_response(400)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': False, 'error': 'record_ids 필드는 배열이어야 합니다.'}).encode())
            return True

        success, results = delete_records([str(r) for r in record_ids])
        status_code = 200 if success else 207
        handler.send_response(status_code)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps({'success': success, 'results': results}, ensure_ascii=False).encode())
        return True

    return False
