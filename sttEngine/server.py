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

try:
    from .logger import setup_logging
except ImportError:  # pragma: no cover - fallback for script execution
    from logger import setup_logging

setup_logging()
from datetime import datetime
import threading
import time
import shutil
import hashlib
import asyncio
import websockets

try:
    from .workflow.transcribe import transcribe_audio_files
except ImportError:
    from workflow.transcribe import transcribe_audio_files

try:
    from .workflow.summarize import (
        summarize_text_mapreduce,
        read_text_with_fallback,
        save_output,
        DEFAULT_MODEL,
        DEFAULT_CHUNK_SIZE,
        DEFAULT_TEMPERATURE,
    )
except ImportError:
    from workflow.summarize import (
        summarize_text_mapreduce,
        read_text_with_fallback,
        save_output,
        DEFAULT_MODEL,
        DEFAULT_CHUNK_SIZE,
        DEFAULT_TEMPERATURE,
    )

try:
    from .config import (
        DB_ALIAS,
        get_db_base_path,
        get_default_model,
        normalize_db_record_path,
        resolve_db_path,
        to_db_record_path,
    )
except ImportError:
    from config import (
        DB_ALIAS,
        get_db_base_path,
        get_default_model,
        normalize_db_record_path,
        resolve_db_path,
        to_db_record_path,
    )

try:
    from .one_line_summary import generate_one_line_summary
except ImportError:
    from one_line_summary import generate_one_line_summary

try:
    from .vector_search import search as search_vectors
except ImportError:
    from vector_search import search as search_vectors

try:
    from .search_cache import cleanup_expired_cache, get_cache_stats, delete_cache_record
except ImportError:
    from search_cache import cleanup_expired_cache, get_cache_stats, delete_cache_record

try:
    from .embedding_pipeline import embed_text_ollama, load_index, save_index
except ImportError:
    from embedding_pipeline import embed_text_ollama, load_index, save_index

try:
    from .llamacpp_utils import MODELS_DIR as GGUF_MODELS_DIR, check_model_available
except ImportError:
    from llamacpp_utils import MODELS_DIR as GGUF_MODELS_DIR, check_model_available
import numpy as np
import os


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
SEARCHABLE_SUFFIXES = {".md", ".txt", ".text", ".markdown"}
TASK_TYPES = ("stt", "embedding", "summary")

# Global dictionary to track running processes
running_processes = {}
process_lock = threading.Lock()

# Global dictionary to track task progress
task_progress = {}
progress_lock = threading.Lock()

# WebSocket server setup for real-time progress updates
connected_clients = set()
websocket_loop = asyncio.new_event_loop()


def normalize_record_path(path_str: str) -> str:
    """Normalize stored record paths using the configured DB base path."""
    return normalize_db_record_path(path_str, BASE_DIR)


def to_record_path(path: Path) -> str:
    """Convert an absolute path to a stored record path."""
    return to_db_record_path(path, BASE_DIR)


def resolve_record_path(path_str: str) -> Path:
    """Resolve a stored record path to an absolute filesystem path."""
    return resolve_db_path(path_str, BASE_DIR)


async def _send_progress(task_id, message):
    data = json.dumps({"task_id": task_id, "message": message})
    if connected_clients:
        await asyncio.gather(
            *[client.send(data) for client in list(connected_clients) if not client.closed]
        )


def broadcast_progress(task_id, message):
    if websocket_loop.is_running():
        asyncio.run_coroutine_threadsafe(_send_progress(task_id, message), websocket_loop)


async def websocket_handler(websocket):
    connected_clients.add(websocket)
    try:
        async for _ in websocket:
            pass
    finally:
        connected_clients.discard(websocket)


def start_websocket_server():
    """Start the WebSocket server in its own asyncio event loop."""
    asyncio.set_event_loop(websocket_loop)

    async def run_server():
        async with websockets.serve(websocket_handler, "0.0.0.0", 8765):
            print("WebSocket server running on ws://localhost:8765")
            await asyncio.Future()  # run forever

    websocket_loop.run_until_complete(run_server())


def register_process(task_id: str, process):
    """Register a running process for a task."""
    with process_lock:
        running_processes[task_id] = {
            'process': process,
            'cancelled': False,
            'start_time': time.time()
        }
        print(f"Registered process for task {task_id}, PID: {process.pid}")


def unregister_process(task_id: str):
    """Unregister a process when it completes."""
    with process_lock:
        if task_id in running_processes:
            del running_processes[task_id]
            print(f"Unregistered process for task {task_id}")

def cancel_task(task_id: str):
    """Cancel a running task by terminating its process."""
    with process_lock:
        if task_id in running_processes:
            task_info = running_processes[task_id]
            task_info['cancelled'] = True
            process = task_info['process']
            
            try:
                print(f"Terminating process for task {task_id}, PID: {process.pid}")
                process.terminate()
                
                # Give it a moment to terminate gracefully
                try:
                    process.wait(timeout=5)
                    print(f"Process {process.pid} terminated gracefully")
                except subprocess.TimeoutExpired:
                    print(f"Process {process.pid} didn't terminate gracefully, killing...")
                    process.kill()
                    process.wait()
                    print(f"Process {process.pid} killed")
                    
            except Exception as e:
                print(f"Error terminating process for task {task_id}: {e}")
            
            return True
        else:
            print(f"Task {task_id} not found in running processes")
            return False


def is_task_cancelled(task_id: str):
    """Check if a task has been cancelled."""
    with process_lock:
        if task_id in running_processes:
            return running_processes[task_id]['cancelled']
        return False


def update_task_progress(task_id: str, message: str):
    """Update progress message for a task."""
    with progress_lock:
        task_progress[task_id] = {
            'message': message,
            'timestamp': time.time()
        }
        print(f"Task {task_id}: {message}")
    broadcast_progress(task_id, message)


def get_task_progress(task_id: str):
    """Get current progress for a task."""
    with progress_lock:
        return task_progress.get(task_id, {})


def clear_task_progress(task_id: str):
    """Clear progress for a completed/cancelled task."""
    with progress_lock:
        if task_id in task_progress:
            del task_progress[task_id]

def get_running_tasks():
    """Get information about currently running tasks."""
    with process_lock:
        return {
            task_id: {
                'pid': info['process'].pid,
                'start_time': info['start_time'],
                'cancelled': info['cancelled'],
                'duration': time.time() - info['start_time']
            }
            for task_id, info in running_processes.items()
        }


