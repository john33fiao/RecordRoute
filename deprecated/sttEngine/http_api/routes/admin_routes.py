from __future__ import annotations

import json
import os
import threading
import time
from urllib.parse import parse_qs, urlparse

from ...providers.factory import get_llm_provider
from ..destructive_guard import check_destructive_api_access, reject_destructive_api_request


def _serve_available_models(handler, provider_name: str | None = None) -> None:
    try:
        provider_aliases = {
            'llama.cpp': 'llamacpp',
            'llama_cpp': 'llamacpp',
            'llama-cpp': 'llamacpp',
        }
        provider_models: dict[str, list[str]] = {'ollama': [], 'llamacpp': []}
        provider_status: dict[str, dict[str, str | bool]] = {
            'ollama': {'ok': False, 'message': 'not_checked'},
            'llamacpp': {'ok': False, 'message': 'not_checked'},
        }

        configured_provider = (os.getenv('LLM_PROVIDER') or 'ollama').strip().lower()
        active_provider = provider_aliases.get(configured_provider, configured_provider)
        normalized_requested_provider = (provider_name or '').strip().lower()
        requested_provider = provider_aliases.get(normalized_requested_provider, normalized_requested_provider or None)
        if requested_provider not in {'ollama', 'llamacpp'}:
            requested_provider = active_provider if active_provider in {'ollama', 'llamacpp'} else 'ollama'

        for resolved_provider in (requested_provider,):
            try:
                provider = get_llm_provider(resolved_provider)
                ok, message = provider.healthcheck()
                models = provider.list_models() if ok else []
                provider_models[resolved_provider] = models
                provider_status[resolved_provider] = {'ok': ok, 'message': message}
            except Exception as provider_exc:
                provider_models[resolved_provider] = []
                provider_status[resolved_provider] = {'ok': False, 'message': str(provider_exc)}

        models = provider_models.get(requested_provider, [])

        from ...workflow.summarize import DEFAULT_MODEL
        from ...config import get_default_model

        response_data = {
            'models': models,
            'models_by_provider': provider_models,
            'provider_status': provider_status,
            'default': {
                'whisper': 'large-v3-turbo',
                'summarize': DEFAULT_MODEL,
                'embedding': get_default_model('EMBEDDING'),
                'provider': requested_provider,
            },
        }

        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(response_data, ensure_ascii=False).encode())

    except Exception as e:
        print(f'모델 목록 조회 중 오류: {e}')
        handler.send_response(500)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        error_response = {
            'error': '모델 목록을 조회할 수 없습니다. LLM provider 설정을 확인해주세요.',
            'details': str(e),
        }
        handler.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())


def _schedule_server_shutdown(handler) -> None:
    def shutdown_server():
        time.sleep(0.5)
        print('Client requested server shutdown. Stopping HTTP server...')
        handler.server.shutdown()

    threading.Thread(target=shutdown_server, daemon=True).start()


def handle_get(handler) -> bool:
    parsed = urlparse(handler.path)
    if parsed.path == '/models':
        query = parse_qs(parsed.query)
        provider_name = query.get('provider', [None])[0]
        _serve_available_models(handler, provider_name=provider_name)
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
    _schedule_server_shutdown(handler)
    handler.close_connection = True
    return True
