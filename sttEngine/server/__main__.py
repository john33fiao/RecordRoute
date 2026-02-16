"""Module entrypoint for ``python -m sttEngine.server``."""

from sttEngine.logger import setup_logging
from sttEngine.http_api.app import main as run_app


setup_logging()


def main() -> None:
    run_app()


if __name__ == "__main__":
    main()
