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

from .workflow.transcribe import transcribe_audio_files
from .workflow.summarize import (
    summarize_text_mapreduce,
    read_text_with_fallback,
    save_output,
    DEFAULT_MODEL,
    DEFAULT_CHUNK_SIZE,
    DEFAULT_TEMPERATURE,
)
from .config import get_default_model
from .one_line_summary import generate_one_line_summary
from .vector_search import search as search_vectors
from .search_cache import cleanup_expired_cache, get_cache_stats, delete_cache_record
from .embedding_pipeline import embed_text_ollama, load_index, save_index
from ollama_utils import ensure_ollama_server, check_ollama_model_available
import numpy as np
import os


BASE_DIR = Path(getattr(sys, "_MEIPASS", Path(__file__).parent.parent)).resolve()
UPLOAD_DIR = BASE_DIR / "DB" / "uploads"
OUTPUT_DIR = BASE_DIR / "DB" / "whisper_output"
VECTOR_DIR = BASE_DIR / "DB" / "vector_store"
HISTORY_FILE = BASE_DIR / "DB" / "upload_history.json"
FILE_REGISTRY_FILE = BASE_DIR / "DB" / "file_registry.json"

# Global dictionary to track running processes
running_processes = {}
process_lock = threading.Lock()

# Global dictionary to track task progress
task_progress = {}
progress_lock = threading.Lock()

# WebSocket server setup for real-time progress updates
connected_clients = set()
websocket_loop = asyncio.new_event_loop()


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
    audio_extensions = {'.flac', '.m4a', '.mp3', '.mp4', '.mpeg', '.mpga', '.oga', '.ogg', '.wav', '.webm'}
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


def load_upload_history():
    """Load upload history from JSON file."""
    if HISTORY_FILE.exists():
        try:
            with open(HISTORY_FILE, 'r', encoding='utf-8') as f:
                return json.load(f)
        except (json.JSONDecodeError, IOError):
            return []
    return []


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
        "file_path": str(file_path.relative_to(BASE_DIR)),
        "folder_name": file_path.parent.name,  # UUID folder name
        "completed_tasks": {
            "stt": False,
            "embedding": False
        },
        "download_links": {},
        "title_summary": "",
        "tags": [],
        "file_hash": file_hash
    }

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
                return json.load(f)
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

