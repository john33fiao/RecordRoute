from __future__ import annotations

import json

from sttEngine.http_api.routes import admin_routes, management_routes, records_routes


def _body(handler) -> dict:
    handler.wfile.seek(0)
    raw = handler.wfile.read().decode('utf-8')
    return json.loads(raw) if raw else {}


def test_destructive_routes_blocked_in_default_safe_mode_without_credentials(dummy_handler_factory, monkeypatch):
    monkeypatch.delenv('RECORDROUTE_DESTRUCTIVE_API_TOKEN', raising=False)
    monkeypatch.delenv('RECORDROUTE_DESTRUCTIVE_API_SESSION_ID', raising=False)
    monkeypatch.delenv('RECORDROUTE_DESTRUCTIVE_API_SESSION_TOKEN', raising=False)
    monkeypatch.delenv('RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE', raising=False)

    handler = dummy_handler_factory({'record_id': 'abc'})
    handler.path = '/reset'

    handled = management_routes.handle_post(handler)

    assert handled is True
    assert handler.status_code == 403
    assert _body(handler)['error_code'] == 'destructive_api_protected'


def test_destructive_route_allows_with_admin_token_header(dummy_handler_factory, monkeypatch):
    monkeypatch.setenv('RECORDROUTE_DESTRUCTIVE_API_TOKEN', 'token-123')
    monkeypatch.setenv('RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE', 'true')

    handler = dummy_handler_factory({'record_id': 'abc'})
    handler.path = '/reset_summary_embedding'
    handler.headers['X-RecordRoute-Admin-Token'] = 'token-123'

    monkeypatch.setattr(records_routes, 'reset_summary_and_embedding', lambda _id: (True, 'ok'))

    handled = records_routes.handle_post(handler)

    assert handled is True
    assert handler.status_code == 200
    assert _body(handler)['success'] is True


def test_destructive_route_allows_with_session_credentials_in_body(dummy_handler_factory, monkeypatch):
    monkeypatch.delenv('RECORDROUTE_DESTRUCTIVE_API_TOKEN', raising=False)
    monkeypatch.setenv('RECORDROUTE_DESTRUCTIVE_API_SESSION_ID', 'session-a')
    monkeypatch.setenv('RECORDROUTE_DESTRUCTIVE_API_SESSION_TOKEN', 'session-t')
    monkeypatch.setenv('RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE', 'true')

    handler = dummy_handler_factory({'record_ids': ['r1'], 'session_id': 'session-a', 'session_token': 'session-t'})
    handler.path = '/delete_records'

    monkeypatch.setattr(records_routes, 'delete_records', lambda _ids: (True, {'r1': {'success': True}}))

    handled = records_routes.handle_post(handler)

    assert handled is True
    assert handler.status_code == 200
    assert _body(handler)['success'] is True


def test_shutdown_blocked_without_token_when_safe_mode_enabled(dummy_handler_factory, monkeypatch):
    monkeypatch.delenv('RECORDROUTE_DESTRUCTIVE_API_TOKEN', raising=False)
    monkeypatch.setenv('RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE', 'true')

    handler = dummy_handler_factory({})
    handler.path = '/shutdown'

    handled = admin_routes.handle_post(handler)

    assert handled is True
    assert handler.status_code == 403


def test_shutdown_allowed_when_safe_mode_disabled(dummy_handler_factory, monkeypatch):
    monkeypatch.setenv('RECORDROUTE_DESTRUCTIVE_API_SAFE_MODE', 'false')

    handler = dummy_handler_factory({})
    handler.path = '/shutdown'
    handler.close_connection = False
    handler._schedule_server_shutdown = lambda: None

    handled = admin_routes.handle_post(handler)

    assert handled is True
    assert handler.status_code == 200
    assert handler.close_connection is True
