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


BASE_DIR = Path(__file__).parent.resolve()
UPLOAD_DIR = BASE_DIR / "uploads"
OUTPUT_DIR = BASE_DIR / "whisper_output"
WORKFLOW_DIR = BASE_DIR / "sttEngine" / "workflow"
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


def run_workflow(file_path: Path, steps):
    """Run the requested workflow steps sequentially.

    Args:
        file_path: Path to the uploaded audio or text file.
        steps: list of step names, e.g. ["stt", "correct", "summary"].

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
                results["stt"] = f"/download/{upload_folder_name}/{text_file.name}"
                current_file = text_file
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
            results["stt"] = f"/download/{upload_folder_name}/{stt_file.name}"
            current_file = stt_file

        if "correct" in steps:
            subprocess.run([PYTHON, str(WORKFLOW_DIR / "correct.py"), str(current_file)], check=True)
            corrected_file = current_file.with_name(f"{current_file.stem}.corrected.md")
            results["correct"] = f"/download/{upload_folder_name}/{corrected_file.name}"
            current_file = corrected_file

        if "summary" in steps:
            subprocess.run([PYTHON, str(WORKFLOW_DIR / "summarize.py"), str(current_file)], check=True)
            summary_file = current_file.with_name(f"{current_file.stem}.summary.md")
            results["summary"] = f"/download/{upload_folder_name}/{summary_file.name}"

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
        else:
            self.send_response(404)
            self.end_headers()

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
                
                self.send_response(200)
                self.send_header("Content-Type", "application/json")
                self.end_headers()
                self.wfile.write(json.dumps({
                    "file_path": str(file_path.relative_to(BASE_DIR)),
                    "file_type": file_type
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
            if not file_path:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"Missing file_path")
                return

            results = run_workflow(BASE_DIR / file_path, steps)
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
