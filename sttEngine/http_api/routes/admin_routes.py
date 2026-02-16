from __future__ import annotations

import json

from ..destructive_guard import check_destructive_api_access, reject_destructive_api_request


def handle_get(handler) -> bool:
    if handler.path == '/models':
        handler._serve_available_models()
        return True
    return False


def handle_post(handler) -> bool:
    if handler.path != '/shutdown':
        return False

    payload = {}
    allowed, message = check_destructive_api_access(handler, payload)
    if not allowed:
        reject_destructive_api_request(handler, message or 'Forbidden')
        return True

    print('Shutdown request received via /shutdown endpoint')
    response_data = {
        'success': True,
        'message': '서버 종료 요청이 접수되었습니다. 잠시 후 서버가 종료됩니다.',
    }
    handler.send_response(200)
    handler.send_header('Content-Type', 'application/json')
    handler.end_headers()
    handler.wfile.write(json.dumps(response_data, ensure_ascii=False).encode())
    handler._schedule_server_shutdown()
    handler.close_connection = True
    return True
