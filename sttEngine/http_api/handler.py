from __future__ import annotations

import json
import threading
import time
from http.server import BaseHTTPRequestHandler

from ..ollama_utils import ensure_ollama_server
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



    def _serve_available_models(self):
        try:
            server_ok, server_msg = ensure_ollama_server()
            if not server_ok:
                raise Exception(f"Ollama 서버를 사용할 수 없습니다: {server_msg}")

            import subprocess

            result = subprocess.run(["ollama", "list"], capture_output=True, text=True, timeout=10)
            if result.returncode != 0:
                raise Exception(f"Ollama list failed: {result.stderr}")

            lines = result.stdout.strip().split("\n")
            models = []

            for line in lines[1:] if len(lines) > 1 else lines:
                if line.strip():
                    parts = line.split()
                    if parts:
                        model_name = parts[0]
                        if "/" not in model_name and model_name not in ["mxbai-embed-large"]:
                            models.append(model_name)

            from ..workflow.summarize import DEFAULT_MODEL
            from ..config import get_default_model

            response_data = {
                "models": models,
                "default": {
                    "whisper": "large-v3-turbo",
                    "summarize": DEFAULT_MODEL,
                    "embedding": get_default_model("EMBEDDING"),
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
                "error": "모델 목록을 조회할 수 없습니다. Ollama가 실행 중인지 확인해주세요.",
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

