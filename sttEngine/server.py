"""Simple HTTP server for uploading files and running workflow steps.

The server exposes:
  * ``GET /`` – serve the upload HTML page.
  * ``POST /upload`` – accept an audio file and store it under ``DB/uploads/``.
  * ``POST /process`` – run selected workflow steps for the uploaded file.
  * ``GET /download/<file>`` – return processed files for download.

Only selected workflow steps return download links.
"""

from http.server import ThreadingHTTPServer, BaseHTTPRequestHandler
import json
import os
import subprocess
import sys
import uuid
from pathlib import Path
from typing import Any
import re
from urllib.parse import unquote
from datetime import datetime
import threading
import time

# Setup logging
try:
    from .logger import setup_logging
except ImportError:
    from logger import setup_logging

setup_logging()

# Import newly created modules
try:
    from .path_utils import normalize_record_path, to_record_path, resolve_record_path
    from .file_utils import get_file_type, get_audio_duration, compute_file_hash, file_hash, is_valid_uuid
    from .registry_manager import (
        load_file_registry,
        save_file_registry,
        register_file,
        get_file_by_uuid,
        resolve_file_identifier,
        migrate_existing_files,
    )
    from .history_manager import (
        load_upload_history,
        get_active_history,
        save_upload_history,
        add_upload_record,
        update_task_completion,
        update_title_summary,
        update_filename,
        TASK_TYPES,
    )
    from .task_manager import (
        register_process,
        unregister_process,
        cancel_task,
        is_task_cancelled,
        update_task_progress,
        get_task_progress,
        clear_task_progress,
        get_running_tasks,
    )
    from .websocket_server import broadcast_progress, start_websocket_server
    from .search_utils import _timestamp_to_sort_key, _collect_searchable_documents, _collect_keyword_matches, SEARCHABLE_SUFFIXES
    from .embedding_manager import (
        find_existing_stt_file,
        run_incremental_embedding,
        generate_embedding,
        generate_and_store_title_summary,
    )
    from .file_operations import (
        reset_upload_record,
        delete_file,
        delete_records,
        update_stt_text,
        reset_summary_and_embedding,
        reset_tasks_for_all_records,
    )
    from .workflow_runner import run_workflow
    from .config import (
        DB_ALIAS,
        get_db_base_path,
        get_default_model,
    )
except ImportError:
    from path_utils import normalize_record_path, to_record_path, resolve_record_path
    from file_utils import get_file_type, get_audio_duration, compute_file_hash, file_hash, is_valid_uuid
    from registry_manager import (
        load_file_registry,
        save_file_registry,
        register_file,
        get_file_by_uuid,
        resolve_file_identifier,
        migrate_existing_files,
    )
    from history_manager import (
        load_upload_history,
        get_active_history,
        save_upload_history,
        add_upload_record,
        update_task_completion,
        update_title_summary,
        update_filename,
        TASK_TYPES,
    )
    from task_manager import (
        register_process,
        unregister_process,
        cancel_task,
        is_task_cancelled,
        update_task_progress,
        get_task_progress,
        clear_task_progress,
        get_running_tasks,
    )
    from websocket_server import broadcast_progress, start_websocket_server
    from search_utils import _timestamp_to_sort_key, _collect_searchable_documents, _collect_keyword_matches, SEARCHABLE_SUFFIXES
    from embedding_manager import (
        find_existing_stt_file,
        run_incremental_embedding,
        generate_embedding,
        generate_and_store_title_summary,
    )
    from file_operations import (
        reset_upload_record,
        delete_file,
        delete_records,
        update_stt_text,
        reset_summary_and_embedding,
        reset_tasks_for_all_records,
    )
    from workflow_runner import run_workflow
    from config import (
        DB_ALIAS,
        get_db_base_path,
        get_default_model,
    )

try:
    from .vector_search import search as search_vectors
except ImportError:
    from vector_search import search as search_vectors

try:
    from .search_cache import cleanup_expired_cache, get_cache_stats, delete_cache_record
except ImportError:
    from search_cache import cleanup_expired_cache, get_cache_stats, delete_cache_record

try:
    from .llamacpp_utils import MODELS_DIR as GGUF_MODELS_DIR, check_model_available
except ImportError:
    from llamacpp_utils import MODELS_DIR as GGUF_MODELS_DIR, check_model_available

# PyInstaller 환경 감지 및 적절한 BASE_DIR 설정
if getattr(sys, 'frozen', False):
    # PyInstaller로 패키징된 경우: 실행 파일의 디렉토리 사용
    BASE_DIR = Path(sys.executable).parent.resolve()
else:
    # 개발 환경: 프로젝트 루트 사용
    BASE_DIR = Path(__file__).parent.parent.resolve()

