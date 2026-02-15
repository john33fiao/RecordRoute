from __future__ import annotations

import json
from urllib.parse import parse_qs, unquote, urlparse

from ...similarity_matrix import get_documents_metadata, get_similarity_graph


def _read_json_payload(handler):
    length = int(handler.headers.get('Content-Length', 0))
    return json.loads(handler.rfile.read(length)) if length else {}


def handle_get(handler) -> bool:
    if handler.path.startswith('/api/similarity-graph'):
        parsed = urlparse(handler.path)
        params = parse_qs(parsed.query)
        min_similarity = float(params.get('min_similarity', params.get('threshold', ['0.65']))[0])
        max_neighbors = int(params.get('max_neighbors', ['12'])[0])
        max_nodes = int(params.get('max_nodes', ['150'])[0])
        sampling_strategy = (params.get('sampling', ['hybrid'])[0] or 'hybrid').lower()
        doc_id = (params.get('doc_id', [''])[0] or '').strip() or None
        refresh = params.get('refresh', ['false'])[0].lower() == 'true'

        payload = get_similarity_graph(
            min_similarity=min_similarity,
            max_neighbors=max_neighbors,
            max_nodes=max_nodes,
            sampling_strategy=sampling_strategy,
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
        handler._serve_similar_documents(file_identifier)
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

    handler._serve_similar_documents_with_filename(file_identifier, user_filename, refresh)
    return True
