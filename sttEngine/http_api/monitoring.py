from __future__ import annotations

import json
import threading
import time
from collections.abc import Mapping

_MONITORED_STEPS = ("stt", "diarize", "correct", "summary")
_metrics_lock = threading.Lock()
_step_metrics: dict[str, dict[str, float | int | str | None]] = {
    step: {
        "runs": 0,
        "successes": 0,
        "failures": 0,
        "total_duration_seconds": 0.0,
        "last_duration_seconds": None,
        "last_status": None,
        "last_error_code": None,
        "last_updated_at": None,
    }
    for step in _MONITORED_STEPS
}


def record_workflow_step_metric(
    *,
    step: str,
    status: str,
    duration_seconds: float,
    task_id: str | None = None,
    record_id: str | None = None,
    error_code: str | None = None,
) -> None:
    """Record timing/failure metrics for workflow steps and emit structured logs."""
    if step not in _step_metrics:
        return

    now = time.time()
    safe_duration = max(0.0, float(duration_seconds))
    with _metrics_lock:
        metric = _step_metrics[step]
        metric["runs"] = int(metric["runs"]) + 1
        metric["total_duration_seconds"] = float(metric["total_duration_seconds"]) + safe_duration
        metric["last_duration_seconds"] = safe_duration
        metric["last_status"] = status
        metric["last_error_code"] = error_code
        metric["last_updated_at"] = now
        if status == "failed":
            metric["failures"] = int(metric["failures"]) + 1
        else:
            metric["successes"] = int(metric["successes"]) + 1

    payload = {
        "event": "workflow_step_metric",
        "step": step,
        "status": status,
        "duration_seconds": round(safe_duration, 3),
        "task_id": task_id,
        "record_id": record_id,
        "error_code": error_code,
        "timestamp": now,
    }
    print(f"[METRIC] {json.dumps(payload, ensure_ascii=False)}")


def get_workflow_metrics_snapshot() -> Mapping[str, object]:
    with _metrics_lock:
        per_step: dict[str, dict[str, object]] = {}
        for step, metric in _step_metrics.items():
            runs = int(metric["runs"])
            failures = int(metric["failures"])
            successes = int(metric["successes"])
            total_duration = float(metric["total_duration_seconds"])
            per_step[step] = {
                "runs": runs,
                "successes": successes,
                "failures": failures,
                "failure_rate": (failures / runs) if runs else 0.0,
                "avg_duration_seconds": (total_duration / runs) if runs else 0.0,
                "total_duration_seconds": total_duration,
                "last_duration_seconds": metric["last_duration_seconds"],
                "last_status": metric["last_status"],
                "last_error_code": metric["last_error_code"],
                "last_updated_at": metric["last_updated_at"],
            }

    return {
        "steps": per_step,
        "generated_at": time.time(),
    }