DB_BASE_PATH = get_db_base_path(BASE_DIR)
UPLOAD_DIR = DB_BASE_PATH / "uploads"
OUTPUT_DIR = DB_BASE_PATH / "whisper_output"
VECTOR_DIR = DB_BASE_PATH / "vector_store"
HISTORY_FILE = DB_BASE_PATH / "upload_history.json"
FILE_REGISTRY_FILE = DB_BASE_PATH / "file_registry.json"
DELETED_DIR = DB_BASE_PATH / "deleted"
DELETED_UPLOAD_DIR = DELETED_DIR / "uploads"
DELETED_OUTPUT_DIR = DELETED_DIR / "whisper_output"
DELETED_VECTOR_DIR = DELETED_DIR / "vector_store"

# Wrapper functions for module functions - these adapt new module functions to work with global paths
def load_upload_history_wrapper():
    return load_upload_history(HISTORY_FILE)

def save_upload_history_wrapper(history):
    return save_upload_history(history, HISTORY_FILE)

def get_active_history_wrapper(history=None):
    return get_active_history(history, HISTORY_FILE)

def add_upload_record_wrapper(file_path, file_type, duration=None, file_hash=None):
    return add_upload_record(file_path, file_type, BASE_DIR, HISTORY_FILE, duration, file_hash)

def update_task_completion_wrapper(record_id, task, file_path):
    return update_task_completion(record_id, task, file_path, BASE_DIR, HISTORY_FILE, FILE_REGISTRY_FILE)

def update_title_summary_wrapper(record_id, summary):
    return update_title_summary(record_id, summary, HISTORY_FILE)

def update_filename_wrapper(record_id, new_filename):
    return update_filename(record_id, new_filename, HISTORY_FILE)

def load_file_registry_wrapper():
    return load_file_registry(FILE_REGISTRY_FILE)

def save_file_registry_wrapper(registry):
    return save_file_registry(registry, FILE_REGISTRY_FILE)

def register_file_wrapper(file_path, record_id, task_type, original_filename=None):
    return register_file(file_path, record_id, task_type, BASE_DIR, FILE_REGISTRY_FILE, original_filename)

def get_file_by_uuid_wrapper(file_uuid):
    return get_file_by_uuid(file_uuid, FILE_REGISTRY_FILE)

def migrate_existing_files_wrapper():
    return migrate_existing_files(BASE_DIR, FILE_REGISTRY_FILE, HISTORY_FILE, load_upload_history_wrapper, save_upload_history_wrapper)

def resolve_file_identifier_wrapper(file_identifier):
    return resolve_file_identifier(file_identifier, BASE_DIR, FILE_REGISTRY_FILE)

def normalize_record_path_wrapper(path_str):
    return normalize_record_path(path_str, BASE_DIR)

def to_record_path_wrapper(path):
    return to_record_path(path, BASE_DIR)

def resolve_record_path_wrapper(path_str):
    return resolve_record_path(path_str, BASE_DIR)

def find_existing_stt_file_wrapper(original_file_path):
    return find_existing_stt_file(original_file_path, OUTPUT_DIR)

def run_incremental_embedding_wrapper():
    return run_incremental_embedding(OUTPUT_DIR, VECTOR_DIR)

def generate_embedding_wrapper(file_path, record_id=None):
    return generate_embedding(file_path, VECTOR_DIR, BASE_DIR, HISTORY_FILE, FILE_REGISTRY_FILE, record_id)

def generate_and_store_title_summary_wrapper(record_id, file_path, model=None):
    return generate_and_store_title_summary(record_id, file_path, HISTORY_FILE, model)

def reset_upload_record_wrapper(record_id):
    return reset_upload_record(record_id, BASE_DIR, HISTORY_FILE, OUTPUT_DIR, VECTOR_DIR)

def delete_file_wrapper(file_identifier, file_type):
    return delete_file(file_identifier, file_type, BASE_DIR, HISTORY_FILE, FILE_REGISTRY_FILE)

def delete_records_wrapper(record_ids):
    return delete_records(
        record_ids, BASE_DIR, HISTORY_FILE, FILE_REGISTRY_FILE,
        UPLOAD_DIR, OUTPUT_DIR, VECTOR_DIR,
        DELETED_UPLOAD_DIR, DELETED_OUTPUT_DIR, DELETED_VECTOR_DIR,
        DB_ALIAS
    )

def update_stt_text_wrapper(file_identifier, new_text):
    return update_stt_text(file_identifier, new_text, BASE_DIR, HISTORY_FILE, FILE_REGISTRY_FILE, OUTPUT_DIR)

def reset_summary_and_embedding_wrapper(record_id):
    return reset_summary_and_embedding(record_id, BASE_DIR, HISTORY_FILE, FILE_REGISTRY_FILE, OUTPUT_DIR, VECTOR_DIR, DB_ALIAS)

def reset_tasks_for_all_records_wrapper(tasks):
    return reset_tasks_for_all_records(tasks, BASE_DIR, HISTORY_FILE, FILE_REGISTRY_FILE, OUTPUT_DIR, VECTOR_DIR, DB_ALIAS)

def _collect_searchable_documents_wrapper():
    return _collect_searchable_documents(BASE_DIR, FILE_REGISTRY_FILE)

