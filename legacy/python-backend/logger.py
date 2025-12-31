import sys
from datetime import datetime
from pathlib import Path

try:  # pragma: no cover - fallback import for script execution context
    from .config import get_db_base_path
except Exception:  # pragma: no cover - during packaging or direct execution
    try:
        from config import get_db_base_path  # type: ignore
    except Exception:  # pragma: no cover - ensure logger remains usable
        get_db_base_path = None  # type: ignore

MAX_LOG_SIZE = 1 * 1024 * 1024  # 1MB

class _LogFile:
    def __init__(self, directory: Path, max_bytes: int = MAX_LOG_SIZE) -> None:
        self.directory = directory
        self.max_bytes = max_bytes
        self.directory.mkdir(parents=True, exist_ok=True)
        self._file = self._open_latest()
        self.write(f"--- Log started at {datetime.now().isoformat()} ---\n")

    def _open_latest(self):
        logs = sorted(self.directory.glob("*.log"))
        if logs:
            last = logs[-1]
            if last.stat().st_size < self.max_bytes:
                return open(last, "a", encoding="utf-8")
        name = self._timestamped_name()
        return open(self.directory / name, "a", encoding="utf-8")

    def _rollover(self):
        if self._file:
            self._file.close()
        name = self._timestamped_name()
        self._file = open(self.directory / name, "a", encoding="utf-8")

    def _timestamped_name(self) -> str:
        """Return a filesystem-safe log file name for the current timestamp."""
        # Windows disallows ``:`` in file names, so we normalize the timestamp by
        # replacing the separators with ``-``.
        return datetime.now().strftime("%Y-%m-%d_%H-%M-%S.log")

    def write(self, message: str) -> None:
        if not isinstance(message, str):
            message = str(message)
        if self._file.tell() + len(message.encode("utf-8")) > self.max_bytes:
            self._rollover()
        self._file.write(message)
        self._file.flush()

    def flush(self) -> None:
        self._file.flush()

def _get_log_directory() -> Path:
    """Resolve the log directory respecting the configured DB folder."""
    project_root = Path(__file__).resolve().parent.parent

    if callable(globals().get("get_db_base_path")):
        try:
            db_root = get_db_base_path()
            if db_root:
                db_root = Path(db_root)
                return (db_root / "log").resolve()
        except Exception:
            pass

    return (project_root / "db" / "log").resolve()


def setup_logging():
    """Redirect stdout and stderr to log files while keeping console output."""
    log_dir = _get_log_directory()
    logfile = _LogFile(log_dir)

    class Tee:
        def __init__(self, *streams):
            self.streams = streams

        def write(self, data):
            for s in self.streams:
                s.write(data)
                s.flush()

        def flush(self):
            for s in self.streams:
                s.flush()

    sys.stdout = Tee(sys.__stdout__, logfile)
    sys.stderr = Tee(sys.__stderr__, logfile)
    return logfile
