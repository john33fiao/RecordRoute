"""Simple HTTP server for uploading files and running workflow steps.

The server exposes:
  * ``GET /`` – serve the upload HTML page.
  * ``POST /upload`` – accept an audio file and store it under ``uploads/``.
  * ``POST /process`` – run selected workflow steps for the uploaded file.
  * ``GET /download/<file>`` – return processed files for download.

Only selected workflow steps return download links.
"""

from http.server import HTTPServer, BaseHTTPRequestHandler
import cgi
import json
import os
import subprocess
import sys
import uuid
from pathlib import Path


BASE_DIR = Path(__file__).parent.resolve()
UPLOAD_DIR = BASE_DIR / "uploads"
OUTPUT_DIR = BASE_DIR / "whisper_output"
WORKFLOW_DIR = BASE_DIR / "sttEngine" / "workflow"
PYTHON = sys.executable


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

    try:
        if "stt" in steps:
            subprocess.run(
                [PYTHON, str(WORKFLOW_DIR / "transcribe.py"), str(current_file.parent), "--output_dir", str(OUTPUT_DIR)],
                check=True,
            )
            stt_file = OUTPUT_DIR / f"{file_path.stem}.md"
            results["stt"] = f"/download/{stt_file.name}"
            current_file = stt_file

        if "correct" in steps:
            subprocess.run([PYTHON, str(WORKFLOW_DIR / "correct.py"), str(current_file)], check=True)
            corrected_file = current_file.with_name(f"{current_file.stem}.corrected.md")
            results["correct"] = f"/download/{corrected_file.name}"
            current_file = corrected_file

        if "summary" in steps:
            subprocess.run([PYTHON, str(WORKFLOW_DIR / "summarize.py"), str(current_file)], check=True)
            summary_file = current_file.with_name(f"{current_file.stem}.summary.md")
            results["summary"] = f"/download/{summary_file.name}"

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

    def _serve_download(self, filename: str):
        file_path = OUTPUT_DIR / filename
        if file_path.exists():
            self.send_response(200)
            self.send_header("Content-Type", "application/octet-stream")
            self.send_header("Content-Disposition", f"attachment; filename={filename}")
            self.end_headers()
            with open(file_path, "rb") as f:
                self.wfile.write(f.read())
        else:
            self.send_response(404)
            self.end_headers()

    def do_GET(self):
        if self.path == "/":
            self._serve_upload_page()
        elif self.path.startswith("/download/"):
            filename = os.path.basename(self.path[len("/download/"):])
            self._serve_download(filename)
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if self.path == "/upload":
            form = cgi.FieldStorage(
                fp=self.rfile,
                headers=self.headers,
                environ={
                    "REQUEST_METHOD": "POST",
                    "CONTENT_TYPE": self.headers.get("Content-Type"),
                },
            )

            file_item = form["file"] if "file" in form else None
            if not file_item or not file_item.filename:
                self.send_response(400)
                self.end_headers()
                self.wfile.write(b"No file uploaded")
                return

            uid = uuid.uuid4().hex
            save_dir = UPLOAD_DIR / uid
            save_dir.mkdir(parents=True, exist_ok=True)
            file_path = save_dir / os.path.basename(file_item.filename)
            with open(file_path, "wb") as output_file:
                output_file.write(file_item.file.read())

            self.send_response(200)
            self.send_header("Content-Type", "application/json")
            self.end_headers()
            self.wfile.write(json.dumps({"file_path": str(file_path.relative_to(BASE_DIR))}).encode())
            return

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
