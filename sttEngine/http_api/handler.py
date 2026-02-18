from __future__ import annotations

import json
import os
import threading
import time
from http.server import BaseHTTPRequestHandler

from ..providers.factory import get_llm_provider
from ..vector_search import search as search_vectors
from ..server.routes import history as history_route
from ..server.routes import process as process_route
from ..server.routes import progress as progress_route

from .history import get_active_history
from .search import (
    build_highlight_snippet,
    collect_keyword_matches,
    collect_searchable_documents,
    get_document_text,
)
from .state import get_running_tasks
from .routes import admin_routes, file_routes, management_routes, records_routes, search_routes, similarity_routes, upload_routes


class UploadHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        """Override to filter out successful HTTP requests (200)."""
        message = format % args
        if not any(code in message for code in ['" 200 ', ' 200 '] ):
            super().log_message(format, *args)

    def do_GET(self):
        if file_routes.handle_get(self):
            return
        if self.path == "/history":
            history_route.handle(self)
        elif self.path.startswith("/progress/"):
            task_id = self.path[len("/progress/") :]
            progress_route.handle(self, task_id)
        elif management_routes.handle_get(self):
            return
        elif search_routes.handle_get(self):
            return
        elif similarity_routes.handle_get(self):
            return
        elif admin_routes.handle_get(self):
            return
        else:
            self.send_response(404)
            self.end_headers()

    def _serve_running_tasks(self):
        try:
            tasks = get_running_tasks()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(tasks, ensure_ascii=False).encode())
        except Exception as e:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(f"Error getting running tasks: {str(e)}".encode())



    def _serve_available_models(self, provider_name: str | None = None):
        try:
            provider_aliases = {
                "llama.cpp": "llamacpp",
                "llama_cpp": "llamacpp",
                "llama-cpp": "llamacpp",
            }
            provider_models: dict[str, list[str]] = {"ollama": [], "llamacpp": []}
            provider_status: dict[str, dict[str, str | bool]] = {
                "ollama": {"ok": False, "message": "not_checked"},
                "llamacpp": {"ok": False, "message": "not_checked"},
            }

            configured_provider = (os.getenv("LLM_PROVIDER") or "ollama").strip().lower()
            active_provider = provider_aliases.get(configured_provider, configured_provider)
            normalized_requested_provider = (provider_name or "").strip().lower()
            requested_provider = provider_aliases.get(normalized_requested_provider, normalized_requested_provider or None)
            if requested_provider not in {"ollama", "llamacpp"}:
                requested_provider = active_provider if active_provider in {"ollama", "llamacpp"} else "ollama"

            for resolved_provider in (requested_provider,):
                try:
                    provider = get_llm_provider(resolved_provider)
                    ok, message = provider.healthcheck()
                    models = provider.list_models() if ok else []
                    provider_models[resolved_provider] = models
                    provider_status[resolved_provider] = {"ok": ok, "message": message}
                except Exception as provider_exc:
                    provider_models[resolved_provider] = []
                    provider_status[resolved_provider] = {"ok": False, "message": str(provider_exc)}

            models = provider_models.get(requested_provider, [])

            from ..workflow.summarize import DEFAULT_MODEL
            from ..config import get_default_model

            response_data = {
                "models": models,
                "models_by_provider": provider_models,
                "provider_status": provider_status,
                "default": {
                    "whisper": "large-v3-turbo",
                    "summarize": DEFAULT_MODEL,
                    "embedding": get_default_model("EMBEDDING"),
                    "provider": requested_provider,
                },
            }

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(response_data, ensure_ascii=False).encode())

        except Exception as e:
            print(f"모델 목록 조회 중 오류: {e}")
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            error_response = {
                "error": "모델 목록을 조회할 수 없습니다. LLM provider 설정을 확인해주세요.",
                "details": str(e),
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())

    def _schedule_server_shutdown(self):
        def shutdown_server():
            time.sleep(0.5)
            print("Client requested server shutdown. Stopping HTTP server...")
            self.server.shutdown()

        threading.Thread(target=shutdown_server, daemon=True).start()

    def do_POST(self):
        if upload_routes.handle_post(self):
            return

        if self.path == "/process":
            process_route.handle(self)
            return

        if management_routes.handle_post(self):
            return

        if records_routes.handle_post(self):
            return

        if similarity_routes.handle_post(self):
            return

        if admin_routes.handle_post(self):
            return

        self.send_response(404)
        self.end_headers()
