from __future__ import annotations

import json
import os
import re
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler

from ..ollama_utils import ensure_ollama_server
from ..vector_search import search as search_vectors
from ..server.routes import history as history_route
from ..server.routes import process as process_route
from ..server.routes import progress as progress_route

from .history import (
    add_upload_record,
    compute_file_hash,
    get_active_history,
    load_upload_history,
)
from .paths import UPLOAD_DIR, to_record_path
from .search import (
    build_highlight_snippet,
    collect_keyword_matches,
    collect_searchable_documents,
    get_document_text,
)
from .state import get_running_tasks
from .workflow import get_audio_duration, get_file_type
from .routes import admin_routes, file_routes, management_routes, records_routes, search_routes, similarity_routes


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

    def _parse_multipart(self, data, boundary):
        parts = data.split(f"--{boundary}".encode())
        files = {}

        for part in parts[1:-1]:
            if b"Content-Disposition" not in part:
                continue

            headers, body = part.split(b"\r\n\r\n", 1)
            headers = headers.decode("utf-8")
            body = body.rstrip(b"\r\n")

            if "filename=" in headers:
                filename_match = re.search(r'filename="([^"]*)"', headers)
                name_match = re.search(r'name="([^"]*)"', headers)

                if filename_match and name_match:
                    filename = filename_match.group(1)
                    name = name_match.group(1)
                    files.setdefault(name, []).append({"filename": filename, "data": body})

        return files

    def do_POST(self):
        if self.path == "/upload":
            try:
                print(f"Upload request received - Content-Length: {self.headers.get('Content-Length')}")
                print(f"Content-Type: {self.headers.get('Content-Type')}")

                content_type = self.headers.get("Content-Type", "")
                if not content_type.startswith("multipart/form-data"):
                    print("Upload failed: Not multipart/form-data")
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b"Invalid content type")
                    return

                boundary_match = re.search(r"boundary=([^;]+)", content_type)
                if not boundary_match:
                    print("Upload failed: No boundary found")
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b"No boundary found")
                    return

                boundary = boundary_match.group(1).strip()
                content_length = int(self.headers.get("Content-Length", 0))
                data = self.rfile.read(content_length)

                files = self._parse_multipart(data, boundary)
                print(f"Parsed fields: {list(files.keys())}")

                file_entries = files.get("files") or files.get("file")
                if not file_entries:
                    print("Upload failed: No files provided")
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b"No file uploaded")
                    return

                history = load_upload_history()
                uploaded_files = []
                for file_info in file_entries:
                    if not file_info.get("filename"):
                        continue

                    file_hash = compute_file_hash(file_info["data"])
                    existing = next((r for r in history if r.get("file_hash") == file_hash), None)
                    if existing:
                        uploaded_files.append(
                            {
                                "duplicate": True,
                                "original_record_id": existing["id"],
                                "filename": file_info["filename"],
                            }
                        )
                        continue

                    uid = uuid.uuid4().hex
                    save_dir = UPLOAD_DIR / uid
                    save_dir.mkdir(parents=True, exist_ok=True)
                    file_path = save_dir / os.path.basename(file_info["filename"])

                    with open(file_path, "wb") as output_file:
                        output_file.write(file_info["data"])

                    print(f"File saved successfully: {file_path}")

                    file_type = get_file_type(file_path)

                    duration = None
                    if file_type == "audio":
                        duration = get_audio_duration(file_path)

                    record = add_upload_record(file_path, file_type, duration, file_hash)
                    history.insert(0, record)

                    uploaded_files.append(
                        {
                            "file_path": to_record_path(file_path),
                            "file_type": file_type,
                            "record_id": record["id"],
                        }
                    )

                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(uploaded_files).encode())
                return

            except Exception as e:
                print(f"Upload error: {str(e)}")
                print(f"Exception type: {type(e).__name__}")
                import traceback

                traceback.print_exc()
                self.send_response(500)
                self.end_headers()
                self.wfile.write(f"Upload error: {str(e)}".encode())
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