def run_workflow_wrapper(file_path, steps, record_id=None, task_id=None, model_settings=None):
    return run_workflow(file_path, steps, BASE_DIR, OUTPUT_DIR, VECTOR_DIR, HISTORY_FILE, FILE_REGISTRY_FILE, record_id, task_id, model_settings)

def update_task_progress_wrapper(task_id, message):
    return update_task_progress(task_id, message, broadcast_progress)

# Assign wrapper functions to maintain compatibility
load_upload_history = load_upload_history_wrapper
save_upload_history = save_upload_history_wrapper
get_active_history = get_active_history_wrapper
add_upload_record = add_upload_record_wrapper
update_task_completion = update_task_completion_wrapper
update_title_summary = update_title_summary_wrapper
update_filename = update_filename_wrapper
load_file_registry = load_file_registry_wrapper
save_file_registry = save_file_registry_wrapper
register_file = register_file_wrapper
get_file_by_uuid = get_file_by_uuid_wrapper
migrate_existing_files = migrate_existing_files_wrapper
resolve_file_identifier = resolve_file_identifier_wrapper
normalize_record_path = normalize_record_path_wrapper
to_record_path = to_record_path_wrapper
resolve_record_path = resolve_record_path_wrapper
find_existing_stt_file = find_existing_stt_file_wrapper
run_incremental_embedding = run_incremental_embedding_wrapper
generate_embedding = generate_embedding_wrapper
generate_and_store_title_summary = generate_and_store_title_summary_wrapper
reset_upload_record = reset_upload_record_wrapper
delete_file = delete_file_wrapper
delete_records = delete_records_wrapper
update_stt_text = update_stt_text_wrapper
reset_summary_and_embedding = reset_summary_and_embedding_wrapper
reset_tasks_for_all_records = reset_tasks_for_all_records_wrapper
_collect_searchable_documents = _collect_searchable_documents_wrapper
run_workflow = run_workflow_wrapper
update_task_progress = update_task_progress_wrapper


class UploadHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        """Override to filter out successful HTTP requests (200)."""
        # Only log non-200 status codes
        message = format % args
        if not any(code in message for code in ['" 200 ', ' 200 ']):
            super().log_message(format, *args)
    def _serve_upload_page(self):
        try:
            with open(BASE_DIR / "frontend" / "upload.html", "rb") as f:
                content = f.read()
            self.send_response(200)
            self.send_header("Content-Type", "text/html; charset=utf-8")
            self.end_headers()
            self.wfile.write(content)
        except FileNotFoundError:
            self.send_response(404)
            self.end_headers()

    def _serve_static(self, filename: str, content_type: str):
        """Serve static frontend assets like CSS or JS files."""
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

    def _serve_download(self, file_identifier: str):
        # Check if it's a UUID (new system) or file path (legacy system)
        if self._is_uuid(file_identifier):
            # New UUID-based system
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
            # Legacy path-based system
            normalized_path = normalize_record_path(file_identifier)
            full_path = resolve_record_path(normalized_path)
            filename = os.path.basename(file_identifier) or full_path.name

        if full_path.exists():
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            
            # RFC 6266: Use UTF-8 encoding for non-ASCII filenames
            try:
                # Try ASCII encoding first
                filename.encode('ascii')
                self.send_header("Content-Disposition", f"attachment; filename={filename}")
            except UnicodeEncodeError:
                # Use UTF-8 encoding for non-ASCII filenames
                from urllib.parse import quote
                encoded_filename = quote(filename.encode('utf-8'))
                self.send_header("Content-Disposition", f"attachment; filename*=UTF-8''{encoded_filename}")
                
            self.end_headers()
            with open(full_path, "rb") as f:
                self.wfile.write(f.read())
        else:
            self.send_response(404)
            self.end_headers()

    def _is_uuid(self, test_string: str) -> bool:
        """Check if a string is a valid UUID."""
        try:
            uuid.UUID(test_string)
            return True
        except ValueError:
            return False

    def do_GET(self):
        if self.path == "/":
            self._serve_upload_page()
        elif self.path in ("/upload.css", "/upload.js"):
            content_type = "text/css" if self.path.endswith(".css") else "application/javascript"
            self._serve_static(self.path.lstrip("/"), content_type)
        elif self.path.startswith("/download/"):
            file_identifier = unquote(self.path[len("/download/"):])
            self._serve_download(file_identifier)
        elif self.path == "/history":
            self._serve_history()
        elif self.path == "/tasks":
            self._serve_running_tasks()
        elif self.path.startswith("/progress/"):
            task_id = self.path[len("/progress/"):]
            self._serve_task_progress(task_id)
        elif self.path.startswith("/file_search"):
            from urllib.parse import urlparse, parse_qs
            parsed = urlparse(self.path)
            params = parse_qs(parsed.query)
            query = params.get("q", [""])[0].lower()

            results = []
            if query:
                history = get_active_history()
                for record in history:
                    filename = record.get("filename", "")
                    tags = record.get("tags", [])
                    if query in filename.lower() or any(query in t.lower() for t in tags):
                        results.append({
                            "id": record.get("id"),
                            "filename": filename,
                            "tags": tags
                        })

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(results, ensure_ascii=False).encode())
        elif self.path.startswith("/search"):
            from urllib.parse import urlparse, parse_qs

            parsed = urlparse(self.path)
            params = parse_qs(parsed.query)
            query = params.get("q", [""])[0].strip()
            start_date = params.get("start", [None])[0]
            end_date = params.get("end", [None])[0]

            try:
                response_data = {
                    "keywordMatches": [],
                    "similarDocuments": []
                }

                if query:
                    documents, path_index = _collect_searchable_documents()
                    history = get_active_history()
                    history_map = {record.get("id"): record for record in history}

                    keyword_matches = _collect_keyword_matches(query, documents, history_map)
                    response_data["keywordMatches"] = keyword_matches

                    keyword_paths = {item["file"] for item in keyword_matches}
                    keyword_uuids = {item["file_uuid"] for item in keyword_matches}

                    hits = search_vectors(
                        query,
                        BASE_DIR,
                        top_k=10,
                        start_date=start_date,
                        end_date=end_date
                    )

                    similar_documents = []
                    for hit in hits:
                        rel_path = hit.get("file")
                        if not rel_path:
                            continue

                        doc = path_index.get(rel_path)
                        if doc and (doc["uuid"] in keyword_uuids or rel_path in keyword_paths):
                            continue  # Already listed in keyword matches
                        if not doc and rel_path in keyword_paths:
                            continue

                        display_name = Path(rel_path).name
                        link = f"/download/{rel_path}"
                        uploaded_at = None
                        source_filename = None
                        file_uuid = None

                        if doc:
                            record = history_map.get(doc["info"].get("record_id"), {})
                            uploaded_at = record.get("timestamp")
                            source_filename = record.get("filename")
                            display_name = doc["info"].get("original_filename") or display_name
                            link = f"/download/{doc['uuid']}"
                            file_uuid = doc["uuid"]

                        similar_documents.append({
                            "file_uuid": file_uuid,
                            "file": rel_path,
                            "display_name": display_name,
                            "score": hit.get("score"),
                            "uploaded_at": uploaded_at,
                            "source_filename": source_filename,
                            "link": link,
                        })

                        if len(similar_documents) >= 5:
                            break

                    response_data["similarDocuments"] = similar_documents

                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(response_data, ensure_ascii=False).encode())

            except Exception as e:
                print(f"검색 요청 처리 중 오류: {e}")
                self.send_response(500)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                error_response = {
                    "error": "검색 중 오류가 발생했습니다. Ollama 서버가 실행 중인지 확인하고, 임베딩 모델이 설치되어 있는지 확인해주세요.",
                    "details": str(e)
                }
                self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())
        elif self.path.startswith("/similar/"):
            # Extract file identifier from URL (can be UUID or file path)
            file_identifier = unquote(self.path[len("/similar/"):])
            self._serve_similar_documents(file_identifier)
        elif self.path == "/models":
            self._serve_available_models()
        elif self.path == "/cache/stats":
            self._serve_cache_stats()
        elif self.path == "/cache/cleanup":
            self._serve_cache_cleanup()
        else:
            self.send_response(404)
            self.end_headers()
    
    def _serve_history(self):
        """Serve upload history as JSON."""
        try:
            history = get_active_history()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(history, ensure_ascii=False).encode())
        except Exception as e:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(f"Error loading history: {str(e)}".encode())

    def _serve_running_tasks(self):
        """Serve information about currently running tasks."""
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

    def _serve_task_progress(self, task_id: str):
        """Serve progress information for a specific task."""
        try:
            progress = get_task_progress(task_id)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(progress, ensure_ascii=False).encode())
        except Exception as e:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(f"Error getting task progress: {str(e)}".encode())

    def _serve_similar_documents(self, file_identifier: str):
        """Find similar documents based on the provided file's content."""
        try:
            # Determine file path based on identifier (UUID or path)
            if self._is_uuid(file_identifier):
                # New UUID-based system
                file_info = get_file_by_uuid(file_identifier)
                if file_info:
                    file_path = normalize_record_path(file_info["file_path"])
                    full_path = resolve_record_path(file_path)
                    current_file_name = file_info["original_filename"]
                else:
                    self.send_response(404)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    error_response = {"error": "파일을 찾을 수 없습니다."}
                    self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())
                    return
            else:
                # Legacy path-based system
                file_path = normalize_record_path(file_identifier)
                full_path = resolve_record_path(file_path)
                current_file_name = os.path.basename(file_identifier) or full_path.name
            
            if not full_path.exists():
                self.send_response(404)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                error_response = {"error": "파일을 찾을 수 없습니다."}
                self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())
                return
            
            # Read file content to use as search query
            try:
                with open(full_path, 'r', encoding='utf-8') as f:
                    content = f.read()
            except UnicodeDecodeError:
                try:
                    with open(full_path, 'r', encoding='cp949') as f:
                        content = f.read()
                except UnicodeDecodeError:
                    with open(full_path, 'r', encoding='euc-kr') as f:
                        content = f.read()
            
            # Use the content to search for similar documents (top 6 to exclude self)
            print(f"[DEBUG] 유사 문서 검색 시작 - 현재 파일: {current_file_name}")
            hits = search_vectors(content, BASE_DIR, top_k=6)
            print(f"[DEBUG] 벡터 검색 결과: {len(hits)}개")
            for i, hit in enumerate(hits):
                print(f"[DEBUG] {i+1}. {hit['file']} (score: {hit['score']:.3f})")
            
            # Filter out the current document itself and limit to top 5
            similar_docs = []
            registry = load_file_registry()
            print(f"[DEBUG] 레지스트리에 등록된 파일 수: {len(registry)}")

            current_path_norm = os.path.normpath(normalize_record_path(file_path))
            for hit in hits:
                normalized_hit = normalize_record_path(hit["file"])
                hit_path_norm = os.path.normpath(normalized_hit)
                print(f"[DEBUG] 검토 중인 파일: {hit_path_norm} vs 현재 파일: {current_path_norm}")
                # Skip if it's the same file (compare normalized paths)
                if hit_path_norm != current_path_norm:
                    # Try to find UUID for this file in registry
                    file_uuid = None
                    for uuid_key, file_info in registry.items():
                        stored_norm = os.path.normpath(normalize_record_path(file_info["file_path"]))
                        if stored_norm == hit_path_norm:
                            file_uuid = uuid_key
                            break

                    # Use UUID if available, otherwise fallback to path
                    download_link = f"/download/{file_uuid}" if file_uuid else f"/download/{normalized_hit}"

                    similar_docs.append({
                        "file": normalized_hit,
                        "score": hit["score"],
                        "link": download_link
                    })
                    print(f"[DEBUG] 유사 문서 추가됨: {hit_path_norm} (score: {hit['score']:.3f})")
                else:
                    print(f"[DEBUG] 같은 파일로 제외됨: {hit_path_norm}")
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
                "details": str(e)
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())

    def _serve_similar_documents_with_filename(self, file_identifier: str, user_filename: str = None, refresh: bool = False):
        """Find similar documents with optional user filename for display."""
        try:
            # Determine file path based on identifier (UUID or path)
            if self._is_uuid(file_identifier):
                # New UUID-based system
                file_info = get_file_by_uuid(file_identifier)
                if file_info:
                    file_path = normalize_record_path(file_info["file_path"])
                    full_path = resolve_record_path(file_path)
                    current_file_name = user_filename or file_info["original_filename"]
                    print(f"[DEBUG] 유사문서 검색 - 파일 경로: {file_path}")
                    print(f"[DEBUG] 유사문서 검색 - 전체 경로: {full_path}")
                    print(f"[DEBUG] 유사문서 검색 - 파일 존재: {full_path.exists()}")
                else:
                    self.send_response(404)
                    self.send_header("Content-Type", "application/json")
                    self.end_headers()
                    error_response = {"error": "파일을 찾을 수 없습니다."}
                    self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())
                    return
            else:
                # Legacy path-based system
                file_path = normalize_record_path(file_identifier)
                full_path = resolve_record_path(file_path)
                current_file_name = user_filename or os.path.basename(file_identifier) or full_path.name
            
            if not full_path.exists():
                self.send_response(404)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                error_response = {"error": "파일을 찾을 수 없습니다."}
                self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())
                return
            
            # Read file content to use as search query
            try:
                with open(full_path, 'r', encoding='utf-8') as f:
                    content = f.read()
            except UnicodeDecodeError:
                try:
                    with open(full_path, 'r', encoding='cp949') as f:
                        content = f.read()
                except UnicodeDecodeError:
                    with open(full_path, 'r', encoding='euc-kr') as f:
                        content = f.read()
            
            # Use the content to search for similar documents (top 6 to exclude self)
            if refresh:
                delete_cache_record(content, 6)
            hits = search_vectors(content, BASE_DIR, top_k=6)
            
            # Filter out the current document itself and limit to top 5
            similar_docs = []
            registry = load_file_registry()
            history = load_upload_history()

            current_path_norm = os.path.normpath(normalize_record_path(file_path))
            for hit in hits:
                normalized_hit = normalize_record_path(hit["file"])
                hit_path_norm = os.path.normpath(normalized_hit)
                # Skip if it's the same file (compare normalized paths)
                if hit_path_norm != current_path_norm:
                    # Try to find UUID for this file in registry
                    file_uuid = None
                    record_id = None
                    for uuid_key, file_info in registry.items():
                        stored_norm = os.path.normpath(normalize_record_path(file_info["file_path"]))
                        if stored_norm == hit_path_norm:
                            file_uuid = uuid_key
                            record_id = file_info.get("record_id")
                            break

                    # Find user filename from history if available
                    user_filename_found = None
                    title_summary = ""
                    if record_id:
                        for record in history:
                            if record["id"] == record_id:
                                user_filename_found = record.get("filename")
                                title_summary = (record.get("title_summary") or "").strip()
                                break

                    # Use UUID if available, otherwise fallback to path
                    download_link = f"/download/{file_uuid}" if file_uuid else f"/download/{normalized_hit}"

                    # Use user filename if available, otherwise original filename
                    display_filename = user_filename_found or os.path.basename(normalized_hit)

                    similar_docs.append({
                        "file": normalized_hit,
                        "score": hit["score"],
                        "link": download_link,
                        "display_name": display_filename,
                        "title_summary": title_summary
                    })
                if len(similar_docs) >= 5:
                    break
            
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
                "details": str(e)
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())

    def _serve_available_models(self):
        """Serve available GGUF models as JSON."""
        try:
            # GGUF 모델 디렉토리에서 .gguf 파일 목록 가져오기
            models = []
            if GGUF_MODELS_DIR.exists():
                for model_file in GGUF_MODELS_DIR.glob("*.gguf"):
                    models.append(model_file.name)

            # 모델명으로 정렬
            models.sort()

            response_data = {
                "models": models,
                "default": {
                    "whisper": "large-v3-turbo",
                    "summarize": DEFAULT_MODEL,
                    "embedding": get_default_model("EMBEDDING")
                }
            }

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(response_data, ensure_ascii=False).encode())

        except Exception as e:
            error_str = str(e)
            print(f"모델 목록 조회 중 오류: {error_str}")
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            error_response = {
                "error": "모델 목록을 조회할 수 없습니다",
                "details": error_str
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())

    def _schedule_server_shutdown(self):
        """Schedule a graceful shutdown once the response is sent."""

        def shutdown_server():
            time.sleep(0.5)
            print("Client requested server shutdown. Stopping HTTP server...")
            self.server.shutdown()

        threading.Thread(target=shutdown_server, daemon=True).start()

    def _parse_multipart(self, data, boundary):
        """Simple multipart/form-data parser"""
        parts = data.split(f'--{boundary}'.encode())
        files = {}

        for part in parts[1:-1]:  # Skip first empty and last closing parts
            if b'Content-Disposition' not in part:
                continue

            headers, body = part.split(b'\r\n\r\n', 1)
            headers = headers.decode('utf-8')
            body = body.rstrip(b'\r\n')

            # Extract filename from Content-Disposition header
            if 'filename=' in headers:
                filename_match = re.search(r'filename="([^"]*)"', headers)
                name_match = re.search(r'name="([^"]*)"', headers)

                if filename_match and name_match:
                    filename = filename_match.group(1)
                    name = name_match.group(1)

                    files.setdefault(name, []).append({
                        'filename': filename,
                        'data': body
                    })

        return files

    def do_POST(self):
        if self.path == "/upload":
            try:
                print(f"Upload request received - Content-Length: {self.headers.get('Content-Length')}")
                print(f"Content-Type: {self.headers.get('Content-Type')}")
                
                content_type = self.headers.get('Content-Type', '')
                if not content_type.startswith('multipart/form-data'):
                    print("Upload failed: Not multipart/form-data")
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b"Invalid content type")
                    return
                
                # Extract boundary
                boundary_match = re.search(r'boundary=([^;]+)', content_type)
                if not boundary_match:
                    print("Upload failed: No boundary found")
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b"No boundary found")
                    return
                
                boundary = boundary_match.group(1).strip()
                content_length = int(self.headers.get('Content-Length', 0))
                data = self.rfile.read(content_length)
                
                files = self._parse_multipart(data, boundary)
                print(f"Parsed fields: {list(files.keys())}")

                file_entries = files.get('files') or files.get('file')
                if not file_entries:
                    print("Upload failed: No files provided")
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b"No file uploaded")
                    return

                history = load_upload_history()
                uploaded_files = []
                for file_info in file_entries:
                    if not file_info.get('filename'):
                        continue

                    file_hash = compute_file_hash(file_info['data'])
                    existing = next((r for r in history if r.get('file_hash') == file_hash), None)
                    if existing:
                        uploaded_files.append({
                            "duplicate": True,
                            "original_record_id": existing["id"],
                            "filename": file_info['filename']
                        })
                        continue

                    uid = uuid.uuid4().hex
                    save_dir = UPLOAD_DIR / uid
                    save_dir.mkdir(parents=True, exist_ok=True)
                    file_path = save_dir / os.path.basename(file_info['filename'])

                    with open(file_path, "wb") as output_file:
                        output_file.write(file_info['data'])

                    print(f"File saved successfully: {file_path}")

                    file_type = get_file_type(file_path)

                    # Get audio duration if it's an audio file
                    duration = None
                    if file_type == 'audio':
                        duration = get_audio_duration(file_path)

                    # Add to upload history
                    record = add_upload_record(file_path, file_type, duration, file_hash)
                    history.insert(0, record)

                    uploaded_files.append({
                        "file_path": to_record_path(file_path),
                        "file_type": file_type,
                        "record_id": record["id"]
                    })

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

        if self.path == "/process":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return
            file_path = payload.get("file_path")
            steps = payload.get("steps", [])
            record_id = payload.get("record_id")
            task_id = payload.get("task_id")  # Get task_id from frontend
            model_settings = payload.get("model_settings", {})  # Get model settings from frontend
            
            if not file_path:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing file_path")
                return
            
            # Generate task_id if not provided
            if not task_id:
                task_id = str(uuid.uuid4())

            print(f"Processing task {task_id} with steps {steps} and model settings {model_settings}")
            normalized_path = normalize_record_path(file_path)
            absolute_path = resolve_record_path(normalized_path)

            results = run_workflow(absolute_path, steps, record_id, task_id, model_settings)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(results).encode())
            return

        if self.path == "/cancel":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return
            task_id = payload.get("task_id")
            
            if not task_id:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing task_id")
                return
            
            success = cancel_task(task_id)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": success}).encode())
            return

        if self.path == "/shutdown":
            print("Shutdown request received via /shutdown endpoint")
            response_data = {
                "success": True,
                "message": "서버 종료 요청이 접수되었습니다. 잠시 후 서버가 종료됩니다."
            }
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(response_data, ensure_ascii=False).encode())
            self._schedule_server_shutdown()
            self.close_connection = True
            return

        if self.path == "/reset":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return
            record_id = payload.get("record_id")

            if not record_id:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing record_id")
                return

            success = reset_upload_record(record_id)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": success}).encode())
            return

        if self.path == "/update_filename":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return
            record_id = payload.get("record_id")
            new_filename = payload.get("filename")

            if not record_id or not new_filename:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing record_id or filename")
                return

            update_filename(record_id, new_filename)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"success": True}).encode())
            return

        if self.path == "/incremental_embedding":
            try:
                processed_count = run_incremental_embedding()
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "success": True,
                    "processed_count": processed_count,
                    "message": f"증분 임베딩 완료: {processed_count}개 파일 처리됨"
                }).encode())
            except Exception as e:
                self.send_response(500)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "success": False,
                    "error": str(e)
                }).encode())
            return

        if self.path == "/check_existing_stt":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return
            
            file_path = payload.get("file_path")
            if not file_path:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing file_path")
                return
            
            try:
                normalized_path = normalize_record_path(file_path)
                original_file = resolve_record_path(normalized_path)
                existing_stt = find_existing_stt_file(original_file)

                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "has_stt": existing_stt is not None,
                    "stt_file": to_record_path(existing_stt) if existing_stt else None
                }).encode())
            except Exception as e:
                self.send_response(500)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "has_stt": False,
                    "error": str(e)
                }).encode())
            return

        if self.path == "/update_stt_text":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"success": False, "error": "Invalid JSON payload"}).encode())
                return

            file_identifier = payload.get("file_identifier")
            content = payload.get("content", "")

            if not isinstance(content, str):
                content = str(content)

            success, message, record_id = update_stt_text(file_identifier, content)

            if success:
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "success": True,
                    "record_id": record_id
                }).encode())
            else:
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "success": False,
                    "error": message,
                    "record_id": record_id
                }).encode())
            return

        if self.path == "/reset_summary_embedding":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"success": False, "error": "Invalid JSON payload"}).encode())
                return

            record_id = payload.get("record_id")
            success, message = reset_summary_and_embedding(record_id)

            status_code = 200 if success else 400
            self.send_response(status_code)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({
                "success": success,
                "message": message
            }).encode())
            return

        if self.path == "/reset_all_tasks":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"success": False, "error": "Invalid JSON payload"}).encode())
                return

            tasks = payload.get("tasks")
            if not isinstance(tasks, list):
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"success": False, "error": "tasks 필드는 배열이어야 합니다."}).encode())
                return

            success, counts, message = reset_tasks_for_all_records(set(tasks))

            status_code = 200 if success else 400
            self.send_response(status_code)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({
                "success": success,
                "message": message,
                "counts": counts
            }).encode())
            return

        if self.path == "/similar":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return
            
            file_identifier = payload.get("file_identifier")
            user_filename = payload.get("user_filename")
            refresh = payload.get("refresh", False)

            if not file_identifier:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing file_identifier")
                return

            self._serve_similar_documents_with_filename(file_identifier, user_filename, refresh)
            return

        if self.path == "/delete":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return
            
            file_identifier = payload.get("file_identifier")
            file_type = payload.get("file_type")
            
            if not file_identifier or not file_type:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing file_identifier or file_type")
                return
            
            success, error_msg = delete_file(file_identifier, file_type)
            
            if success:
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"success": True}).encode())
            else:
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({"error": error_msg}).encode())
            return

        if self.path == "/delete_records":
            length = int(self.headers.get("Content-Length", 0))
            try:
                payload = json.loads(self.rfile.read(length)) if length else {}
            except json.JSONDecodeError:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Invalid JSON payload")
                return

            record_ids = payload.get("record_ids")
            if not isinstance(record_ids, list):
                self.send_response(400)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "success": False,
                    "error": "record_ids 필드는 배열이어야 합니다.",
                }).encode())
                return

            success, results = delete_records([str(r) for r in record_ids])
            status_code = 200 if success else 207
            self.send_response(status_code)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({
                "success": success,
                "results": results,
            }, ensure_ascii=False).encode())
            return

        self.send_response(404)
        self.end_headers()

    def _serve_cache_stats(self):
        """Serve cache statistics as JSON."""
        try:
            stats = get_cache_stats()
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(stats, ensure_ascii=False).encode())
        except Exception as e:
            self.send_response(500)
            self.end_headers()
            self.wfile.write(f"Error getting cache stats: {str(e)}".encode())

    def _serve_cache_cleanup(self):
        """Clean up expired cache entries and return cleanup stats."""
        try:
            cleaned_count = cleanup_expired_cache()
            response = {
                "success": True,
                "cleaned_entries": cleaned_count,
                "message": f"정리된 만료된 캐시 항목: {cleaned_count}개"
            }
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(response, ensure_ascii=False).encode())
        except Exception as e:
            self.send_response(500)
            self.end_headers()
            error_response = {
                "success": False,
                "error": f"캐시 정리 중 오류: {str(e)}"
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())


