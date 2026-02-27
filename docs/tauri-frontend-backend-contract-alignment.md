# Tauri 전환 프론트-백 계약 정합성 기준 (WBS 9.2)

이 문서는 WBS 9.2(프론트-백 계약 정합성 선결)의 단일 기준입니다.
기준 시점은 현재 코드베이스(`frontend/src/api/client.ts`, `frontend/vite.config.ts`, `frontend/src/runtime/endpoints.ts`)입니다.

## 1) 갭 요약 및 우선순위

### P0 (즉시 고정)

1. **API 경로 계약 불일치 정리**
   - 프론트 현재 호출: `/upload`, `/process`, `/tasks`, `/progress/{taskId}`, `/shutdown`
   - Rust/OpenAPI 핵심 계약: `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`
   - 정책: 9.2 단계에서는 **프론트 경로를 즉시 제거하지 않고**, Rust 오케스트레이터에 호환 레이어를 두거나 프론트 어댑터를 통해 단계 전환한다.

2. **환경변수 단일 기준 고정**
   - API base 단일 기준: `VITE_API_BASE_URL`
   - WebSocket 단일 기준: `VITE_WS_URL`
   - Tauri 로컬 런타임 fallback: `VITE_TAURI_BACKEND_URL` → 미지정 시 `127.0.0.1:8080`

### P1 (9.3 진입 전 확정)

3. **Tauri API 라우트 표준 결정 (`/api/*` 래핑 여부)**
   - 정책: **신규 Rust 계약 경로는 비래핑(non-`/api`)을 기본**으로 유지한다.
   - 예외: 기존 프론트 기능 중 `/api/*`를 이미 사용 중인 read-heavy endpoint(`'/api/similarity-graph'` 등)는 마이그레이션 완료 전까지 유지하고, OpenAPI Rust 핵심 계약에는 포함하지 않는다.

## 2) 개발/배포 모드별 기준

## 2-1. 개발 모드(Vite dev server)

- 프록시 타깃 해석 우선순위
  1) `VITE_API_BASE_URL`
  2) `VITE_TAURI_BACKEND_URL`
  3) 기본값 `http://localhost:8080`
- 위 기준은 `frontend/vite.config.ts`와 동일하게 유지한다.

## 2-2. 웹 배포 모드(브라우저)

- `resolveApiBaseUrl()`
  - `VITE_API_BASE_URL` 우선
  - 미설정 시 same-origin(빈 base) 사용
- `resolveWebSocketUrl()`
  - `VITE_WS_URL` 우선
  - 미설정 시 `window.location` 기반으로 `ws(s)://<host>/ws`

## 2-3. Tauri 배포 모드

- `resolveApiBaseUrl()`
  - `VITE_API_BASE_URL` > `VITE_TAURI_BACKEND_URL` > `http://127.0.0.1:8080`
- `resolveWebSocketUrl()`
  - `VITE_WS_URL` > (`VITE_TAURI_BACKEND_URL`를 ws로 치환 + `/ws`) > `ws://127.0.0.1:8080/ws`
- 정책: Tauri 릴리스에서는 `VITE_TAURI_BACKEND_URL`을 명시해 오리진 추론 의존을 제거한다.

## 3) 이행 순서

1. 9.2 단계: 본 문서 기준으로 계약/우선순위 확정(완료)
2. 9.3 단계: 보안/패키징 정책과 함께 환경변수 주입 경로 확정
3. 9.4~9.5 단계: `/upload|/process|/tasks|/progress|/shutdown` → `/jobs` 기반 API로 점진 전환 및 smoke gate 연동

## 4) 완료 판정(9.2)

- [x] 프론트-백 API 갭 문서화 및 우선순위(P0/P1) 확정
- [x] `vite proxy`와 런타임 API base 환경변수 단일 기준(`VITE_API_BASE_URL`) 문서 고정
- [x] `resolveWebSocketUrl()`의 Tauri 동작 정책 문서 고정
- [x] Tauri 도입 전/후 라우트 표준(비래핑 기본 + 기존 `/api/*` 한시 유지) 결정
