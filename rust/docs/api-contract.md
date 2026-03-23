# Rust API Contract Inventory (P0)

이 문서는 Rust 마이그레이션 **P0 계약 동결 단계**에서 생성/유지하는 fixture 인벤토리입니다.

## 고정 결정사항
- Rust 1차 task registry는 **memory-only**로 가정합니다.
- 정적/파일 서빙은 **Rust 서버 직접 서빙**을 기본으로 하되, 현재 Docker/Nginx 프록시 배치와도 **호환**되어야 합니다.
- fixture 범위는 **프론트 사용 API + 핵심 고위험 계약**으로 제한합니다.
- capture 방식은 **hybrid**입니다.
  - 실서버 재현이 쉬운 HTTP/WS 케이스는 ephemeral server 기준
  - `models`, destructive safe-mode guard처럼 논리 중심 케이스는 기존 handler/unit-test 패턴 재사용

## 디렉터리 구조
```text
rust/fixtures/contracts/
├── http/
├── ws/
└── meta/
    └── manifest.json
```

## HTTP fixture inventory

| Fixture | Endpoint | Why frozen in P0 | Key invariant |
| --- | --- | --- | --- |
| `get_history_list.json` | `GET /history` | 프론트 메인 리스트 | `DB/...` alias 유지 |
| `get_tasks_memory_registry.json` | `GET /tasks` | Rust memory-only task registry 기준 고정 | `progress_percent`, `eta_seconds` 보존 |
| `get_progress_task.json` | `GET /progress/{task_id}` | 진행률 카드/폴링 기준 | 표준 `error` 필드 유지 |
| `get_segments_sidecar.json` | `GET /segments/{file_identifier}` | STT segment schema 고정 | `{start,end,text,speaker}` |
| `get_download_file.json` | `GET /download/{uuid_or_path}` | 파일 다운로드/직접 서빙 기준 | direct serve + proxy compatibility |
| `get_search_normalized_v2.json` | `GET /search` | 고위험 query normalization | `contract_version: search-v2` |
| `get_similarity_graph_filtered.json` | `GET /api/similarity-graph` | 프론트 그래프 진단 필드 유지 | `meta.filters`, `sampling`, `neighbor_strategy`, `incremental` |
| `get_models_default_provider.json` | `GET /models` | provider 기본값 | `models_by_task` 포함 |
| `get_models_llamacpp_query.json` | `GET /models?provider=llamacpp` | provider query 차이 | top-level `models` 전환 |
| `post_upload_success.json` | `POST /upload` | 업로드 후 Rust/Python 연동 기준 | `file_path`는 DB alias |
| `post_process_step_normalization.json` | `POST /process` | step normalization/diarize skipped | `summarize -> summary` |
| `post_cancel_task.json` | `POST /cancel` | task cancellation 기준 | memory-only registry 대상 |
| `post_reset_forbidden_safe_mode.json` | `POST /reset` | destructive guard 기본 차단 | `403` + `destructive_api_protected` |
| `post_reset_authorized_token.json` | `POST /reset` | destructive guard header auth | header token 성공 |
| `post_reset_all_tasks_forbidden.json` | `POST /reset_all_tasks` | destructive guard 범위 확인 | 동일 보호 정책 |
| `post_update_filename.json` | `POST /update_filename` | 프론트 편집 흐름 | record identity 유지 |
| `post_update_stt_text.json` | `POST /update_stt_text` | 수동 전사 수정 | record_id 기반 갱신 |
| `post_check_existing_stt.json` | `POST /check_existing_stt` | 기존 전사 reuse | DB alias 경로 반환 |
| `post_reset_summary_embedding_session.json` | `POST /reset_summary_embedding` | body session auth 성공 | session credential path |
| `post_similar_documents.json` | `POST /similar` | 유사문서 POST 계약 | GET variant와 별개 |
| `post_delete_forbidden_safe_mode.json` | `POST /delete` | destructive guard 기본 차단 | `403` 보호 |
| `post_delete_records_authorized.json` | `POST /delete_records` | bulk destructive auth 성공 | token auth 성공 |
| `post_shutdown_authorized.json` | `POST /shutdown` | 서버 종료 보호 | safe mode 인증 필요 |

## WebSocket fixture inventory

| Fixture | Endpoint | Why frozen in P0 | Key invariant |
| --- | --- | --- | --- |
| `progress_success.json` | `/ws` | 정상 진행 frame 순서 | ordered frames + `progress_percent`/`eta_seconds` |
| `progress_error.json` | `/ws` | 오류 frame 필드 보존 | flattened fields + nested `error` object |

## 정규화 대상
fixture 저장 전 다음 값은 전부 정규화합니다.
- generated id (`task_id`, `record_id`, 업로드 UUID 등)
- timestamp / elapsed clock
- localhost 포트
- temp path
- OS 절대경로
- 동적 URL 조각
- multipart boundary
- 인증 토큰/세션값

## P0 범위 밖 엔드포인트
다음 엔드포인트는 문서에만 남기고 fixture는 생성하지 않습니다.
- `GET /file_search`
- `GET /similar/{uuid_or_path}`
- `GET /api/documents/metadata`
- `GET /cache/stats`
- `GET /cache/cleanup`
- `GET /metrics/workflow`

## Drift policy
- fixture drift는 **사양 변경**으로 취급합니다.
- drift가 의도적이면 같은 변경에서 다음을 함께 갱신합니다.
  1. fixture JSON
  2. `rust/SPEC.md`
  3. `rust/TODO.md`
