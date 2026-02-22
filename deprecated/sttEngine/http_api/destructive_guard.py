from __future__ import annotations

import os
from typing import Any


TOKEN_HEADER = 'X-RecordRoute-Admin-Token'
SESSION_ID_HEADER = 'X-RecordRoute-Session-Id'
SESSION_TOKEN_HEADER = 'X-RecordRoute-Session-Token'


def _truthy_env(value: str | None) -> bool:
    return (value or '').strip().lower() in {'1', 'true', 'yes', 'on'}


def _get_payload_value(payload: dict[str, Any], key: str) -> str:
    value = payload.get(key)
    return str(value).strip() if value is not None else ''


def check_destructive_api_access(handler, payload: dict[str, Any] | None = None) -> tuple[bool, str | None]:
    """파괴적 API 접근 제어.

    기본값은 안전 모드(deny by default)이며, 토큰 또는 세션(아이디+토큰)
    중 하나가 검증되면 요청을 허용한다.
    """

    safe_mode_enabled = _truthy_env(os.getenv('RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE', 'true'))
    if not safe_mode_enabled:
        return True, None

    payload = payload or {}

    configured_admin_token = (os.getenv('RECORDROUTE_DESTRUCTIVE_API_TOKEN') or '').strip()
    configured_session_id = (os.getenv('RECORDROUTE_DESTRUCTIVE_API_SESSION_ID') or '').strip()
    configured_session_token = (os.getenv('RECORDROUTE_DESTRUCTIVE_API_SESSION_TOKEN') or '').strip()

    has_token_policy = bool(configured_admin_token)
    has_session_policy = bool(configured_session_id and configured_session_token)

    if not has_token_policy and not has_session_policy:
        return False, '파괴적 API 보호가 활성화되어 있지만 인증 토큰/세션이 설정되지 않았습니다.'

    request_admin_token = (handler.headers.get(TOKEN_HEADER) or _get_payload_value(payload, 'admin_token')).strip()
    if has_token_policy and request_admin_token and request_admin_token == configured_admin_token:
        return True, None

    request_session_id = (handler.headers.get(SESSION_ID_HEADER) or _get_payload_value(payload, 'session_id')).strip()
    request_session_token = (handler.headers.get(SESSION_TOKEN_HEADER) or _get_payload_value(payload, 'session_token')).strip()
    if (
        has_session_policy
        and request_session_id
        and request_session_token
        and request_session_id == configured_session_id
        and request_session_token == configured_session_token
    ):
        return True, None

    return False, '파괴적 API 보호 정책에 의해 요청이 거부되었습니다. 토큰 또는 세션 정보를 확인해주세요.'


def reject_destructive_api_request(handler, message: str) -> None:
    handler.send_response(403)
    handler.send_header('Content-Type', 'application/json')
    handler.end_headers()
    handler.wfile.write(
        (
            '{"success": false, "error": "'
            + message.replace('"', '\\"')
            + '", "error_code": "destructive_api_protected", "retryable": false}'
        ).encode()
    )

