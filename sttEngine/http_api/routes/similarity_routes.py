from __future__ import annotations

import json
from urllib.parse import parse_qs, unquote, urlparse

from ...similarity_matrix import get_documents_metadata, get_similarity_graph
from .similar_documents import SimilarDocumentNotFound, find_similar_documents


def _read_json_payload(handler):
    length = int(handler.headers.get('Content-Length', 0))
    return json.loads(handler.rfile.read(length)) if length else {}




def _first(params, *names: str, default: str = "") -> str:
    for name in names:
        values = params.get(name)
        if values:
            return values[0]
    return default


def _parse_int(value: str, default: int, *, minimum: int | None = None, maximum: int | None = None) -> int:
    try:
        parsed = int(value)
    except (TypeError, ValueError):
        parsed = default
    if minimum is not None:
        parsed = max(minimum, parsed)
    if maximum is not None:
        parsed = min(maximum, parsed)
    return parsed


def _parse_float(value: str, default: float, *, minimum: float | None = None, maximum: float | None = None) -> float:
    try:
        parsed = float(value)
    except (TypeError, ValueError):
        parsed = default
    if minimum is not None:
        parsed = max(minimum, parsed)
    if maximum is not None:
        parsed = min(maximum, parsed)
    return parsed


def _parse_bool(value: str) -> bool:
    return (value or '').strip().lower() in {'1', 'true', 't', 'yes', 'y', 'on'}

def handle_get(handler) -> bool:
    if handler.path.startswith('/api/similarity-graph'):
        parsed = urlparse(handler.path)
        params = parse_qs(parsed.query)
        min_similarity = _parse_float(_first(params, 'min_similarity', 'threshold', default='0.65'), 0.65, minimum=0.0, maximum=1.0)
        max_neighbors = _parse_int(_first(params, 'max_neighbors', default='12'), 12, minimum=1)
        max_nodes = _parse_int(_first(params, 'max_nodes', default='150'), 150, minimum=1)
        sampling_strategy = (_first(params, 'sampling', default='hybrid') or 'hybrid').strip().lower()
        doc_id = (_first(params, 'doc_id', default='') or '').strip() or None
        refresh = _parse_bool(_first(params, 'refresh', default='false'))
        raw_doc_types = params.get('doc_types', params.get('file_type', []))
        doc_types: list[str] = []
        for item in raw_doc_types:
            doc_types.extend([part.strip().lower() for part in item.split(',') if part.strip()])
        start_date = (_first(params, 'start_date', 'start', default='') or '').strip() or None
        end_date = (_first(params, 'end_date', 'end', default='') or '').strip() or None
        keyword = (params.get('keyword', [''])[0] or '').strip() or None

        payload = get_similarity_graph(
            min_similarity=min_similarity,
            max_neighbors=max_neighbors,
            max_nodes=max_nodes,
            sampling_strategy=sampling_strategy,
            doc_id=doc_id,
            doc_types=doc_types,
            start_date=start_date,
            end_date=end_date,
            keyword=keyword,
            refresh=refresh,
        )

        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(payload, ensure_ascii=False).encode())
        return True

    if handler.path.startswith('/api/documents/metadata'):
        payload = get_documents_metadata()
        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(payload, ensure_ascii=False).encode())
        return True

    if handler.path.startswith('/similar/'):
        file_identifier = unquote(handler.path[len('/similar/'):])
        try:
            similar_docs = find_similar_documents(file_identifier)
            handler.send_response(200)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps(similar_docs, ensure_ascii=False).encode())
        except SimilarDocumentNotFound as exc:
            handler.send_response(404)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(json.dumps({'error': str(exc)}, ensure_ascii=False).encode())
        except Exception as exc:
            handler.send_response(500)
            handler.send_header('Content-Type', 'application/json')
            handler.end_headers()
            handler.wfile.write(
                json.dumps(
                    {
                        'error': '유사 문서 검색 중 오류가 발생했습니다. 색인이 생성되어 있는지 확인해주세요.',
                        'details': str(exc),
                    },
                    ensure_ascii=False,
                ).encode()
            )
        return True

    return False


def handle_post(handler) -> bool:
    if handler.path != '/similar':
        return False

    try:
        payload = _read_json_payload(handler)
    except json.JSONDecodeError:
        handler.send_response(400)
        handler.end_headers()
        handler.wfile.write(b'Invalid JSON payload')
        return True

    file_identifier = payload.get('file_identifier')
    user_filename = payload.get('user_filename')
    refresh = payload.get('refresh', False)

    if not file_identifier:
        handler.send_response(400)
        handler.end_headers()
        handler.wfile.write(b'Missing file_identifier')
        return True

    try:
        similar_docs = find_similar_documents(file_identifier, user_filename=user_filename, refresh=refresh)
        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(similar_docs, ensure_ascii=False).encode())
    except SimilarDocumentNotFound as exc:
        handler.send_response(404)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps({'error': str(exc)}, ensure_ascii=False).encode())
    except Exception as exc:
        handler.send_response(500)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(
            json.dumps(
                {
                    'error': '유사 문서 검색 중 오류가 발생했습니다. 색인이 생성되어 있는지 확인해주세요.',
                    'details': str(exc),
                },
                ensure_ascii=False,
            ).encode()
        )
    return True