def get_file_type(file_path: Path):
    """Determine if the file is audio or text. 
    
    Returns:
        'audio' for audio files, 'text' for text files, 'pdf' for PDF files, 'unknown' for others.
    """
    audio_extensions = {'.flac', '.m4a', '.mp3', '.mp4', '.mpeg', '.mpga', '.oga', '.ogg', '.qta', '.wav', '.webm'}
    text_extensions = {'.md', '.txt', '.text', '.markdown'}
    pdf_extensions = {'.pdf'}

    suffix = file_path.suffix.lower()
    if suffix in audio_extensions:
        return 'audio'
    elif suffix in text_extensions:
        return 'text'
    elif suffix in pdf_extensions:
        return 'pdf'
    else:
        return 'unknown'


def get_audio_duration(file_path: Path):
    """Get audio file duration using ffprobe."""
    try:
        result = subprocess.run([
            'ffprobe', '-v', 'quiet', '-show_entries', 'format=duration',
            '-of', 'csv=p=0', str(file_path)
        ], capture_output=True, text=True, check=True)
        duration = float(result.stdout.strip())
        minutes = int(duration // 60)
        seconds = int(duration % 60)
        return f"{minutes:02d}:{seconds:02d}"
    except (subprocess.CalledProcessError, ValueError, FileNotFoundError):
        return None


def compute_file_hash(data: bytes) -> str:
    """Compute SHA256 hash for given file data."""
    return hashlib.sha256(data).hexdigest()


def _ensure_record_schema(record: dict) -> bool:
    """Ensure an upload history record has the expected structure."""
    updated = False

    completed = record.get("completed_tasks")
    if not isinstance(completed, dict):
        completed = {}
        record["completed_tasks"] = completed
        updated = True

    for task in TASK_TYPES:
        if task not in completed:
            completed[task] = False
            updated = True

    download_links = record.get("download_links")
    if not isinstance(download_links, dict):
        record["download_links"] = {}
        updated = True

    if not isinstance(record.get("deleted"), bool):
        record["deleted"] = False
        updated = True

    if "deleted_at" not in record:
        record["deleted_at"] = None
        updated = True

    if not isinstance(record.get("deleted_assets"), dict):
        record["deleted_assets"] = {}
        updated = True

    return updated


def load_upload_history():
    """Load upload history from JSON file and normalize record schema."""
    if HISTORY_FILE.exists():
        try:
            with open(HISTORY_FILE, 'r', encoding='utf-8') as f:
                history = json.load(f)

            if not isinstance(history, list):
                return []

            updated = False
            for record in history:
                if _ensure_record_schema(record):
                    updated = True

            if updated:
                save_upload_history(history)

            return history
        except (json.JSONDecodeError, IOError):
            return []
    return []


def get_active_history(history: list[dict] | None = None) -> list[dict]:
    """Return history entries that are not marked as deleted."""
    if history is None:
        history = load_upload_history()
    return [record for record in history if not record.get("deleted")]


def save_upload_history(history):
    """Save upload history to JSON file."""
    try:
        with open(HISTORY_FILE, 'w', encoding='utf-8') as f:
            json.dump(history, f, ensure_ascii=False, indent=2)
    except IOError:
        pass

def add_upload_record(file_path: Path, file_type: str, duration: str = None, file_hash: str = None):
    """Add a new upload record to history."""
    history = load_upload_history()

    record = {
        "id": str(uuid.uuid4()),
        "timestamp": datetime.now().isoformat(),
        "filename": file_path.name,
        "file_type": file_type,
        "duration": duration,
        "file_path": to_record_path(file_path),
        "folder_name": file_path.parent.name,  # UUID folder name
        "completed_tasks": {task: False for task in TASK_TYPES},
        "download_links": {},
        "title_summary": "",
        "tags": [],
        "file_hash": file_hash,
        "deleted": False,
        "deleted_at": None,
        "deleted_assets": {}
    }

    _ensure_record_schema(record)

    history.insert(0, record)  # Add to beginning (most recent first)

    # Keep only last 100 records
    if len(history) > 100:
        history = history[:100]

    save_upload_history(history)
    return record

def load_file_registry():
    """Load file registry from JSON file."""
    if FILE_REGISTRY_FILE.exists():
        try:
            with open(FILE_REGISTRY_FILE, 'r', encoding='utf-8') as f:
                registry = json.load(f)

            if isinstance(registry, dict):
                updated = False
                for info in registry.values():
                    if not isinstance(info, dict):
                        continue
                    if not isinstance(info.get("deleted"), bool):
                        info["deleted"] = False
                        updated = True
                    if "deleted_at" not in info:
                        info["deleted_at"] = None
                        updated = True
                if updated:
                    save_file_registry(registry)
                return registry
        except (json.JSONDecodeError, IOError):
            return {}
    return {}

def save_file_registry(registry):
    """Save file registry to JSON file."""
    try:
        with open(FILE_REGISTRY_FILE, 'w', encoding='utf-8') as f:
            json.dump(registry, f, ensure_ascii=False, indent=2)
    except IOError:
        pass


def is_valid_uuid(value: str) -> bool:
    """Check whether a string is a valid UUID value."""
    if not value:
        return False
    try:
        uuid.UUID(str(value))
        return True
    except (ValueError, TypeError):
        return False


def resolve_file_identifier(file_identifier: str):
    """Resolve a file identifier (UUID or path) to an absolute path and metadata."""
    if not file_identifier:
        return None, None, None, None

    identifier = file_identifier.strip()
    if identifier.startswith("/download/"):
        identifier = identifier[len("/download/"):]

    identifier = identifier.lstrip("/").replace('\\', '/')

    registry = load_file_registry()

    if is_valid_uuid(identifier):
        file_info = registry.get(identifier)
        if not file_info:
            return None, None, None, identifier

        file_path = normalize_record_path(file_info.get("file_path", ""))
        if not file_path:
            return None, file_info.get("record_id"), file_info.get("task_type"), identifier

        full_path = resolve_record_path(file_path)
        record_id = file_info.get("record_id")
        task_type = file_info.get("task_type")
        return full_path, record_id, task_type, identifier

    # Legacy path-based identifier
    file_path = normalize_record_path(identifier)
    if not file_path:
        return None, None, None, identifier

    full_path = resolve_record_path(file_path)
    record_id = None
    task_type = None
    resolved_identifier = identifier

    for uuid_key, info in registry.items():
        stored_path = normalize_record_path(info.get("file_path", ""))
        if resolve_record_path(stored_path) == full_path:
            record_id = info.get("record_id")
            task_type = info.get("task_type")
            resolved_identifier = uuid_key
            break

    return full_path, record_id, task_type, resolved_identifier


def _timestamp_to_sort_key(timestamp_str: str) -> float:
    """Convert ISO timestamp string to numeric sort key."""
    if not timestamp_str:
        return float('-inf')
    try:
        return datetime.fromisoformat(timestamp_str).timestamp()
    except ValueError:
        return float('-inf')


def _collect_searchable_documents():
    """Return documents eligible for keyword search and similarity mapping."""
    documents = []
    path_index = {}
    registry = load_file_registry()

    for file_uuid, info in registry.items():
        if isinstance(info, dict) and info.get("deleted"):
            continue
        rel_path = normalize_record_path(info.get("file_path"))
        if not rel_path:
            continue

        full_path = resolve_record_path(rel_path)
        if not full_path.exists():
            continue

        if full_path.suffix.lower() not in SEARCHABLE_SUFFIXES:
            continue

        doc = {
            "uuid": file_uuid,
            "info": info,
            "full_path": full_path,
            "relative_path": rel_path,
        }

        documents.append(doc)
        path_index.setdefault(rel_path, doc)

    return documents, path_index


def _collect_keyword_matches(query: str, documents, history_map, limit: int = 5):
    """Return top keyword matches sorted by frequency and recency."""
    if not query:
        return []

    pattern = re.compile(re.escape(query), re.IGNORECASE)
    matches = []

    for doc in documents:
        try:
            text = read_text_with_fallback(doc["full_path"])
        except Exception as exc:  # pragma: no cover - defensive logging
            print(f"키워드 검색을 위한 파일 읽기 실패 {doc['full_path']}: {exc}")
            continue

        count = len(pattern.findall(text))
        if count <= 0:
            continue

        record = history_map.get(doc["info"].get("record_id"), {})
        timestamp = record.get("timestamp")

        matches.append({
            "file_uuid": doc["uuid"],
            "file": doc["relative_path"],
            "display_name": doc["info"].get("original_filename") or Path(doc["relative_path"]).name,
            "count": count,
            "uploaded_at": timestamp,
            "source_filename": record.get("filename"),
            "link": f"/download/{doc['uuid']}",
        })

    matches.sort(key=lambda item: (-item["count"], -_timestamp_to_sort_key(item.get("uploaded_at"))))
    return matches[:limit]

def register_file(file_path: str, record_id: str, task_type: str, original_filename: str = None):
    """Register a file with UUID and return the file UUID."""
    registry = load_file_registry()
    file_uuid = str(uuid.uuid4())

    normalized_path = normalize_record_path(file_path)

    file_info = {
        "file_uuid": file_uuid,
        "file_path": normalized_path,
        "record_id": record_id,
        "task_type": task_type,
        "original_filename": original_filename or os.path.basename(normalized_path),
        "created_at": datetime.now().isoformat(),
        "deleted": False,
        "deleted_at": None
    }
    
    registry[file_uuid] = file_info
    save_file_registry(registry)
    return file_uuid

def get_file_by_uuid(file_uuid: str):
    """Get file info by UUID."""
    registry = load_file_registry()
    return registry.get(file_uuid)

def migrate_existing_files():
    """Migrate existing files from upload history to file registry."""
    history = load_upload_history()
    registry = load_file_registry()
    updated = False
    
    for record in history:
        if record.get("deleted"):
            continue
        record_id = record["id"]
        download_links = record.get("download_links", {})
        
        # Process each download link
        for task_type, download_url in download_links.items():
            if download_url.startswith("/download/"):
                file_path = normalize_record_path(download_url[10:])  # Remove "/download/" prefix

                # Check if this file is already registered
                already_registered = False
                for file_info in registry.values():
                    if normalize_record_path(file_info["file_path"]) == file_path and file_info["record_id"] == record_id:
                        already_registered = True
                        break

                if not already_registered:
                    # Register the file and update download link
                    full_path = resolve_record_path(file_path)
                    if full_path.exists():
                        file_uuid = register_file(file_path, record_id, task_type, os.path.basename(full_path))
                        # Update the download link to use UUID
                        record["download_links"][task_type] = f"/download/{file_uuid}"
                        updated = True
    
    if updated:
        save_upload_history(history)
        print("기존 파일들이 레지스트리에 등록되었습니다.")

def update_task_completion(record_id: str, task: str, file_path: str):
    """Update task completion status and register file with UUID."""
    history = load_upload_history()
    
    # Register the file and get UUID
    file_uuid = register_file(file_path, record_id, task)
    download_url = f"/download/{file_uuid}"
    
    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return file_uuid
            record["completed_tasks"][task] = True
            record["download_links"][task] = download_url
            break
    
    save_upload_history(history)
    return file_uuid

def update_title_summary(record_id: str, summary: str):
    """Store one-line summary for a record."""
    history = load_upload_history()
    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return
            record["title_summary"] = summary
            break
    save_upload_history(history)

def update_filename(record_id: str, new_filename: str):
    """Update filename for a record."""
    history = load_upload_history()
    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return
            record["filename"] = new_filename
            break
    save_upload_history(history)

def generate_and_store_title_summary(record_id: str, file_path: Path, model: str = None):
    """Generate one-line summary and store it."""
    try:
        summary = generate_one_line_summary(file_path, model=model)
        update_title_summary(record_id, summary)
    except Exception as e:
        print(f"One-line summary generation failed: {e}")

def find_existing_stt_file(original_file_path: Path):
    """Find existing STT result file for the given original file."""
    stem = original_file_path.stem
    
    # Extract UUID from the original file path (DB/uploads/UUID/filename)
    upload_uuid = original_file_path.parent.name
    print(f"[DEBUG] 업로드 UUID: {upload_uuid}")
    
    # Look for STT file in whisper_output/UUID/filename.md
    stt_output_dir = OUTPUT_DIR / upload_uuid
    potential_files = [
        stt_output_dir / f"{stem}.md",
        stt_output_dir / f"{stem}.corrected.md"
    ]
    
    for stt_file in potential_files:
        if stt_file.exists() and not stt_file.name.endswith('.summary.md'):
            print(f"[DEBUG] STT 파일 발견: {stt_file}")
            return stt_file
    
    print(f"[DEBUG] '{stem}.md' STT 파일을 찾지 못함 (경로: {stt_output_dir})")
    return None

def run_incremental_embedding(base_dir: Path = None):
    """Run incremental embedding on all existing STT result files."""
    if base_dir is None:
        base_dir = OUTPUT_DIR
    
    try:
        # Get embedding model name
        try:
            from sttEngine.config import get_model_for_task, get_default_model
            model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
        except:
            model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")
        
        # Load existing index
        index = load_index()
        processed_count = 0
        
        # Find all STT result files
        for md_file in base_dir.glob("**/*.md"):
            # Skip summary files
            if md_file.name.endswith('.summary.md'):
                continue
                
            # Check if already processed and up-to-date
            checksum = file_hash(md_file)
            key = str(md_file.resolve())
            if index.get(key, {}).get("sha256") == checksum:
                continue  # Already up-to-date
            
            try:
                # Read text content
                text = md_file.read_text(encoding="utf-8")
                
                # Generate embedding
                vector = embed_text_ollama(text, model_name)
                
                # Create vector directory if not exists
                VECTOR_DIR.mkdir(parents=True, exist_ok=True)
                
                # Save embedding vector with unique name
                vector_file = VECTOR_DIR / f"{md_file.parent.name}_{md_file.stem}.npy"
                np.save(vector_file, vector)
                
                # Update index
                index[key] = {
                    "sha256": checksum,
                    "vector": vector_file.name,
                    "deleted": False,
                    "deleted_path": None,
                    "vector_deleted_path": None,
                }
                
                processed_count += 1
                print(f"임베딩 생성 완료: {md_file.name}")
                
            except Exception as e:
                print(f"임베딩 생성 실패 {md_file.name}: {e}")
                continue
        
        # Save updated index
        save_index(index)
        print(f"증분 임베딩 완료: {processed_count}개 파일 처리됨")
        return processed_count
        
    except Exception as e:
        print(f"증분 임베딩 실행 실패: {e}")
        return 0

def file_hash(path: Path) -> str:
    """Return a stable SHA256 checksum for the given file."""
    import hashlib
    h = hashlib.sha256()
    with path.open("rb") as f:
        for chunk in iter(lambda: f.read(8192), b""):
            h.update(chunk)
    return h.hexdigest()

def generate_embedding(file_path: Path, record_id: str = None):
    """Generate embedding for a text file and store it."""
    try:
        # Get embedding model name
        try:
            from sttEngine.config import get_model_for_task, get_default_model
            model_name = get_model_for_task("EMBEDDING", get_default_model("EMBEDDING"))
        except:
            model_name = os.environ.get("EMBEDDING_MODEL", "bge-m3:latest")
        
        # Read text content
        text = file_path.read_text(encoding="utf-8")
        
        # Generate embedding
        vector = embed_text_ollama(text, model_name)
        
        # Create vector directory if not exists
        VECTOR_DIR.mkdir(parents=True, exist_ok=True)
        
        # Save embedding vector
        vector_file = VECTOR_DIR / f"{file_path.stem}.npy"
        np.save(vector_file, vector)
        
        # Update index
        index = load_index()
        checksum = file_hash(file_path)
        
        index[str(file_path.resolve())] = {
            "sha256": checksum,
            "vector": vector_file.name,
            "deleted": False,
            "deleted_path": None,
            "vector_deleted_path": None,
        }
        save_index(index)
        
        # Update task completion
        if record_id:
            file_path_str = to_record_path(file_path)
            update_task_completion(record_id, "embedding", file_path_str)
        
        print(f"Embedding generated for {file_path.name}")
        return True
        
    except Exception as e:
        print(f"Embedding generation failed for {file_path.name}: {e}")
        return False

def reset_upload_record(record_id: str) -> bool:
    """Remove processed files and reset completion status for a record."""
    history = load_upload_history()

    for record in history:
        if record["id"] == record_id:
            if record.get("deleted"):
                return False
            folder = record.get("folder_name")
            output_dir = OUTPUT_DIR / folder if folder else None
            try:
                if output_dir and output_dir.exists():
                    shutil.rmtree(output_dir)
            except Exception:
                pass

            # Remove embedding vectors and index entries related to this record
            if output_dir:
                index = load_index()
                keys_to_remove = []
                for key, meta in index.items():
                    try:
                        Path(key).resolve().relative_to(output_dir.resolve())
                        keys_to_remove.append((key, meta))
                    except ValueError:
                        continue

                for key, meta in keys_to_remove:
                    vector_name = meta.get("vector")
                    if vector_name:
                        vector_path = VECTOR_DIR / vector_name
                        if vector_path.exists():
                            # Check if this vector is referenced elsewhere
                            if not any(
                                v.get("vector") == vector_name and k != key
                                for k, v in index.items()
                            ):
                                try:
                                    vector_path.unlink()
                                except Exception:
                                    pass
                    del index[key]

                if keys_to_remove:
                    save_index(index)

            record["completed_tasks"] = {
                task: False for task in TASK_TYPES
            }
            record["download_links"] = {}
            record["title_summary"] = ""

            save_upload_history(history)
            return True

    return False

def delete_file(file_identifier: str, file_type: str) -> tuple[bool, str]:
    """Delete a specific file (STT or summary) and update history.

    Args:
        file_identifier: File UUID from download URL
        file_type: Type of file ('stt' or 'summary')
    
    Returns:
        Tuple of (success, error_message)
    """
    try:
        # Get file info by UUID
        file_info = get_file_by_uuid(file_identifier)
        if not file_info:
            return False, "파일을 찾을 수 없습니다."
        
        # Get the actual file path
        file_path = Path(BASE_DIR) / file_info["file_path"]
        
        if not file_path.exists():
            return False, "파일이 존재하지 않습니다."
        
        # Verify file type matches
        if file_type == 'stt' and not file_path.name.endswith('.md'):
            return False, "STT 파일이 아닙니다."
        elif file_type == 'summary' and not file_path.name.endswith('.summary.md'):
            return False, "요약 파일이 아닙니다."
        
        # Delete the file
        try:
            file_path.unlink()
        except Exception as e:
            print(f"Failed to delete {file_path}: {e}")
            return False, "파일 삭제에 실패했습니다."
        
        # Update history record
        history = load_upload_history()
        record_id = file_info["record_id"]
        
        for record in history:
            if record["id"] == record_id:
                if record.get("deleted"):
                    return False, "삭제된 항목입니다."
                # Update completion status
                record["completed_tasks"][file_type] = False
                
                # Remove download link
                if file_type in record["download_links"]:
                    del record["download_links"][file_type]
                
                # If deleting summary, also clear title_summary
                if file_type == 'summary':
                    record["title_summary"] = ""
                
                break
        
        # Remove from file registry
        registry = load_file_registry()
        if file_identifier in registry:
            del registry[file_identifier]
            save_file_registry(registry)
        
        # Save updated history
        save_upload_history(history)
        
        return True, ""
        
    except Exception as e:
        print(f"Error deleting file: {e}")
        return False, f"삭제 중 오류가 발생했습니다: {str(e)}"


def _delete_single_record_assets(
    record: dict,
    registry: dict,
    index: dict,
    moved_vector_names: set[str],
) -> dict:
    """Move record assets to the deleted area and update metadata."""

    record_id = record.get("id")
    folder_name = record.get("folder_name")
    deleted_at = datetime.now().isoformat()

    upload_dir = (UPLOAD_DIR / folder_name).resolve() if folder_name else None
    deleted_upload_dir = (DELETED_UPLOAD_DIR / folder_name).resolve() if folder_name else None
    output_dir = (OUTPUT_DIR / folder_name).resolve() if folder_name else None
    deleted_output_dir = (DELETED_OUTPUT_DIR / folder_name).resolve() if folder_name else None
    vector_dir = VECTOR_DIR.resolve()
    deleted_vector_dir = DELETED_VECTOR_DIR.resolve()

    registry_changed = False
    index_changed = False

    files_assets: dict[str, list[str]] = {}
    record_assets: dict[str, Any] = {}

    registry_updates: list[tuple[str, dict, Path]] = []
    for file_uuid, info in (registry or {}).items():
        if not isinstance(info, dict):
            continue
        if info.get("record_id") != record_id:
            continue

        file_path_str = info.get("file_path")
        if not file_path_str:
            continue

        try:
            absolute_path = resolve_record_path(file_path_str)
        except Exception:
            absolute_path = None

        new_path: Path | None = None
        if absolute_path is not None:
            if upload_dir:
                try:
                    rel = absolute_path.relative_to(upload_dir)
                    new_path = deleted_upload_dir / rel
                except ValueError:
                    pass
            if new_path is None and output_dir:
                try:
                    rel = absolute_path.relative_to(output_dir)
                    new_path = deleted_output_dir / rel
                except ValueError:
                    pass
            if new_path is None:
                try:
                    rel = absolute_path.relative_to(vector_dir)
                    new_path = deleted_vector_dir / rel
                except ValueError:
                    pass

        if new_path is None:
            continue

        registry_updates.append((file_uuid, info, new_path))

    index_entries: list[tuple[str, dict, Path]] = []
    vector_names: set[str] = set()
    if output_dir:
        for key, meta in (index or {}).items():
            if not isinstance(meta, dict):
                continue
            try:
                rel = Path(key).resolve().relative_to(output_dir)
            except (ValueError, FileNotFoundError):
                continue

            deleted_path = (deleted_output_dir / rel) if deleted_output_dir else None
            if deleted_path is None:
                continue

            index_entries.append((key, meta, deleted_path))

            vector_name = meta.get("vector")
            if vector_name:
                vector_names.add(vector_name)

    if upload_dir and upload_dir.exists():
        deleted_upload_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(upload_dir), str(deleted_upload_dir))
        record_assets["uploads"] = to_record_path(deleted_upload_dir)

    if output_dir and output_dir.exists():
        deleted_output_dir.parent.mkdir(parents=True, exist_ok=True)
        shutil.move(str(output_dir), str(deleted_output_dir))
        record_assets["outputs"] = to_record_path(deleted_output_dir)

    for file_uuid, info, new_path in registry_updates:
        info["file_path"] = to_record_path(new_path)
        info["deleted"] = True
        info["deleted_at"] = deleted_at
        task_type = info.get("task_type")
        if task_type:
            files_assets.setdefault(task_type, []).append(info["file_path"])
        registry_changed = True

    if files_assets:
        record_assets["files"] = files_assets

    moved_vector_paths: list[str] = []
    for key, meta, deleted_path in index_entries:
        meta["deleted"] = True
        meta["deleted_at"] = deleted_at
        meta["deleted_path"] = str(deleted_path)
        vector_name = meta.get("vector")
        if vector_name:
            deleted_vector_path = DELETED_VECTOR_DIR / vector_name
            meta["vector_deleted_path"] = str(deleted_vector_path)
        index_changed = True

    for vector_name in vector_names:
        target_path = DELETED_VECTOR_DIR / vector_name
        if vector_name not in moved_vector_names:
            source_path = VECTOR_DIR / vector_name
            target_path.parent.mkdir(parents=True, exist_ok=True)
            if source_path.exists():
                shutil.move(str(source_path), str(target_path))
            moved_vector_names.add(vector_name)
        moved_vector_paths.append(to_record_path(target_path))

    if moved_vector_paths:
        record_assets["vectors"] = moved_vector_paths

    record["deleted"] = True
    record["deleted_at"] = deleted_at
    record["deleted_assets"] = record_assets

    return {
        "registry_changed": registry_changed,
        "index_changed": index_changed,
    }


