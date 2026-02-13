from __future__ import annotations

import subprocess
import threading
import time

from .ws import broadcast_progress


TASK_TYPES = ("stt", "embedding", "summary")

# Global dictionary to track running processes
running_processes: dict[str, dict] = {}
process_lock = threading.Lock()

# Global dictionary to track task progress
task_progress: dict[str, dict] = {}
progress_lock = threading.Lock()


def register_process(task_id: str, process) -> None:
    """Register a running process for a task."""
    with process_lock:
        running_processes[task_id] = {
            "process": process,
            "cancelled": False,
            "start_time": time.time(),
        }
        print(f"Registered process for task {task_id}, PID: {process.pid}")


def unregister_process(task_id: str) -> None:
    """Unregister a process when it completes."""
    with process_lock:
        if task_id in running_processes:
            del running_processes[task_id]
            print(f"Unregistered process for task {task_id}")


def cancel_task(task_id: str) -> bool:
    """Cancel a running task by terminating its process."""
    with process_lock:
        if task_id in running_processes:
            task_info = running_processes[task_id]
            task_info["cancelled"] = True
            process = task_info["process"]

            try:
                print(f"Terminating process for task {task_id}, PID: {process.pid}")
                process.terminate()

                # Give it a moment to terminate gracefully
                try:
                    process.wait(timeout=5)
                    print(f"Process {process.pid} terminated gracefully")
                except subprocess.TimeoutExpired:
                    print(f"Process {process.pid} didn't terminate gracefully, killing...")
                    process.kill()
                    process.wait()
                    print(f"Process {process.pid} killed")

            except Exception as e:
                print(f"Error terminating process for task {task_id}: {e}")

            return True

        print(f"Task {task_id} not found in running processes")
        return False


def is_task_cancelled(task_id: str) -> bool:
    """Check if a task has been cancelled."""
    with process_lock:
        if task_id in running_processes:
            return bool(running_processes[task_id].get("cancelled"))
        return False


def update_task_progress(
    task_id: str,
    message: str,
    error_code: str | None = None,
    retryable: bool | None = None,
    failed_step: str | None = None,
) -> None:
    """Update progress message for a task."""
    with progress_lock:
        task_progress[task_id] = {
            "task_id": task_id,
            "message": message,
            "timestamp": time.time(),
            "error_code": error_code,
            "retryable": retryable,
            "failed_step": failed_step,
        }
        print(f"Task {task_id}: {message}")
    broadcast_progress(task_id, message, error_code, retryable, failed_step)


def get_task_progress(task_id: str) -> dict:
    """Get current progress for a task."""
    with progress_lock:
        return task_progress.get(task_id, {})


def clear_task_progress(task_id: str) -> None:
    """Clear progress for a completed/cancelled task."""
    with progress_lock:
        if task_id in task_progress:
            del task_progress[task_id]


def get_running_tasks() -> dict[str, dict]:
    """Get information about currently running tasks."""
    with process_lock:
        return {
            task_id: {
                "pid": info["process"].pid,
                "start_time": info["start_time"],
                "cancelled": info["cancelled"],
                "duration": time.time() - info["start_time"],
            }
            for task_id, info in running_processes.items()
        }
