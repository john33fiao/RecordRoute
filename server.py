"""Simple HTTP server to handle audio file uploads.

This server serves the test upload page and accepts file uploads at /upload.
Files are stored in the local 'uploads' directory.
"""
from http.server import HTTPServer, BaseHTTPRequestHandler
import cgi
import os

UPLOAD_DIR = 'uploads'

class UploadHandler(BaseHTTPRequestHandler):
    def do_GET(self):
        if self.path == '/':
            try:
                with open('frontend/upload.html', 'rb') as f:
                    content = f.read()
                self.send_response(200)
                self.send_header('Content-Type', 'text/html; charset=utf-8')
                self.end_headers()
                self.wfile.write(content)
            except FileNotFoundError:
                self.send_response(404)
                self.end_headers()
        else:
            self.send_response(404)
            self.end_headers()

    def do_POST(self):
        if self.path != '/upload':
            self.send_response(404)
            self.end_headers()
            return

        form = cgi.FieldStorage(
            fp=self.rfile,
            headers=self.headers,
            environ={
                'REQUEST_METHOD': 'POST',
                'CONTENT_TYPE': self.headers.get('Content-Type'),
            },
        )

        file_item = form['file'] if 'file' in form else None
        if not file_item or not file_item.filename:
            self.send_response(400)
            self.end_headers()
            self.wfile.write(b'No file uploaded')
            return

        os.makedirs(UPLOAD_DIR, exist_ok=True)
        file_path = os.path.join(UPLOAD_DIR, os.path.basename(file_item.filename))
        with open(file_path, 'wb') as output_file:
            output_file.write(file_item.file.read())

        self.send_response(200)
        self.end_headers()
        self.wfile.write(b'File uploaded successfully')

if __name__ == '__main__':
    os.makedirs(UPLOAD_DIR, exist_ok=True)
    server = HTTPServer(('0.0.0.0', 8000), UploadHandler)
    print('Serving on http://localhost:8000')
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