def delete_records(record_ids: list[str]) -> tuple[bool, dict[str, dict]]:
    """Delete multiple upload records by moving their assets to a deleted folder."""

    if not record_ids:
        return False, {}

    history = load_upload_history()
    registry = load_file_registry()
    index = load_index()

    history_by_id = {record.get("id"): record for record in history}
    results: dict[str, dict] = {}

    history_changed = False
    registry_changed = False
    index_changed = False
    moved_vector_names: set[str] = set()

    for record_id in record_ids:
        record = history_by_id.get(record_id)
        if not record:
            results[record_id] = {
                "success": False,
                "error": "기록을 찾을 수 없습니다.",
            }
            continue

        if record.get("deleted"):
            results[record_id] = {
                "success": False,
                "error": "이미 삭제된 항목입니다.",
            }
            continue

        try:
            summary = _delete_single_record_assets(record, registry, index, moved_vector_names)
            history_changed = True
            registry_changed = registry_changed or summary.get("registry_changed", False)
            index_changed = index_changed or summary.get("index_changed", False)
            results[record_id] = {"success": True}
        except Exception as exc:
            results[record_id] = {
                "success": False,
                "error": str(exc),
            }

    if history_changed:
        save_upload_history(history)
    if registry_changed:
        save_file_registry(registry)
    if index_changed:
        save_index(index)

    overall_success = (
        bool(results)
        and all(result.get("success") for result in results.values())
    )
    return overall_success, results


