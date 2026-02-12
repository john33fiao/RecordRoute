"""Simple HTTP server for uploading files and running workflow steps.

The server exposes:
  * ``GET /`` – serve the upload HTML page.
  * ``POST /upload`` – accept an audio file and store it under ``DB/uploads/``.
  * ``POST /process`` – run selected workflow steps for the uploaded file.
  * ``GET /download/<file>`` – return processed files for download.

Only selected workflow steps return download links.
"""

try:
    from .logger import setup_logging
except ImportError:  # pragma: no cover - fallback for script execution
    from logger import setup_logging


setup_logging()


def main() -> None:
    from .http_api.app import main as run_app

    run_app()


if __name__ == "__main__":
    main()