def register_file(file_path: str, record_id: str, task_type: str, original_filename: str = None):
    """Register a file with UUID and return the file UUID."""
    registry = load_file_registry()
    file_uuid = str(uuid.uuid4())
    
    file_info = {
        "file_uuid": file_uuid,
        "file_path": file_path,
        "record_id": record_id,
        "task_type": task_type,
        "original_filename": original_filename or os.path.basename(file_path),
        "created_at": datetime.now().isoformat()
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
        record_id = record["id"]
        download_links = record.get("download_links", {})
        
        # Process each download link
        for task_type, download_url in download_links.items():
            if download_url.startswith("/download/"):
                file_path = download_url[10:]  # Remove "/download/" prefix
                
                # Check if this file is already registered
                already_registered = False
                for file_info in registry.values():
                    if file_info["file_path"] == file_path and file_info["record_id"] == record_id:
                        already_registered = True
                        break
                
                if not already_registered:
                    # Register the file and update download link
                    full_path = BASE_DIR / file_path
                    if full_path.exists():
                        file_uuid = register_file(str(full_path.relative_to(BASE_DIR)), record_id, task_type, os.path.basename(file_path))
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
            record["title_summary"] = summary
            break
    save_upload_history(history)

def update_filename(record_id: str, new_filename: str):
    """Update filename for a record."""
    history = load_upload_history()
    for record in history:
        if record["id"] == record_id:
            record["filename"] = new_filename
            break
    save_upload_history(history)

def generate_and_store_title_summary(record_id: str, file_path: Path):
    """Generate one-line summary and store it."""
    try:
        summary = generate_one_line_summary(file_path)
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
            model_name = os.environ.get("EMBEDDING_MODEL", "nomic-embed-text")
        
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
                    "vector": vector_file.name
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
            model_name = os.environ.get("EMBEDDING_MODEL", "nomic-embed-text")
        
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
            "vector": vector_file.name
        }
        save_index(index)
        
        # Update task completion
        if record_id:
            file_path_str = str(file_path.relative_to(BASE_DIR))
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
                "stt": False,
                "embedding": False,
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
                    file_path_str = str(text_file.relative_to(BASE_DIR))
                    update_task_completion(record_id, "stt", file_path_str)
                    generate_and_store_title_summary(record_id, current_file)
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
                    file_path_str = str(text_file.relative_to(BASE_DIR))
                    update_task_completion(record_id, "stt", file_path_str)
                    generate_and_store_title_summary(record_id, text_file)

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
                file_path_str = str(stt_file.relative_to(BASE_DIR))
                update_task_completion(record_id, "stt", file_path_str)
                generate_and_store_title_summary(record_id, current_file)

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
                        file_path_str = str(current_file.relative_to(BASE_DIR))
                        update_task_completion(record_id, "stt", file_path_str)
                        generate_and_store_title_summary(record_id, current_file)
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
                        file_path_str = str(current_file.relative_to(BASE_DIR))
                        update_task_completion(record_id, "stt", file_path_str)
                        generate_and_store_title_summary(record_id, current_file)

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
                        file_path_str = str(current_file.relative_to(BASE_DIR))
                        update_task_completion(record_id, "stt", file_path_str)
                        generate_and_store_title_summary(record_id, current_file)
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
                        file_path_str = str(current_file.relative_to(BASE_DIR))
                        update_task_completion(record_id, "stt", file_path_str)
                        generate_and_store_title_summary(record_id, current_file)
                
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
                file_path_str = str(summary_file.relative_to(BASE_DIR))
                update_task_completion(record_id, "summary", file_path_str)
                generate_and_store_title_summary(record_id, current_file)

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
                file_path = file_info["file_path"]
                filename = file_info["original_filename"]
                
                # Handle legacy paths without DB/ prefix and normalize path separators
                file_path = file_path.replace('\\', '/')  # Normalize to forward slashes
                if not file_path.startswith("DB/"):
                    file_path = f"DB/{file_path}"
                
                full_path = BASE_DIR / file_path
            else:
                self.send_response(404)
                self.end_headers()
                return
        else:
            # Legacy path-based system
            file_path = file_identifier
            filename = os.path.basename(file_path)
            full_path = BASE_DIR / file_path
        
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
                history = load_upload_history()
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
            query = params.get("q", [""])[0]
            start_date = params.get("start", [None])[0]
            end_date = params.get("end", [None])[0]

            try:
                results = []
                if query:
                    hits = search_vectors(query, BASE_DIR,
                                          start_date=start_date,
                                          end_date=end_date)
                    results = [{"file": r["file"], "score": r["score"], "link": f"/download/{r['file']}"} for r in hits]
                
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps(results, ensure_ascii=False).encode())
            
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
            history = load_upload_history()
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
                    file_path = file_info["file_path"]
                    
                    # Handle legacy paths without DB/ prefix
                    if not file_path.startswith("DB/"):
                        file_path = f"DB/{file_path}"
                    
                    full_path = BASE_DIR / file_path
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
                file_path = file_identifier
                full_path = BASE_DIR / file_path
                current_file_name = os.path.basename(file_path)
            
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

            current_path_norm = os.path.normpath(file_path)
            for hit in hits:
                hit_path_norm = os.path.normpath(hit["file"])
                print(f"[DEBUG] 검토 중인 파일: {hit_path_norm} vs 현재 파일: {current_path_norm}")
                # Skip if it's the same file (compare normalized paths)
                if hit_path_norm != current_path_norm:
                    # Try to find UUID for this file in registry
                    file_uuid = None
                    for uuid_key, file_info in registry.items():
                        if os.path.normpath(file_info["file_path"]) == hit_path_norm:
                            file_uuid = uuid_key
                            break

                    # Use UUID if available, otherwise fallback to path
                    download_link = f"/download/{file_uuid}" if file_uuid else f"/download/{hit['file']}"

                    similar_docs.append({
                        "file": hit["file"],
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
                    file_path = file_info["file_path"]
                    
                    # Handle legacy paths without DB/ prefix and normalize path separators
                    file_path = file_path.replace('\\', '/')  # Normalize to forward slashes
                    if not file_path.startswith("DB/"):
                        file_path = f"DB/{file_path}"
                    
                    full_path = BASE_DIR / file_path
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
                file_path = file_identifier
                full_path = BASE_DIR / file_path
                current_file_name = user_filename or os.path.basename(file_path)
            
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

            current_path_norm = os.path.normpath(file_path)
            for hit in hits:
                hit_path_norm = os.path.normpath(hit["file"])
                # Skip if it's the same file (compare normalized paths)
                if hit_path_norm != current_path_norm:
                    # Try to find UUID for this file in registry
                    file_uuid = None
                    record_id = None
                    for uuid_key, file_info in registry.items():
                        if os.path.normpath(file_info["file_path"]) == hit_path_norm:
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
                    download_link = f"/download/{file_uuid}" if file_uuid else f"/download/{hit['file']}"

                    # Use user filename if available, otherwise original filename
                    display_filename = user_filename_found or os.path.basename(hit["file"])

                    similar_docs.append({
                        "file": hit["file"],
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
        """Serve available Ollama models as JSON."""
        try:
            # Ollama 서버 상태 확인 및 필요시 시작
            server_ok, server_msg = ensure_ollama_server()
            if not server_ok:
                raise Exception(f"Ollama 서버를 사용할 수 없습니다: {server_msg}")
            
            import subprocess
            result = subprocess.run(['ollama', 'list'], capture_output=True, text=True, timeout=10)
            
            if result.returncode != 0:
                raise Exception(f"Ollama list failed: {result.stderr}")
            
            # Parse ollama list output
            lines = result.stdout.strip().split('\n')
            models = []
            
            # Skip header line if present
            for line in lines[1:] if len(lines) > 1 else lines:
                if line.strip():
                    # Extract model name (first column)
                    parts = line.split()
                    if parts:
                        model_name = parts[0]
                        # Filter out model names with slashes or special characters typically not used for LLM models
                        if '/' not in model_name and model_name not in ['mxbai-embed-large']:
                            models.append(model_name)
            
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
            print(f"모델 목록 조회 중 오류: {e}")
            self.send_response(500)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            error_response = {
                "error": "모델 목록을 조회할 수 없습니다. Ollama가 실행 중인지 확인해주세요.",
                "details": str(e)
            }
            self.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())

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
                        "file_path": str(file_path.relative_to(BASE_DIR)),
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
            results = run_workflow(BASE_DIR / file_path, steps, record_id, task_id, model_settings)
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
                original_file = BASE_DIR / file_path
                existing_stt = find_existing_stt_file(original_file)
                
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "has_stt": existing_stt is not None,
                    "stt_file": str(existing_stt.relative_to(BASE_DIR)) if existing_stt else None
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
    UPLOAD_DIR.mkdir(exist_ok=True)
    OUTPUT_DIR.mkdir(exist_ok=True)
    
    # Migrate existing files to UUID system
    migrate_existing_files()

    # Start WebSocket server for progress updates
    ws_thread = threading.Thread(target=start_websocket_server, daemon=True)
    ws_thread.start()

    # Use ThreadingHTTPServer to allow concurrent request handling.
    # This lets the server respond to cancellation requests while
    # long-running tasks are processing in separate threads.
    server = ThreadingHTTPServer(("127.0.0.1", 8080), UploadHandler)
    print("Serving on http://localhost:8080")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