def update_stt_text(file_identifier: str, new_text: str) -> tuple[bool, str, str | None]:
    """Update the contents of an STT result file."""

    if not file_identifier:
        return False, "파일 식별자가 필요합니다.", None

    file_path, record_id, task_type, _ = resolve_file_identifier(file_identifier)

    if not file_path or not file_path.exists():
        return False, "파일을 찾을 수 없습니다.", record_id

    if file_path.name.endswith('.summary.md'):
        return False, "요약 파일은 수정할 수 없습니다.", record_id

    if task_type and task_type not in ("stt", "embedding"):
        return False, "STT 파일만 수정할 수 있습니다.", record_id

    if file_path.suffix.lower() not in {'.md', '.txt', '.text', '.markdown'}:
        return False, "지원하지 않는 파일 형식입니다.", record_id

    try:
        file_path.write_text(new_text, encoding='utf-8')
    except Exception as exc:
        print(f"Failed to write updated STT text: {exc}")
        return False, "텍스트를 저장하지 못했습니다.", record_id

    if not record_id:
        history = load_upload_history()
        resolved_path = file_path.resolve()
        for record in history:
            folder = record.get("folder_name")
            if not folder:
                continue
            output_dir = (OUTPUT_DIR / folder).resolve()
            try:
                resolved_path.relative_to(output_dir)
                record_id = record["id"]
                break
            except ValueError:
                continue

    return True, "", record_id


