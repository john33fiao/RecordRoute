from __future__ import annotations

from http.server import BaseHTTPRequestHandler
from ..server.routes import history as history_route
from ..server.routes import process as process_route
from ..server.routes import progress as progress_route
from .routes import admin_routes, file_routes, management_routes, records_routes, search_routes, similarity_routes, upload_routes


class UploadHandler(BaseHTTPRequestHandler):
    def log_message(self, format, *args):
        """Override to filter out successful HTTP requests (200)."""
        message = format % args
        if not any(code in message for code in ['" 200 ', ' 200 '] ):
            super().log_message(format, *args)

    def do_GET(self):
        if file_routes.handle_get(self):
            return
        if self.path == "/history":
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


    def do_POST(self):
        if upload_routes.handle_post(self):
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
