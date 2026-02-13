"""Simple HTTP server for uploading files and running workflow steps.

The server exposes:
  * ``GET /`` – serve the upload HTML page.
  * ``POST /upload`` – accept an audio file and store it under ``DB/uploads/``.
  * ``POST /process`` – run selected workflow steps for the uploaded file.
  * ``GET /download/<file>`` – return processed files for download.

Only selected workflow steps return download links.
"""

from sttEngine.logger import setup_logging
from sttEngine.http_api.app import main as run_app

setup_logging()


def main() -> None:
    run_app()


if __name__ == "__main__":
    main()
