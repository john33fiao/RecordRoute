from __future__ import annotations

from ...http_api.workflow import run_workflow
from ...http_api.state import update_task_progress, TaskStage
from ..services.errors import map_workflow_exception


def run_process_task(task_request: dict) -> dict:
    try:
        update_task_progress(task_request["task_id"], "작업 접수 완료", stage=TaskStage.UPLOAD)
        result = run_workflow(
            task_request["absolute_path"],
            task_request["steps"],
            task_request["record_id"],
            task_request["task_id"],
            task_request["model_settings"],
        )
        result["task_id"] = task_request["task_id"]
        result["retry_mode"] = task_request.get("retry_mode", "new_task")
        if task_request.get("retry_of_task_id"):
            result["retry_of_task_id"] = task_request.get("retry_of_task_id")
        return result
    except Exception as exc:
        mapped = map_workflow_exception(exc, "workflow")
        return {
            "error": mapped.message,
            "error_code": mapped.code,
            "retryable": mapped.retryable,
            "failed_step": mapped.failed_step or "workflow",
        }
