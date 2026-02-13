from __future__ import annotations

from ...http_api.workflow import run_workflow
from ..services.errors import map_workflow_exception


def run_process_task(task_request: dict) -> dict:
    try:
        result = run_workflow(
            task_request["absolute_path"],
            task_request["steps"],
            task_request["record_id"],
            task_request["task_id"],
            task_request["model_settings"],
        )
        return result
    except Exception as exc:
        mapped = map_workflow_exception(exc, "workflow")
        return {
            "error": mapped.message,
            "error_code": mapped.code,
            "retryable": mapped.retryable,
            "failed_step": mapped.failed_step or "workflow",
        }
