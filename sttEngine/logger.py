import sys
from datetime import datetime
from pathlib import Path

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
        name = datetime.now().strftime("%Y-%m-%d %H:%M:%S") + ".log"
        return open(self.directory / name, "a", encoding="utf-8")

    def _rollover(self):
        if self._file:
            self._file.close()
        name = datetime.now().strftime("%Y-%m-%d %H:%M:%S") + ".log"
        self._file = open(self.directory / name, "a", encoding="utf-8")

    def write(self, message: str) -> None:
        if not isinstance(message, str):
            message = str(message)
        if self._file.tell() + len(message.encode("utf-8")) > self.max_bytes:
            self._rollover()
        self._file.write(message)
        self._file.flush()

    def flush(self) -> None:
        self._file.flush()

def setup_logging():
    """Redirect stdout and stderr to log files while keeping console output."""
    log_dir = Path(__file__).resolve().parent.parent / "db" / "log"
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
