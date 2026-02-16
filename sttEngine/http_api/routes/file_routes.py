from __future__ import annotations

import posixpath
import uuid
from urllib.parse import quote, unquote

from ..paths import BASE_DIR, normalize_record_path, resolve_record_path
from ..registry import get_file_by_uuid

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


def _is_uuid(value: str) -> bool:
    try:
        uuid.UUID(value)
        return True
    except ValueError:
        return False


def _serve_upload_page(handler):
    dist_index = BASE_DIR / "frontend" / "dist" / "index.html"
    legacy_index = BASE_DIR / "frontend" / "legacy" / "upload.html"
    try:
        target = dist_index if dist_index.exists() else legacy_index
        with open(target, "rb") as f:
            content = f.read()
        handler.send_response(200)
        handler.send_header("Content-Type", "text/html; charset=utf-8")
        handler.end_headers()
        handler.wfile.write(content)
    except FileNotFoundError:
        handler.send_response(404)
        handler.end_headers()


def _serve_dist_file(handler, rel_path: str):
    normalized_rel_path = posixpath.normpath(rel_path).lstrip("/")
    dist_root = (BASE_DIR / "frontend" / "dist").resolve()
    file_path = BASE_DIR / "frontend" / "dist" / normalized_rel_path
    try:
        if not file_path.resolve().is_relative_to(dist_root):
            handler.send_response(403)
            handler.end_headers()
            return
        with open(file_path, "rb") as f:
            content = f.read()
        ext = file_path.suffix.lower()
        content_type = CONTENT_TYPES.get(ext, "application/octet-stream")
        handler.send_response(200)
        handler.send_header("Content-Type", content_type)
        if normalized_rel_path.startswith("assets/") or "/assets/" in normalized_rel_path:
            handler.send_header("Cache-Control", "public, max-age=31536000, immutable")
        handler.end_headers()
        handler.wfile.write(content)
    except FileNotFoundError:
        handler.send_response(404)
        handler.end_headers()


def _serve_static(handler, filename: str, content_type: str):
    try:
        with open(BASE_DIR / "frontend" / filename, "rb") as f:
            content = f.read()
        handler.send_response(200)
        handler.send_header("Content-Type", content_type)
        handler.end_headers()
        handler.wfile.write(content)
    except FileNotFoundError:
        handler.send_response(404)
        handler.end_headers()


def _serve_download(handler, file_identifier: str):
    if _is_uuid(file_identifier):
        file_info = get_file_by_uuid(file_identifier)
        if file_info:
            if file_info.get("deleted"):
                handler.send_response(404)
                handler.end_headers()
                return
            file_path = normalize_record_path(file_info["file_path"])
            filename = file_info["original_filename"]
            full_path = resolve_record_path(file_path)
        else:
            handler.send_response(404)
            handler.end_headers()
            return
    else:
        normalized_path = normalize_record_path(file_identifier)
        full_path = resolve_record_path(normalized_path)
        filename = file_identifier.split("/")[-1] or full_path.name

    if full_path.exists():
        handler.send_response(200)
        handler.send_header("Content-Type", "application/octet-stream")
        try:
            filename.encode("ascii")
            handler.send_header("Content-Disposition", f"attachment; filename={filename}")
        except UnicodeEncodeError:
            encoded_filename = quote(filename.encode("utf-8"))
            handler.send_header("Content-Disposition", f"attachment; filename*=UTF-8''{encoded_filename}")
        handler.end_headers()
        with open(full_path, "rb") as f:
            handler.wfile.write(f.read())
    else:
        handler.send_response(404)
        handler.end_headers()


def handle_get(handler) -> bool:
    if handler.path == "/":
        _serve_upload_page(handler)
        return True

    if handler.path.startswith("/assets/"):
        _serve_dist_file(handler, handler.path.lstrip("/"))
        return True

    if handler.path in ("/upload.css", "/upload.js"):
        content_type = "text/css" if handler.path.endswith(".css") else "application/javascript"
        _serve_static(handler, handler.path.lstrip("/"), content_type)
        return True

    if handler.path in ("/graph-view", "/graph-view.html"):
        _serve_static(handler, "graph-view.html", "text/html; charset=utf-8")
        return True

    if handler.path in ("/favicon.ico", "/vite.svg"):
        _serve_dist_file(handler, handler.path.lstrip("/"))
        return True

    if handler.path.startswith("/download/"):
        file_identifier = unquote(handler.path[len("/download/") :])
        _serve_download(handler, file_identifier)
        return True

    return False
