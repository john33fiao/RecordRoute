from __future__ import annotations

import json
from urllib.parse import parse_qs, unquote, urlparse

from ...similarity_matrix import get_documents_metadata, get_similarity_graph
from .similar_documents import SimilarDocumentNotFound, find_similar_documents


def _read_json_payload(handler):
    length = int(handler.headers.get('Content-Length', 0))
    return json.loads(handler.rfile.read(length)) if length else {}


def _bad_request(handler, message: str) -> None:
    handler.send_response(400)
    handler.send_header('Content-Type', 'application/json')
    handler.end_headers()
    handler.wfile.write(json.dumps({'error': message}, ensure_ascii=False).encode())


def _parse_float(value: str, default: float) -> float:
    try:
        return float(value)
    except (TypeError, ValueError):
        return default


def _parse_int(value: str, default: int) -> int:
    try:
        return int(value)
    except (TypeError, ValueError):
        return default


def handle_get(handler) -> bool:
    if handler.path.startswith('/api/similarity-graph'):
        parsed = urlparse(handler.path)
        params = parse_qs(parsed.query)
        min_similarity = _parse_float(params.get('min_similarity', params.get('threshold', ['0.65']))[0], 0.65)
        max_neighbors = _parse_int(params.get('max_neighbors', ['12'])[0], 12)
        max_nodes = _parse_int(params.get('max_nodes', ['150'])[0], 150)
        sampling_strategy = (params.get('sampling', ['hybrid'])[0] or 'hybrid').lower()
        doc_id = (params.get('doc_id', [''])[0] or '').strip() or None
        refresh = params.get('refresh', ['false'])[0].lower() == 'true'
        candidate_strategy = (params.get('candidate_strategy', ['auto'])[0] or 'auto').lower()
        if candidate_strategy not in {'auto', 'exact', 'lsh'}:
            _bad_request(handler, "Invalid candidate_strategy. Use one of: auto, exact, lsh")
            return True

        payload = get_similarity_graph(
            min_similarity=min_similarity,
            max_neighbors=max_neighbors,
            max_nodes=max_nodes,
            sampling_strategy=sampling_strategy,
            candidate_strategy=candidate_strategy,
            doc_id=doc_id,
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
