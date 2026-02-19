from __future__ import annotations

import json

from ...search_cache import cleanup_expired_cache, get_cache_stats
from ..embedding import run_incremental_embedding
from ..records import reset_tasks_for_all_records, reset_upload_record
from ..destructive_guard import check_destructive_api_access, reject_destructive_api_request
from ..state import cancel_task, get_running_tasks
from ..monitoring import get_workflow_metrics_snapshot


def _read_json_payload(handler):
    length = int(handler.headers.get('Content-Length', 0))
    return json.loads(handler.rfile.read(length)) if length else {}


def _serve_running_tasks(handler) -> None:
    try:
        tasks = get_running_tasks()
        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(tasks, ensure_ascii=False).encode())
    except Exception as e:
        handler.send_response(500)
        handler.end_headers()
        handler.wfile.write(f'Error getting running tasks: {str(e)}'.encode())


def handle_get(handler) -> bool:
    if handler.path == '/tasks':
        _serve_running_tasks(handler)
        return True

    if handler.path == '/cache/stats':
        try:
            stats = get_cache_stats()
            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps(stats, ensure_ascii=False).encode())
        except Exception as e:
            handler.send_response(500)
            handler.end_headers()
            handler.wfile.write(f'Error getting cache stats: {str(e)}'.encode())
        return True

    if handler.path == '/cache/cleanup':
        try:
            cleaned_count = cleanup_expired_cache()
            response = {
                'success': True,
                'cleaned_entries': cleaned_count,
                'message': f'정리된 만료된 캐시 항목: {cleaned_count}개',
            }
            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps(response, ensure_ascii=False).encode())
        except Exception as e:
            handler.send_response(500)
            handler.end_headers()
            handler.wfile.write(
                json.dumps({'success': False, 'error': f'캐시 정리 중 오류: {str(e)}'}, ensure_ascii=False).encode()
            )
        return True

    if handler.path == '/metrics/workflow':
        try:
            metrics = get_workflow_metrics_snapshot()
            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps(metrics, ensure_ascii=False).encode())
        except Exception as e:
            handler.send_response(500)
            handler.end_headers()
            handler.wfile.write(json.dumps({'error': f'메트릭 조회 중 오류: {str(e)}'}, ensure_ascii=False).encode())
        return True

    return False


def handle_post(handler) -> bool:
    if handler.path == '/cancel':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Invalid JSON payload')
            return True
        task_id = payload.get('task_id')
        if not task_id:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Missing task_id')
            return True

        success = cancel_task(task_id)
        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps({'success': success}).encode())
        return True

    if handler.path == '/reset':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Invalid JSON payload')
            return True
        allowed, message = check_destructive_api_access(handler, payload)
        if not allowed:
            reject_destructive_api_request(handler, message or 'Forbidden')
            return True

        record_id = payload.get('record_id')
        if not record_id:
            handler.send_response(400)
            handler.end_headers()
            handler.wfile.write(b'Missing record_id')
            return True

        success = reset_upload_record(record_id)
        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps({'success': success}).encode())
        return True

    if handler.path == '/incremental_embedding':
        try:
            processed_count = run_incremental_embedding()
            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(
                json.dumps(
                    {
                        'success': True,
                        'processed_count': processed_count,
                        'message': f'증분 임베딩 완료: {processed_count}개 파일 처리됨',
                    }
                ).encode()
            )
        except Exception as e:
            handler.send_response(500)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': False, 'error': str(e)}).encode())
        return True

    if handler.path == '/reset_all_tasks':
        try:
            payload = _read_json_payload(handler)
        except json.JSONDecodeError:
            handler.send_response(400)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': False, 'error': 'Invalid JSON payload'}).encode())
            return True

        allowed, message = check_destructive_api_access(handler, payload)
        if not allowed:
            reject_destructive_api_request(handler, message or 'Forbidden')
            return True

        tasks = payload.get('tasks')
        if not isinstance(tasks, list):
            handler.send_response(400)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'success': False, 'error': 'tasks 필드는 배열이어야 합니다.'}).encode())
            return True

        success, counts, message = reset_tasks_for_all_records(set(tasks))
        status_code = 200 if success else 400
        handler.send_response(status_code)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps({'success': success, 'message': message, 'counts': counts}).encode())
        return True

    return False
