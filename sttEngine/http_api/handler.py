from __future__ import annotations

import json
import os
import re
import threading
import time
import uuid
from http.server import BaseHTTPRequestHandler
from pathlib import Path
from urllib.parse import unquote

from ..search_cache import delete_cache_record
from ..vector_search import search as search_vectors
from ..ollama_utils import ensure_ollama_server
from ..server.routes import history as history_route
from ..server.routes import process as process_route
from ..server.routes import progress as progress_route

from .history import (
    add_upload_record,
    compute_file_hash,
    get_active_history,
    load_upload_history,
)
from .paths import BASE_DIR, OUTPUT_DIR, UPLOAD_DIR, normalize_record_path, resolve_record_path, to_record_path
from .registry import get_file_by_uuid, load_file_registry
from .search import (
    build_highlight_snippet,
    collect_keyword_matches,
    collect_searchable_documents,
    get_document_text,
)
from .state import get_running_tasks
from .workflow import get_audio_duration, get_file_type
from .routes import admin_routes, management_routes, records_routes, search_routes, similarity_routes


class UploadHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        """Override to filter out successful HTTP requests (200)."""
        message = format % args
        if not any(code in message for code in ['" 200 ', ' 200 '] ):
            super().log_message(format, *args)

    CONTENT_TYPES = {
        ".html": "text/html; charset=utf-8",
        ".css": "text/css; charset=utf-8",
        ".js": "application/javascript; charset=utf-8",
        ".mjs": "application/javascript; charset=utf-8",
        ".json": "application/json; charset=utf-8",
        ".svg": "image/svg+xml",
        ".png": "image/png",
        ".jpg": "image/jpeg",
        ".jpeg": "image/jpeg",
        ".gif": "image/gif",
        ".ico": "image/x-icon",
        ".woff": "font/woff",
        ".woff2": "font/woff2",
        ".ttf": "font/ttf",
        ".eot": "application/vnd.ms-fontobject",
        ".map": "application/json",
    }

    def _serve_upload_page(self):
        """Serve React app index.html from frontend/dist/."""
        dist_index = BASE_DIR / "frontend" / "dist" / "index.html"
        legacy_index = BASE_DIR / "frontend" / "legacy" / "upload.html"
        try:
            target = dist_index if dist_index.exists() else legacy_index
            with open(target, "rb") as f:
                content = f.read()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(content)
        except FileNotFoundError:
            self.send_response(404)
            self.end_headers()

    def _serve_dist_file(self, rel_path: str):
        """Serve static files from frontend/dist/ (Vite build output)."""
        import posixpath

        rel_path = posixpath.normpath(rel_path).lstrip("/")
        file_path = BASE_DIR / "frontend" / "dist" / rel_path
        try:
            if not file_path.resolve().is_relative_to((BASE_DIR / "frontend" / "dist").resolve()):
                self.send_response(403)
                self.end_headers()
                return
            with open(file_path, "rb") as f:
                content = f.read()
            ext = file_path.suffix.lower()
            content_type = self.CONTENT_TYPES.get(ext, "application/octet-stream")
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            if "/assets/" in rel_path:
                self.send_header("Cache-Control", "public, max-age=31536000, immutable")
            self.end_headers()
            self.wfile.write(content)
        except FileNotFoundError:
            self.send_response(404)
            self.end_headers()

    def _serve_static(self, filename: str, content_type: str):
        """Serve static frontend assets (legacy fallback)."""
        try:
            with open(BASE_DIR / "frontend" / filename, "rb") as f:
                content = f.read()
            self.send_response(200)
            self.send_header("Content-Type", content_type)
            self.end_headers()
            self.wfile.write(content)
        except FileNotFoundError:
            self.send_response(404)
            self.end_headers()

    def _is_uuid(self, test_string: str) -> bool:
        try:
            uuid.UUID(test_string)
            return True
        except ValueError:
            return False

    def _serve_download(self, file_identifier: str):
        if self._is_uuid(file_identifier):
            file_info = get_file_by_uuid(file_identifier)
            if file_info:
                if file_info.get("deleted"):
                    self.send_response(404)
                    self.end_headers()
                    return
                file_path = normalize_record_path(file_info["file_path"])
                filename = file_info["original_filename"]
                full_path = resolve_record_path(file_path)
            else:
                self.send_response(404)
                self.end_headers()
                return
        else:
            normalized_path = normalize_record_path(file_identifier)
            full_path = resolve_record_path(normalized_path)
            filename = os.path.basename(file_identifier) or full_path.name

        if full_path.exists():
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            try:
                filename.encode("ascii")
                self.send_header("Content-Disposition", f"attachment; filename={filename}")
            except UnicodeEncodeError:
                from urllib.parse import quote

                encoded_filename = quote(filename.encode("utf-8"))
                self.send_header(
                    "Content-Disposition", f"attachment; filename*=UTF-8''{encoded_filename}"
                )
            self.end_headers()
            with open(full_path, "rb") as f:
                self.wfile.write(f.read())
        else:
            self.send_response(404)
            self.end_headers()

    def do_GET(self):
        if self.path == "/":
            self._serve_upload_page()
        elif self.path.startswith("/assets/"):
            self._serve_dist_file(self.path.lstrip("/"))
        elif self.path in ("/upload.css", "/upload.js"):
            content_type = "text/css" if self.path.endswith(".css") else "application/javascript"
            self._serve_static(self.path.lstrip("/"), content_type)
        elif self.path in ("/graph-view", "/graph-view.html"):
            self._serve_static("graph-view.html", "text/html; charset=utf-8")
        elif self.path in ("/favicon.ico", "/vite.svg"):
            self._serve_dist_file(self.path.lstrip("/"))
        elif self.path.startswith("/download/"):
            file_identifier = unquote(self.path[len("/download/") :])
            self._serve_download(file_identifier)
        elif self.path == "/history":
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

    def _resolve_text_file_for_similar(self, full_path: Path):
        TEXT_SUFFIXES = {".md", ".txt", ".text", ".markdown"}
        if full_path.suffix.lower() in TEXT_SUFFIXES:
            return full_path

        upload_uuid = full_path.parent.name
        stem = full_path.stem
        stt_output_dir = OUTPUT_DIR / upload_uuid

        if stt_output_dir.exists():
            candidates = [
                stt_output_dir / f"{stem}.summary.md",
                stt_output_dir / f"{stem}.corrected.md",
                stt_output_dir / f"{stem}.md",
            ]
            for candidate in candidates:
                if candidate.exists():
                    print(f"[DEBUG] 유사문서 검색용 텍스트 파일 찾음: {candidate}")
                    return candidate

            md_files = list(stt_output_dir.glob("*.md"))
            if md_files:
                chosen = md_files[0]
                print(f"[DEBUG] 유사문서 검색용 대체 텍스트 파일: {chosen}")
                return chosen

        print(f"[DEBUG] 유사문서 검색용 텍스트 파일을 찾지 못함: {full_path}")
        return None

    def _serve_similar_documents(self, file_identifier: str):
        try:
            if self._is_uuid(file_identifier):
                file_info = get_file_by_uuid(file_identifier)
                if file_info:
                    file_path = normalize_record_path(file_info["file_path"])
                    full_path = resolve_record_path(file_path)
                    current_file_name = file_info["original_filename"]
                    current_record_id = file_info.get("record_id")
                else:
                    self.send_response(404)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": "파일을 찾을 수 없습니다."}, ensure_ascii=False).encode())
                    return
            else:
                file_path = normalize_record_path(file_identifier)
                full_path = resolve_record_path(file_path)
                current_file_name = os.path.basename(file_identifier) or full_path.name
                current_record_id = None

            if not full_path.exists():
                self.send_response(404)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"error": "파일을 찾을 수 없습니다."}, ensure_ascii=False).encode())
                return

            text_path = self._resolve_text_file_for_similar(full_path)
            if text_path is None:
                raise ValueError(f"검색에 사용할 텍스트 파일을 찾을 수 없습니다: {full_path}")

            content = None
            encodings = ["utf-8", "utf-8-sig", "cp949", "euc-kr", "utf-16"]
            for enc in encodings:
                try:
                    with open(text_path, "r", encoding=enc) as f:
                        content = f.read()
                    break
                except UnicodeDecodeError:
                    continue

            if content is None:
                raise ValueError(f"텍스트 파일을 읽을 수 없습니다: {text_path}")

            print(f"[DEBUG] 유사 문서 검색 시작 - 현재 파일: {current_file_name}")
            hits = search_vectors(content, BASE_DIR, top_k=20)
            print(f"[DEBUG] 벡터 검색 결과: {len(hits)}개")
            for i, hit in enumerate(hits):
                print(f"[DEBUG] {i+1}. {hit['file']} (score: {hit['score']:.3f})")

            similar_docs = []
            registry = load_file_registry()
            history = load_upload_history()
            print(f"[DEBUG] 레지스트리에 등록된 파일 수: {len(registry)}")

            registry_path_map: dict[str, tuple[str, dict]] = {}
            for uuid_key, info in registry.items():
                if not isinstance(info, dict):
                    continue
                stored_norm = os.path.normpath(normalize_record_path(info.get("file_path", "")))
                if stored_norm:
                    registry_path_map[stored_norm] = (uuid_key, info)

            current_path_norm = os.path.normpath(normalize_record_path(file_path))
            current_text_path_norm = os.path.normpath(normalize_record_path(to_record_path(text_path)))
            if not current_record_id:
                for record in history:
                    try:
                        record_path_norm = os.path.normpath(normalize_record_path(record.get("file_path", "")))
                    except Exception:
                        continue
                    if record_path_norm == current_path_norm:
                        current_record_id = record.get("id")
                        break

            seen_record_ids: set[str] = set()
            seen_paths: set[str] = set()
            for hit in hits:
                normalized_hit = normalize_record_path(hit["file"])
                hit_path_norm = os.path.normpath(normalized_hit)
                print(f"[DEBUG] 검토 중인 파일: {hit_path_norm} vs 현재 파일: {current_path_norm}")

                uuid_and_info = registry_path_map.get(hit_path_norm)
                file_uuid = uuid_and_info[0] if uuid_and_info else None
                hit_info = uuid_and_info[1] if uuid_and_info else None
                hit_record_id = hit_info.get("record_id") if hit_info else None

                if hit_path_norm in {current_path_norm, current_text_path_norm}:
                    print(f"[DEBUG] 같은 파일로 제외됨: {hit_path_norm}")
                    continue

                if current_record_id and hit_record_id and hit_record_id == current_record_id:
                    print(f"[DEBUG] 같은 record_id로 제외됨: {hit_path_norm} (record_id: {hit_record_id})")
                    continue

                if hit_record_id and hit_record_id in seen_record_ids:
                    print(f"[DEBUG] 중복 record_id로 제외됨: {hit_path_norm} (record_id: {hit_record_id})")
                    continue

                if hit_path_norm in seen_paths:
                    print(f"[DEBUG] 중복 경로로 제외됨: {hit_path_norm}")
                    continue

                download_link = f"/download/{file_uuid}" if file_uuid else f"/download/{normalized_hit}"
                similar_docs.append(
                    {
                        "file": normalized_hit,
                        "score": hit["score"],
                        "link": download_link,
                        "record_id": hit_record_id,
                    }
                )
                if hit_record_id:
                    seen_record_ids.add(hit_record_id)
                seen_paths.add(hit_path_norm)
                print(f"[DEBUG] 유사 문서 추가됨: {hit_path_norm} (score: {hit['score']:.3f})")
                if len(similar_docs) >= 5:
                    break

            print(f"[DEBUG] 최종 유사 문서 수: {len(similar_docs)}")
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(similar_docs, ensure_ascii=False).encode())

        except Exception as e:
            print(f"유사 문서 검색 중 오류: {e}")
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            error_response = {
                "error": "유사 문서 검색 중 오류가 발생했습니다. 색인이 생성되어 있는지 확인해주세요.",
                "details": str(e),
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())

    def _serve_similar_documents_with_filename(self, file_identifier: str, user_filename: str = None, refresh: bool = False):
        try:
            if self._is_uuid(file_identifier):
                file_info = get_file_by_uuid(file_identifier)
                if file_info:
                    file_path = normalize_record_path(file_info["file_path"])
                    full_path = resolve_record_path(file_path)
                    current_file_name = user_filename or file_info["original_filename"]
                    current_record_id = file_info.get("record_id")
                    print(f"[DEBUG] 유사문서 검색 - 파일 경로: {file_path}")
                    print(f"[DEBUG] 유사문서 검색 - 전체 경로: {full_path}")
                    print(f"[DEBUG] 유사문서 검색 - 파일 존재: {full_path.exists()}")
                else:
                    self.send_response(404)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    self.wfile.write(json.dumps({"error": "파일을 찾을 수 없습니다."}, ensure_ascii=False).encode())
                    return
            else:
                file_path = normalize_record_path(file_identifier)
                full_path = resolve_record_path(file_path)
                current_file_name = user_filename or os.path.basename(file_identifier) or full_path.name
                current_record_id = None

            if not full_path.exists():
                self.send_response(404)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"error": "파일을 찾을 수 없습니다."}, ensure_ascii=False).encode())
                return

            text_path = self._resolve_text_file_for_similar(full_path)
            if text_path is None:
                raise ValueError(f"검색에 사용할 텍스트 파일을 찾을 수 없습니다: {full_path}")

            content = None
            encodings = ["utf-8", "utf-8-sig", "cp949", "euc-kr", "utf-16"]
            for enc in encodings:
                try:
                    with open(text_path, "r", encoding=enc) as f:
                        content = f.read()
                    break
                except UnicodeDecodeError:
                    continue

            if content is None:
                raise ValueError(f"텍스트 파일을 읽을 수 없습니다: {text_path}")

            if refresh:
                delete_cache_record(content, 20)

            hits = search_vectors(content, BASE_DIR, top_k=20)

            similar_docs = []
            registry = load_file_registry()
            history = load_upload_history()

            history_by_id: dict[str, dict] = {}
            for record in history:
                record_id = record.get("id")
                if record_id:
                    history_by_id[str(record_id)] = record

            registry_path_map: dict[str, tuple[str, dict]] = {}
            for uuid_key, info in registry.items():
                if not isinstance(info, dict):
                    continue
                stored_norm = os.path.normpath(normalize_record_path(info.get("file_path", "")))
                if stored_norm:
                    registry_path_map[stored_norm] = (uuid_key, info)

            current_path_norm = os.path.normpath(normalize_record_path(file_path))
            current_text_path_norm = os.path.normpath(normalize_record_path(to_record_path(text_path)))
            if not current_record_id:
                for record in history:
                    try:
                        record_path_norm = os.path.normpath(normalize_record_path(record.get("file_path", "")))
                    except Exception:
                        continue
                    if record_path_norm == current_path_norm:
                        current_record_id = record.get("id")
                        break

            seen_record_ids: set[str] = set()
            seen_paths: set[str] = set()
            for hit in hits:
                normalized_hit = normalize_record_path(hit["file"])
                hit_path_norm = os.path.normpath(normalized_hit)
                if hit_path_norm in {current_path_norm, current_text_path_norm}:
                    continue

                uuid_and_info = registry_path_map.get(hit_path_norm)
                file_uuid = uuid_and_info[0] if uuid_and_info else None
                hit_info = uuid_and_info[1] if uuid_and_info else None
                record_id = hit_info.get("record_id") if hit_info else None

                if current_record_id and record_id and record_id == current_record_id:
                    continue

                if record_id and record_id in seen_record_ids:
                    continue

                if hit_path_norm in seen_paths:
                    continue

                user_filename_found = None
                title_summary = ""
                if record_id:
                    record = history_by_id.get(str(record_id))
                    if record:
                        user_filename_found = record.get("filename")
                        title_summary = (record.get("title_summary") or "").strip()

                download_link = f"/download/{file_uuid}" if file_uuid else f"/download/{normalized_hit}"
                display_filename = user_filename_found or os.path.basename(normalized_hit)

                similar_docs.append(
                    {
                        "file": normalized_hit,
                        "score": hit["score"],
                        "link": download_link,
                        "display_name": display_filename,
                        "title_summary": title_summary,
                        "record_id": record_id,
                    }
                )
                if record_id:
                    seen_record_ids.add(record_id)
                seen_paths.add(hit_path_norm)
                if len(similar_docs) >= 5:
                    break

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(similar_docs, ensure_ascii=False).encode())

        except Exception as e:
            print(f"유사 문서 검색 중 오류: {e}")
            import traceback

            traceback.print_exc()
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            error_response = {
                "error": "유사 문서 검색 중 오류가 발생했습니다. 색인이 생성되어 있는지 확인해주세요.",
                "details": str(e),
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())

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

