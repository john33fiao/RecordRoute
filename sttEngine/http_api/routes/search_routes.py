from __future__ import annotations

import importlib
import json
from pathlib import Path
from urllib.parse import parse_qs, urlparse

from ...vector_search import search as search_vectors
from ..history import get_active_history
from ..paths import BASE_DIR
from ..search import (
    build_highlight_snippet,
    collect_keyword_matches,
    collect_searchable_documents,
    get_document_text,
)


def _resolve(name: str, default):
    handler_module = importlib.import_module("sttEngine.http_api.handler")
    return getattr(handler_module, name, default)


def handle_get(handler) -> bool:
    if handler.path.startswith('/file_search'):
        parsed = urlparse(handler.path)
        params = parse_qs(parsed.query)
        query = params.get('q', [''])[0].lower()

        results = []
        if query:
            history = _resolve("get_active_history", get_active_history)()
            for record in history:
                filename = record.get('filename', '')
                tags = record.get('tags', [])
                if query in filename.lower() or any(query in t.lower() for t in tags):
                    results.append({'id': record.get('id'), 'filename': filename, 'tags': tags})

        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(results, ensure_ascii=False).encode())
        return True

    if not handler.path.startswith('/search'):
        return False

    parsed = urlparse(handler.path)
    params = parse_qs(parsed.query)

    def _first(name: str, default=None):
        return params.get(name, [default])[0]

    def _csv(name: str) -> set[str]:
        raw = _first(name, '') or ''
        return {token.strip().lower() for token in raw.split(',') if token.strip()}

    query = (_first('query') or _first('q') or '').strip()
    limit_raw = _first('limit')
    start_date = _first('start_date') or _first('start')
    end_date = _first('end_date') or _first('end')
    sort_by = (_first('sort_by', 'similarity') or 'similarity').lower()
    sort_order = (_first('sort_order', 'desc') or 'desc').lower()
    min_score_raw = _first('min_score')
    page_raw = _first('page', '1')
    page_size_raw = _first('page_size', '5')
    include_timing_raw = _first('include_timing', 'false')
    file_types = _csv('file_type')
    status_filter = (_first('status') or '').strip().lower()
    status_task = (_first('status_task') or 'stt').strip().lower()

    include_timing = str(include_timing_raw).lower() in {'1', 'true', 'yes', 'y', 'on'}

    min_score = None
    if min_score_raw not in (None, ''):
        try:
            min_score = float(min_score_raw)
        except ValueError:
            min_score = None

    try:
        page = max(1, int(page_raw))
    except (TypeError, ValueError):
        page = 1

    try:
        page_size = max(1, int(page_size_raw))
    except (TypeError, ValueError):
        page_size = 5

    try:
        limit = int(limit_raw) if limit_raw not in (None, '') else max(10, page * page_size)
        limit = max(1, limit)
    except (TypeError, ValueError):
        limit = max(10, page * page_size)

    try:
        filter_signature = json.dumps(
            {'file_type': sorted(file_types), 'status': status_filter, 'status_task': status_task},
            ensure_ascii=False,
            sort_keys=True,
        )

        response_data = {
            'query': query,
            'limit': limit,
            'keywordMatches': [],
            'similarDocuments': [],
            'sort': {'by': sort_by, 'order': sort_order},
            'filters': {
                'start_date': start_date,
                'end_date': end_date,
                'min_score': min_score,
                'file_type': sorted(file_types),
                'status': status_filter or None,
                'status_task': status_task or None,
            },
            'pagination': {'page': page, 'pageSize': page_size, 'returned': 0, 'hasNext': False},
            'scoreBreakdown': {'keywordWeight': 0.0, 'vectorWeight': 1.0},
        }

        if query:
            documents, path_index = _resolve("collect_searchable_documents", collect_searchable_documents)()
            history = _resolve("get_active_history", get_active_history)()
            history_map = {record.get('id'): record for record in history}

            def _record_passes(record: dict) -> bool:
                if file_types and str(record.get('file_type', '')).lower() not in file_types:
                    return False
                if status_filter:
                    completed = bool((record.get('completed_tasks') or {}).get(status_task, False))
                    if status_filter in {'completed', 'done', 'success'} and not completed:
                        return False
                    if status_filter in {'pending', 'incomplete', 'todo'} and completed:
                        return False
                return True

            filtered_documents = [
                doc for doc in documents if _record_passes(history_map.get(doc['info'].get('record_id'), {}))
            ]

            keyword_matches = _resolve("collect_keyword_matches", collect_keyword_matches)(query, filtered_documents, history_map, limit=limit)
            response_data['keywordMatches'] = keyword_matches

            keyword_paths = {item['file'] for item in keyword_matches}
            keyword_uuids = {item['file_uuid'] for item in keyword_matches}

            search_payload = _resolve("search_vectors", search_vectors)(
                query,
                BASE_DIR,
                top_k=limit,
                start_date=start_date,
                end_date=end_date,
                sort_by=sort_by,
                sort_order=sort_order,
                min_score=min_score,
                page=page,
                page_size=page_size,
                include_timing=include_timing,
                filter_signature=filter_signature,
            )

            if isinstance(search_payload, dict):
                hits = search_payload.get('results', [])
                if include_timing:
                    response_data['timing'] = search_payload.get('timing', {})
                    response_data['cache'] = {'hit': bool(search_payload.get('cache_hit'))}
                response_data['pagination']['hasNext'] = bool(
                    search_payload.get('total_candidates', 0) > page * page_size
                )
            else:
                hits = search_payload
                response_data['pagination']['hasNext'] = len(hits) >= page_size

            similar_documents = []
            for hit in hits:
                rel_path = hit.get('file')
                if not rel_path:
                    continue

                doc = path_index.get(rel_path)
                if doc and (doc['uuid'] in keyword_uuids or rel_path in keyword_paths):
                    continue
                if not doc and rel_path in keyword_paths:
                    continue

                display_name = Path(rel_path).name
                link = f'/download/{rel_path}'
                uploaded_at = hit.get('uploaded_at')
                source_filename = None
                file_uuid = None
                record_id = None
                snippet = ''

                if doc:
                    record = history_map.get(doc['info'].get('record_id'), {})
                    if not _record_passes(record):
                        continue
                    record_id = doc['info'].get('record_id')
                    uploaded_at = record.get('timestamp') or uploaded_at
                    source_filename = record.get('filename')
                    display_name = doc['info'].get('original_filename') or display_name
                    link = f"/download/{doc['uuid']}"
                    file_uuid = doc['uuid']
                    try:
                        snippet = build_highlight_snippet(get_document_text(doc['full_path']), query)
                    except Exception:
                        snippet = ''

                similar_documents.append(
                    {
                        'file_uuid': file_uuid,
                        'file': rel_path,
                        'display_name': display_name,
                        'score': hit.get('score'),
                        'snippet': snippet,
                        'score_breakdown': {
                            'vector_similarity': hit.get('score'),
                            'keyword_overlap': 0.0,
                            'composite': hit.get('score'),
                        },
                        'uploaded_at': uploaded_at,
                        'source_filename': source_filename,
                        'record_id': record_id,
                        'link': link,
                    }
                )

            response_data['similarDocuments'] = similar_documents
            response_data['pagination']['returned'] = len(similar_documents)
            response_data['contract_version'] = 'search-v2'

        handler.send_response(200)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        handler.wfile.write(json.dumps(response_data, ensure_ascii=False).encode())

    except Exception as e:
        print(f'검색 요청 처리 중 오류: {e}')
        handler.send_response(500)
        handler.send_header('Content-Type', 'application/json')
        handler.end_headers()
        error_response = {
            'error': '검색 중 오류가 발생했습니다. Ollama 서버가 실행 중인지 확인하고, 임베딩 모델이 설치되어 있는지 확인해주세요.',
            'details': str(e),
        }
        handler.wfile.write(json.dumps(error_response, ensure_ascii=False).encode())
    return True
