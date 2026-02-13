from __future__ import annotations

from ...http_api.workflow import run_workflow


def run_process_task(task_request: dict) -> dict:
    return run_workflow(
        task_request["absolute_path"],
        task_request["steps"],
        task_request["record_id"],
        task_request["task_id"],
        task_request["model_settings"],
    )
