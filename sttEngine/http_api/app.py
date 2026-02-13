from __future__ import annotations

import os
import threading
from http.server import ThreadingHTTPServer

from .handler import UploadHandler
from .paths import (
    DELETED_OUTPUT_DIR,
    DELETED_UPLOAD_DIR,
    DELETED_VECTOR_DIR,
    OUTPUT_DIR,
    UPLOAD_DIR,
)
from .registry import migrate_existing_files
from .ws import start_websocket_server


def main(host: str | None = None, port: int | None = None) -> None:
    host = host or os.getenv("HOST", "127.0.0.1")
    port = port or int(os.getenv("PORT", "8080"))
    UPLOAD_DIR.mkdir(parents=True, exist_ok=True)
    OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    DELETED_UPLOAD_DIR.mkdir(parents=True, exist_ok=True)
    DELETED_OUTPUT_DIR.mkdir(parents=True, exist_ok=True)
    DELETED_VECTOR_DIR.mkdir(parents=True, exist_ok=True)

    migrate_existing_files()

    ws_thread = threading.Thread(target=start_websocket_server, daemon=True)
    ws_thread.start()

    server = ThreadingHTTPServer((host, port), UploadHandler)
    print(f"Serving on http://localhost:{port}")
    try:
        server.serve_forever()
    except KeyboardInterrupt:
        pass
    finally:
        server.server_close()
