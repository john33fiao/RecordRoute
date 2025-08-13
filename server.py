"""Simple HTTP server for uploading files and running workflow steps.

The server exposes:
  * ``GET /`` – serve the upload HTML page.
  * ``POST /upload`` – accept an audio file and store it under ``uploads/``.
  * ``POST /process`` – run selected workflow steps for the uploaded file.
  * ``GET /download/<file>`` – return processed files for download.

Only selected workflow steps return download links.
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import json
import os
import subprocess
import sys
import uuid
from pathlib import Path
import re
from urllib.parse import unquote
from datetime import datetime


BASE_DIR = Path(__file__).parent.resolve()
UPLOAD_DIR = BASE_DIR / "uploads"
OUTPUT_DIR = BASE_DIR / "whisper_output"
WORKFLOW_DIR = BASE_DIR / "sttEngine" / "workflow"
HISTORY_FILE = BASE_DIR / "upload_history.json"
PYTHON = sys.executable


def get_file_type(file_path: Path):
    """Determine if the file is audio or text.
    
    Returns:
        'audio' for audio files, 'text' for text files, 'unknown' for others.
    """
    audio_extensions = {'.flac', '.m4a', '.mp3', '.mp4', '.mpeg', '.mpga', '.oga', '.ogg', '.wav', '.webm'}
    text_extensions = {'.md', '.txt', '.text', '.markdown'}
    
    suffix = file_path.suffix.lower()
    if suffix in audio_extensions:
        return 'audio'
    elif suffix in text_extensions:
        return 'text'
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


def add_upload_record(file_path: Path, file_type: str, duration: str = None):
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
            "correct": False,
            "summary": False
        },
        "download_links": {}
    }
    
    history.insert(0, record)  # Add to beginning (most recent first)
    
    # Keep only last 100 records
    if len(history) > 100:
        history = history[:100]
    
    save_upload_history(history)
    return record["id"]


def update_task_completion(record_id: str, task: str, download_url: str):
    """Update task completion status and download link."""
    history = load_upload_history()
    
    for record in history:
        if record["id"] == record_id:
            record["completed_tasks"][task] = True
            record["download_links"][task] = download_url
            break
    
    save_upload_history(history)


def run_workflow(file_path: Path, steps, record_id: str = None):
    """Run the requested workflow steps sequentially.

    Args:
        file_path: Path to the uploaded audio or text file.
        steps: list of step names, e.g. ["stt", "correct", "summary"].
        record_id: Upload record ID for updating history.

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
                    update_task_completion(record_id, "stt", download_url)
            else:
                # If no STT step for text file, use the original file as starting point
                # Copy to output directory for consistency
                text_file = individual_output_dir / f"{file_path.stem}.md"
                import shutil
                shutil.copy2(file_path, text_file)
                current_file = text_file
        
        # For audio files, run STT step
        elif file_type == 'audio' and "stt" in steps:
            subprocess.run(
                [PYTHON, str(WORKFLOW_DIR / "transcribe.py"), str(current_file.parent), "--output_dir", str(individual_output_dir), "--model_size", "large"],
                check=True,
            )
            stt_file = individual_output_dir / f"{file_path.stem}.md"
            download_url = f"/download/{upload_folder_name}/{stt_file.name}"
            results["stt"] = download_url
            current_file = stt_file
            
            # Update history
            if record_id:
                update_task_completion(record_id, "stt", download_url)

        if "correct" in steps:
            subprocess.run([PYTHON, str(WORKFLOW_DIR / "correct.py"), str(current_file)], check=True)
            corrected_file = current_file.with_name(f"{current_file.stem}.corrected.md")
            download_url = f"/download/{upload_folder_name}/{corrected_file.name}"
            results["correct"] = download_url
            current_file = corrected_file
            
            # Update history
            if record_id:
                update_task_completion(record_id, "correct", download_url)

        if "summary" in steps:
            subprocess.run([PYTHON, str(WORKFLOW_DIR / "summarize.py"), str(current_file)], check=True)
            summary_file = current_file.with_name(f"{current_file.stem}.summary.md")
            download_url = f"/download/{upload_folder_name}/{summary_file.name}"
            results["summary"] = download_url
            
            # Update history
            if record_id:
                update_task_completion(record_id, "summary", download_url)

    except Exception as exc:  # pragma: no cover - best effort error handling
        return {"error": str(exc)}

    return results


class UploadHandler(BaseHTTPRequestHandler):
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

    def _serve_download(self, file_path: str):
        # Handle both old flat structure and new nested structure
        full_path = OUTPUT_DIR / file_path
        if full_path.exists():
            filename = os.path.basename(file_path)
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

    def do_GET(self):
        if self.path == "/":
            self._serve_upload_page()
        elif self.path.startswith("/download/"):
            file_path = unquote(self.path[len("/download/"):])
            self._serve_download(file_path)
        elif self.path == "/history":
            self._serve_history()
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
                    
                    files[name] = {
                        'filename': filename,
                        'data': body
                    }
        
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
                print(f"Parsed files: {list(files.keys())}")
                
                if 'file' not in files or not files['file']['filename']:
                    print("Upload failed: No file uploaded or filename is empty")
                    print(f"Available files: {files}")
                    self.send_response(400)
                    self.end_headers()
                    self.wfile.write(b"No file uploaded")
                    return

                file_info = files['file']
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
                record_id = add_upload_record(file_path, file_type, duration)
                
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "file_path": str(file_path.relative_to(BASE_DIR)),
                    "file_type": file_type,
                    "record_id": record_id
                }).encode())
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
            payload = json.loads(self.rfile.read(length)) if length else {}
            file_path = payload.get("file_path")
            steps = payload.get("steps", [])
            record_id = payload.get("record_id")
            
            if not file_path:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing file_path")
                return

            results = run_workflow(BASE_DIR / file_path, steps, record_id)
            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps(results).encode())
            return

        self.send_response(404)
        self.end_headers()


if __name__ == "__main__":
    UPLOAD_DIR.mkdir(exist_ok=True)
    OUTPUT_DIR.mkdir(exist_ok=True)
    server = HTTPServer(("127.0.0.1", 8080), UploadHandler)
    print("Serving on http://localhost:8080")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