def reset_tasks_for_record(
    record: dict,
    tasks: set[str],
    registry: dict,
    index: dict,
) -> tuple[dict[str, bool], bool, bool]:
    """Reset selected task artifacts for a single record."""

    results = {task: False for task in TASK_TYPES}
    if not record or not tasks or record.get("deleted"):
        return results, False, False

    download_links = record.get("download_links", {})
    completed_tasks = record.setdefault("completed_tasks", {task: False for task in TASK_TYPES})

    registry_changed = False
    index_changed = False

    def cleanup_task(task_name: str, delete_file: bool) -> bool:
        nonlocal registry_changed

        link = download_links.get(task_name)
        if not link:
            return False

        file_path, _, _, resolved_identifier = resolve_file_identifier(link)

        if delete_file and file_path and file_path.exists():
            try:
                file_path.unlink()
            except Exception:
                pass

        if resolved_identifier and is_valid_uuid(resolved_identifier):
            entry = registry.get(resolved_identifier)
            if entry and entry.get("task_type") == task_name:
                del registry[resolved_identifier]
                registry_changed = True
        else:
            if file_path:
                try:
                    relative_path = to_record_path(file_path)
                except Exception:
                    relative_path = file_path.as_posix()

                normalized = normalize_record_path(relative_path)
                candidates = {relative_path, normalized}
                if normalized.startswith(f"{DB_ALIAS}/"):
                    candidates.add(normalized[len(DB_ALIAS) + 1 :])

                for key, info in list(registry.items()):
                    stored_path = normalize_record_path(info.get("file_path", ""))
                    if info.get("task_type") == task_name and stored_path in candidates:
                        del registry[key]
                        registry_changed = True

        download_links.pop(task_name, None)
        completed_tasks[task_name] = False

        if task_name == "summary":
            record["title_summary"] = ""

        return True

    if "summary" in tasks and cleanup_task("summary", delete_file=True):
        results["summary"] = True

    embedding_removed = False
    if "embedding" in tasks and cleanup_task("embedding", delete_file=False):
        results["embedding"] = True
        embedding_removed = True

    stt_removed = False
    if "stt" in tasks and cleanup_task("stt", delete_file=True):
        results["stt"] = True
        stt_removed = True

    if embedding_removed:
        try:
            folder_name = record.get("folder_name", "")
            output_dir = OUTPUT_DIR / folder_name
            output_resolved = output_dir.resolve()
            keys_to_remove = []

            for key, meta in list(index.items()):
                try:
                    Path(key).resolve().relative_to(output_resolved)
                    keys_to_remove.append((key, meta))
                except (ValueError, FileNotFoundError):
                    continue

            if keys_to_remove:
                for key, meta in keys_to_remove:
                    vector_name = meta.get("vector")
                    if vector_name:
                        vector_path = VECTOR_DIR / vector_name
                        if vector_path.exists():
                            if not any(
                                v.get("vector") == vector_name and k != key
                                for k, v in index.items()
                            ):
                                try:
                                    vector_path.unlink()
                                except Exception:
                                    pass
                    del index[key]

                index_changed = True
        except Exception as exc:
            print(f"Failed to clean embedding vectors: {exc}")

    if stt_removed:
        try:
            original_path = record.get("file_path")
            folder_name = record.get("folder_name")
            if original_path and folder_name:
                source_path = resolve_record_path(original_path)
                output_dir = OUTPUT_DIR / folder_name
                if source_path and output_dir.exists():
                    stem = Path(source_path).stem
                    corrected_file = output_dir / f"{stem}.corrected.md"
                    if corrected_file.exists():
                        try:
                            corrected_file.unlink()
                        except Exception:
                            pass
        except Exception as exc:
            print(f"Failed to clean STT artifacts: {exc}")

    return results, registry_changed, index_changed


