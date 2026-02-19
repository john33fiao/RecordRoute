from __future__ import annotations

import json

from sttEngine.http_api import monitoring
from sttEngine.http_api.routes import management_routes


def _body(handler) -> dict:
    handler.wfile.seek(0)
    raw = handler.wfile.read().decode('utf-8')
    return json.loads(raw) if raw else {}


def test_record_workflow_step_metric_accumulates_duration_and_failure_rate(monkeypatch):
    monkeypatch.setattr(
        monitoring,
        "_step_metrics",
        {
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
            for step in ("stt", "diarize", "correct", "summary")
        },
    )

    monitoring.record_workflow_step_metric(step="diarize", status="completed", duration_seconds=3.0)
    monitoring.record_workflow_step_metric(
        step="diarize",
        status="failed",
        duration_seconds=1.0,
        error_code="diarization_timeout",
    )

    snapshot = monitoring.get_workflow_metrics_snapshot()
    diarize = snapshot["steps"]["diarize"]
    assert diarize["runs"] == 2
    assert diarize["successes"] == 1
    assert diarize["failures"] == 1
    assert diarize["failure_rate"] == 0.5
    assert diarize["avg_duration_seconds"] == 2.0
    assert diarize["last_status"] == "failed"
    assert diarize["last_error_code"] == "diarization_timeout"


def test_workflow_metrics_route_returns_snapshot(dummy_handler_factory, monkeypatch):
    handler = dummy_handler_factory({})
    handler.path = '/metrics/workflow'

    monkeypatch.setattr(
        management_routes,
        'get_workflow_metrics_snapshot',
        lambda: {"steps": {"stt": {"runs": 3}}, "generated_at": 123.4},
    )

    handled = management_routes.handle_get(handler)

    assert handled is True
    assert handler.status_code == 200
    assert _body(handler)["steps"]["stt"]["runs"] == 3