if __name__ == "__main__":
    import argparse
    import traceback
    import logging

    # 로그 파일 설정 (실행 파일과 같은 위치에 저장)
    log_file = BASE_DIR / "RecordRouteAPI_error.log"

    try:
        # Parse command-line arguments for Electron integration
        parser = argparse.ArgumentParser(description='RecordRoute STT Server')
        parser.add_argument('--ffmpeg_path', type=str, default='ffmpeg',
                            help='Path to ffmpeg executable')
        parser.add_argument('--models_path', type=str, default=None,
                            help='Path to models directory')
        args = parser.parse_args()

        # Set environment variables for other modules to access
        os.environ['FFMPEG_PATH'] = args.ffmpeg_path
        if args.models_path:
            os.environ['MODELS_PATH'] = args.models_path

        print(f"BASE_DIR: {BASE_DIR}")
        print(f"DB_BASE_PATH: {DB_BASE_PATH}")
        print(f"Creating directories...")

        UPLOAD_DIR.mkdir(parents=True, exist_ok=True)
        OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
        DELETED_UPLOAD_DIR.mkdir(parents=True, exist_ok=True)
        DELETED_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
        DELETED_VECTOR_DIR.mkdir(parents=True, exist_ok=True)

        print("Directories created successfully")
        print("Migrating existing files...")

        # Migrate existing files to UUID system
        migrate_existing_files()

        print("Migration complete")
        print("Starting WebSocket server...")

        # Start WebSocket server for progress updates
        ws_thread = threading.Thread(target=start_websocket_server, daemon=True)
        ws_thread.start()

        print("WebSocket server started")
        print("Starting HTTP server...")

        # Use ThreadingHTTPServer to allow concurrent request handling.
        # This lets the server respond to cancellation requests while
        # long-running tasks are processing in separate threads.
        server = ThreadingHTTPServer(("127.0.0.1", 8080), UploadHandler)
        print("Serving HTTP on 127.0.0.1 port 8080")
        print("Server is ready! Press Ctrl+C to stop.")

        try:
            server.serve_forever()
        except KeyboardInterrupt:
            print("\nShutting down server...")
        finally:
            server.server_close()
            print("Server closed")

    except Exception as e:
        error_msg = f"""
{'='*80}
RecordRouteAPI 시작 오류
{'='*80}
시간: {datetime.now().isoformat()}
오류 유형: {type(e).__name__}
오류 메시지: {str(e)}

상세 정보:
{'='*80}
{traceback.format_exc()}
{'='*80}

환경 정보:
- BASE_DIR: {BASE_DIR}
- DB_BASE_PATH: {DB_BASE_PATH}
- Python: {sys.version}
- Platform: {sys.platform}
- Frozen: {getattr(sys, 'frozen', False)}
{'='*80}
"""
        print(error_msg)

        # 로그 파일에 기록
        try:
            with open(log_file, 'a', encoding='utf-8') as f:
                f.write(error_msg)
            print(f"\n오류 로그가 '{log_file}' 파일에 저장되었습니다.")
        except Exception as log_error:
            print(f"로그 파일 저장 실패: {log_error}")

        # PyInstaller 환경에서는 콘솔 창이 닫히지 않도록 대기
        if getattr(sys, 'frozen', False):
            input("\n\n프로그램을 종료하려면 Enter 키를 누르세요...")