def reset_summary_and_embedding(record_id: str) -> tuple[bool, str]:
    """Reset summary and embedding artifacts for a record."""

    if not record_id:
        return False, "record_id가 필요합니다."

    history = load_upload_history()
    record = next((item for item in history if item.get("id") == record_id), None)

    if not record:
        return False, "기록을 찾을 수 없습니다."

    registry = load_file_registry()
    index = load_index()

    results, registry_changed, index_changed = reset_tasks_for_record(
        record,
        {"summary", "embedding"},
        registry,
        index,
    )

    if registry_changed:
        save_file_registry(registry)

    if index_changed:
        save_index(index)

    save_upload_history(history)

    summary_reset = results.get("summary", False)
    embedding_reset = results.get("embedding", False)

    if summary_reset and embedding_reset:
        message = "색인과 요약이 초기화되었습니다."
    elif summary_reset:
        message = "요약이 초기화되었습니다."
    elif embedding_reset:
        message = "색인이 초기화되었습니다."
    else:
        message = "초기화할 항목이 없습니다."

    return True, message


def reset_tasks_for_all_records(tasks: set[str]) -> tuple[bool, dict[str, int], str]:
    """Reset selected task artifacts for every record in history."""

    valid_tasks = set(TASK_TYPES)
    requested_tasks = {task for task in tasks if task in valid_tasks}

    if not requested_tasks:
        return False, {task: 0 for task in valid_tasks}, "유효한 초기화 항목을 선택해주세요."

    history = load_upload_history()
    if not history:
        return True, {task: 0 for task in valid_tasks}, "초기화할 기록이 없습니다."

    registry = load_file_registry()
    index = load_index()

    registry_changed = False
    index_changed = False
    reset_counts = {task: 0 for task in valid_tasks}

    for record in history:
        if record.get("deleted"):
            continue
        results, reg_changed, idx_changed = reset_tasks_for_record(
            record,
            requested_tasks,
            registry,
            index,
        )

        if reg_changed:
            registry_changed = True
        if idx_changed:
            index_changed = True

        for task in requested_tasks:
            if results.get(task):
                reset_counts[task] += 1

    if registry_changed:
        save_file_registry(registry)

    if index_changed:
        save_index(index)

    save_upload_history(history)

    labels = {"stt": "STT", "embedding": "색인", "summary": "요약"}
    summary_parts = [
        f"{labels[task]} {reset_counts[task]}건"
        for task in requested_tasks
        if reset_counts.get(task)
    ]

    if not summary_parts:
        message = "초기화할 항목이 없습니다."
    else:
        message = ", ".join(summary_parts) + " 초기화되었습니다."

    return True, reset_counts, message


def run_workflow(file_path: Path, steps, record_id: str = None, task_id: str = None, model_settings: dict = None):
    """Run the requested workflow steps sequentially.

    Args:
        file_path: Path to the uploaded audio or text file.
        steps: list of step names, e.g. ["stt", "correct", "summary"].
        record_id: Upload record ID for updating history.
        task_id: Unique task ID for tracking and cancellation.

    Returns:
        Dict mapping step name to download URL.
    """

    results = {}
    current_file = file_path
    file_type = get_file_type(file_path)
    
    # Create individual output directory based on upload folder structure
    upload_folder_name = current_file.parent.name  # Get UUID folder name
    individual_output_dir = OUTPUT_DIR / upload_folder_name
    individual_output_dir.mkdir(exist_ok=True)

    try:
        # For text files, skip STT step and copy to output directory
        if file_type == 'text':
            if "stt" in steps:
                # Check if task was cancelled
                if task_id and is_task_cancelled(task_id):
                    return {"error": "Task was cancelled"}
                    
                # For text files, we already have the text content, so just copy it to output
                text_file = individual_output_dir / f"{file_path.stem}.md"
                # Copy the text file to output directory with .md extension
                import shutil
                shutil.copy2(file_path, text_file)
                download_url = f"/download/{upload_folder_name}/{text_file.name}"
                results["stt"] = download_url
                current_file = text_file
                
                # Update history
                if record_id:
                    file_path_str = to_record_path(text_file)
                    update_task_completion(record_id, "stt", file_path_str)
            else:
                # If no STT step for text file, use the original file as starting point
                # Copy to output directory for consistency
                text_file = individual_output_dir / f"{file_path.stem}.md"
                import shutil
                shutil.copy2(file_path, text_file)
                current_file = text_file
            

        # For PDF files, extract text and treat as markdown
        elif file_type == 'pdf':
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            try:
                from pypdf import PdfReader
                reader = PdfReader(str(current_file))
                pdf_text = "\n".join(page.extract_text() or "" for page in reader.pages)
            except Exception as e:
                print(f"PDF text extraction failed: {e}")
                return {"error": f"PDF text extraction failed: {e}"}

            text_file = individual_output_dir / f"{file_path.stem}.md"
            text_file.write_text(pdf_text, encoding='utf-8')

            if "stt" in steps:
                download_url = f"/download/{upload_folder_name}/{text_file.name}"
                results["stt"] = download_url
                if record_id:
                    file_path_str = to_record_path(text_file)
                    update_task_completion(record_id, "stt", file_path_str)

            current_file = text_file
            

        # For audio files, run STT step
        elif file_type == 'audio' and "stt" in steps:
            # Check if task was cancelled before starting STT
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}
                
            print(f"Starting STT for task {task_id}")
            
            # Create progress callback function
            def progress_callback(message):
                if task_id:
                    update_task_progress(task_id, message)
                    
            # Get Whisper model from settings, default to large-v3-turbo
            whisper_model = "large-v3-turbo"
            if model_settings and model_settings.get("whisper"):
                whisper_model = model_settings["whisper"]

            # Get Whisper language from settings, default to Korean
            language = "ko"
            if model_settings and model_settings.get("language") is not None:
                lang = model_settings.get("language")
                if lang in ("", "auto"):
                    language = None
                else:
                    language = lang

            device_choice = "auto"
            if model_settings and model_settings.get("device"):
                device_choice = model_settings.get("device")

            try:
                transcribe_audio_files(
                    input_dir=str(current_file.parent),
                    output_dir=str(individual_output_dir),
                    model_identifier=whisper_model,
                    language=language,
                    initial_prompt="",
                    workers=1,
                    recursive=False,
                    filter_fillers=False,
                    min_seg_length=2,
                    normalize_punct=False,
                    requested_device=device_choice,
                    progress_callback=progress_callback
                )
            except Exception as e:
                print(f"STT process failed: {e}")
                if task_id:
                    update_task_progress(task_id, f"STT 실패: {e}")
                return {"error": f"STT process failed: {e}"}

            stt_file = individual_output_dir / f"{file_path.stem}.md"
            download_url = f"/download/{upload_folder_name}/{stt_file.name}"
            results["stt"] = download_url
            current_file = stt_file

            # Update history
            if record_id:
                file_path_str = to_record_path(stt_file)
                update_task_completion(record_id, "stt", file_path_str)

        if "embedding" in steps and current_file:
            # Check if task was cancelled
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}
            
            # For audio files, check if we have the text file (STT completed)
            if file_type == 'audio' and current_file == file_path:
                # current_file is still the original audio file, check for existing STT result first
                existing_stt = find_existing_stt_file(file_path)
                
                if existing_stt:
                    # Use existing STT result
                    if task_id:
                        update_task_progress(task_id, f"기존 STT 결과 발견: {existing_stt.name}")
                    current_file = existing_stt
                    
                    # Update results to include existing STT
                    download_url = f"/download/{upload_folder_name}/{existing_stt.name}"
                    results["stt"] = download_url
                    
                    # Update history if needed
                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)
                else:
                    # No existing STT result, run STT first
                    if task_id:
                        update_task_progress(task_id, "STT 자동 실행 시작")
                    try:
                        def progress_callback(message):
                            if task_id:
                                update_task_progress(task_id, message)

                        # Get Whisper model from settings, default to large-v3-turbo
                        whisper_model = "large-v3-turbo"
                        if model_settings and model_settings.get("whisper"):
                            whisper_model = model_settings["whisper"]

                        # Get Whisper language from settings, default to Korean
                        language = "ko"
                        if model_settings and model_settings.get("language") is not None:
                            lang = model_settings.get("language")
                            if lang in ("", "auto"):
                                language = None
                            else:
                                language = lang

                        device_choice = "auto"
                        if model_settings and model_settings.get("device"):
                            device_choice = model_settings.get("device")

                        transcribe_audio_files(
                            input_dir=str(current_file.parent),
                            output_dir=str(individual_output_dir),
                            model_identifier=whisper_model,
                            language=language,
                            initial_prompt="",
                            workers=1,
                            recursive=False,
                            filter_fillers=False,
                            min_seg_length=2,
                            normalize_punct=False,
                            requested_device=device_choice,
                            progress_callback=progress_callback
                        )
                    except Exception as e:
                        print(f"STT process failed: {e}")
                        if task_id:
                            update_task_progress(task_id, f"STT 실패: {e}")
                        return {"error": f"STT process failed: {e}"}

                    stt_file = individual_output_dir / f"{file_path.stem}.md"
                    download_url = f"/download/{upload_folder_name}/{stt_file.name}"
                    results["stt"] = download_url
                    current_file = stt_file

                    # Update history
                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)

            if task_id:
                update_task_progress(task_id, "임베딩 생성 시작")

            if generate_embedding(current_file, record_id):
                if task_id:
                    update_task_progress(task_id, "임베딩 생성 완료")
            else:
                if task_id:
                    update_task_progress(task_id, "임베딩 생성 실패")

        if "summary" in steps:
            # Check if task was cancelled before starting summary
            if task_id and is_task_cancelled(task_id):
                return {"error": "Task was cancelled"}

            # For audio files, check if we have the text file (STT completed)
            if file_type == 'audio' and current_file == file_path:
                # current_file is still the original audio file, check for existing STT result first
                existing_stt = find_existing_stt_file(file_path)
                
                if existing_stt:
                    # Use existing STT result
                    if task_id:
                        update_task_progress(task_id, f"기존 STT 결과 발견: {existing_stt.name}")
                    current_file = existing_stt
                    
                    # Update results to include existing STT
                    download_url = f"/download/{upload_folder_name}/{existing_stt.name}"
                    results["stt"] = download_url
                    
                    # Update history if needed
                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)
                else:
                    # No existing STT result, run STT first
                    if task_id:
                        update_task_progress(task_id, "STT 자동 실행 시작")
                    try:
                        def progress_callback(message):
                            if task_id:
                                update_task_progress(task_id, message)

                        # Get Whisper model from settings, default to large-v3-turbo
                        whisper_model = "large-v3-turbo"
                        if model_settings and model_settings.get("whisper"):
                            whisper_model = model_settings["whisper"]

                        # Get Whisper language from settings, default to Korean
                        language = "ko"
                        if model_settings and model_settings.get("language") is not None:
                            lang = model_settings.get("language")
                            if lang in ("", "auto"):
                                language = None
                            else:
                                language = lang

                        device_choice = "auto"
                        if model_settings and model_settings.get("device"):
                            device_choice = model_settings.get("device")

                        transcribe_audio_files(
                            input_dir=str(current_file.parent),
                            output_dir=str(individual_output_dir),
                            model_identifier=whisper_model,
                            language=language,
                            initial_prompt="",
                            workers=1,
                            recursive=False,
                            filter_fillers=False,
                            min_seg_length=2,
                            normalize_punct=False,
                            requested_device=device_choice,
                            progress_callback=progress_callback
                        )
                    except Exception as e:
                        print(f"STT process failed: {e}")
                        if task_id:
                            update_task_progress(task_id, f"STT 실패: {e}")
                        return {"error": f"STT process failed: {e}"}

                    stt_file = individual_output_dir / f"{file_path.stem}.md"
                    download_url = f"/download/{upload_folder_name}/{stt_file.name}"
                    results["stt"] = download_url
                    current_file = stt_file

                    # Update history
                    if record_id:
                        file_path_str = to_record_path(current_file)
                        update_task_completion(record_id, "stt", file_path_str)
                
            source_text_path = Path(current_file) if current_file else None

            print(f"Starting summary for task {task_id}")
            if task_id:
                update_task_progress(task_id, "요약 생성 시작")
                
            # Get summarize model from settings, default to DEFAULT_MODEL
            summarize_model = DEFAULT_MODEL
            if model_settings and model_settings.get("summarize"):
                summarize_model = model_settings["summarize"]
                
            try:
                text = read_text_with_fallback(Path(current_file))
                if task_id:
                    update_task_progress(task_id, "텍스트 분석 중...")
                    
                # Create progress callback function for summary
                def summary_progress_callback(message):
                    if task_id:
                        update_task_progress(task_id, message)
                
                summary = summarize_text_mapreduce(
                    text=text,
                    model=summarize_model,
                    chunk_size=DEFAULT_CHUNK_SIZE,
                    max_tokens=None,
                    temperature=DEFAULT_TEMPERATURE,
                    progress_callback=summary_progress_callback
                )
                
                if task_id:
                    update_task_progress(task_id, "요약 파일 저장 중...")
                    
                output_file = Path(current_file).with_name(f"{Path(current_file).stem}.summary.md")
                save_output(summary, output_file, as_json=False)
                
                if task_id:
                    update_task_progress(task_id, "요약 생성 완료")
            except Exception as e:
                print(f"Summary process failed: {e}")
                if task_id:
                    update_task_progress(task_id, f"요약 생성 실패: {e}")
                return {"error": f"Summary process failed: {e}"}

            summary_file = current_file.with_name(f"{current_file.stem}.summary.md")
            download_url = f"/download/{upload_folder_name}/{summary_file.name}"
            results["summary"] = download_url
            current_file = summary_file

            # Update history
            if record_id:
                file_path_str = to_record_path(summary_file)
                update_task_completion(record_id, "summary", file_path_str)
                if source_text_path:
                    generate_and_store_title_summary(record_id, source_text_path, summarize_model)

    except Exception as exc:  # pragma: no cover - best effort error handling
        # Clean up process registration if something goes wrong
        if task_id:
            unregister_process(task_id)
            update_task_progress(task_id, f"작업 실패: {exc}")
        return {"error": str(exc)}
    
    finally:
        # Clear progress when task completes
        if task_id:
            clear_task_progress(task_id)

    return results


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
