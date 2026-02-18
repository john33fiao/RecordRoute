# 백엔드·프론트엔드 분리 배포 + OpenAPI 연동 계획

## 구현 상태 점검 (코드베이스 기준, 2026-02-18)

아래 항목은 현재 저장소 구현(`Dockerfile`, `docker-compose*.yml`, `sttEngine/http_api/*`, `frontend/src/*`, `README.md`)을 기준으로 대조한 결과입니다.

### 전체 판정 요약
- **서비스 분리 배포 목표:** 미완료
  - 현재 `Dockerfile` + `docker-compose.yml`은 `recordroute` **단일 컨테이너**에서 백엔드와 프론트 정적 빌드/서빙을 함께 처리.
- **OpenAPI 계약 기반 연동:** 미착수
  - `openapi/openapi.yaml` 및 Swagger/ReDoc 노출 구성이 없음.
- **컨테이너 운영 기반:** 부분 완료
  - Docker/Compose 실행 경로, provider profile(`ollama`, `llamacpp`), 볼륨(`./DB:/data/DB`)은 이미 존재.

### 산출물 체크리스트 (실구현 반영)
- [ ] `openapi/openapi.yaml`
- [ ] `Dockerfile.backend` (현재는 통합 `Dockerfile`만 존재)
- [ ] `Dockerfile.frontend`
- [x] `docker-compose.yml` (단, 백엔드/프론트 분리 구조는 아님)
- [ ] 프론트 API/WS 환경변수화 (`VITE_API_BASE_URL`, `VITE_WS_URL` 등)
- [ ] `/health` 엔드포인트
- [ ] 배포/운영 문서(README 분리 아키텍처 기준 확장)

## 작업 방향 TODO (우선순위 체크박스)

### P0 — 계약/구조 분리 착수
- [ ] `openapi/openapi.yaml` 초안 작성
  - [ ] `POST /process`, `GET /progress/{task_id}`, `GET /history`, `GET /search`, `POST /delete_records`(현행 API 기준) 명세 반영
  - [ ] 공통 에러 스키마(`error`, `error_code`, `retryable`, `failed_step`) 반영
  - [ ] WebSocket(`ws://<host>:8765`)은 별도 섹션(또는 AsyncAPI 링크)으로 문서화
- [ ] 백엔드/프론트 Dockerfile 분리
  - [ ] `Dockerfile.backend`: Python 런타임 + `sttEngine` 실행 전용
  - [ ] `Dockerfile.frontend`: Vite build 결과 정적 서빙 전용
- [ ] Compose를 `backend` + `frontend` 2서비스 기준으로 재구성
  - [ ] 기존 provider profile(`ollama`, `llamacpp`) 연동 유지
  - [ ] `depends_on` + `healthcheck` 연결

### P1 — 연동 안정화
- [ ] 백엔드 `GET /health` 추가 (HTTP 200 + 최소 진단 정보)
- [ ] 프론트 API Base URL 환경변수화
  - [ ] `frontend/src/api/client.ts`의 상대경로 호출을 환경변수 기반으로 전환
  - [ ] 개발/운영 기본값 및 fallback 정책 정의
- [ ] 프론트 WebSocket URL 환경변수화
  - [ ] `frontend/src/hooks/useWebSocket.ts`의 `:8765` 하드코딩 제거
- [ ] Gateway/Nginx 경유 `/api`, `/ws` 프록시 전략 문서화

### P2 — 계약 검증/운영 고도화
- [ ] OpenAPI 기반 타입/클라이언트 생성 도입 여부 결정 및 PoC
- [ ] Contract test 추가(스펙-실응답 불일치 탐지)
- [ ] README Docker 섹션을 분리 배포 아키텍처 기준으로 재작성
- [ ] 로그/메트릭/롤백 운영 절차를 컨테이너 배포 기준으로 문서화

## 목표
- 기존 RecordRoute를 **백엔드 API 서버**와 **프론트엔드 웹 앱**으로 분리 배포한다.
- 두 서비스를 각각 독립 Docker 이미지/컨테이너로 운영한다.
- OpenAPI 스펙을 기준으로 계약(Contract) 기반 연동을 구현한다.

---

## 구현 상태 점검 (코드베이스 기준, 2026-02-18)

### 점검 결론
- **분리 배포 아키텍처**: ❌ 미완료
  - 현재는 `recordroute` 단일 서비스가 백엔드 실행 + 프론트 빌드 산출물 서빙을 함께 담당한다.
- **OpenAPI 계약 문서화**: ❌ 미착수
  - `openapi/openapi.yaml` 파일 및 Swagger/ReDoc 노출 경로가 없다.
- **Docker 기반 운영 기초**: ✅ 부분 완료
  - `docker-compose.yml`/`docker-compose.gpu.yml`, provider profile(`ollama`, `llamacpp`), DB 볼륨 마운트가 이미 있다.

### 점검 근거(파일)
- 통합 이미지/엔트리포인트: `Dockerfile`, `docker/entrypoint.sh`
- 단일 앱 서비스 중심 Compose: `docker-compose.yml`, `docker-compose.gpu.yml`
- 프론트 API 호출 형태(상대경로): `frontend/src/api/client.ts`
- 프론트 WS URL 하드코딩(`:8765`): `frontend/src/hooks/useWebSocket.ts`
- 백엔드 라우팅(health 미존재 확인): `sttEngine/http_api/handler.py`, `sttEngine/http_api/routes/*`
- 운영 문서 기준: `README.md`, `AGENTS.md`, `CLAUDE.md`, `GEMINI.md`

---

## 현재 상태 요약
- 백엔드는 `sttEngine/server.py`에서 HTTP API와 WebSocket을 함께 제공한다.
- 프론트는 주 코드베이스 `frontend/src/*`(React + Vite)에서 `frontend/src/api/client.ts`를 통해 백엔드 엔드포인트를 호출하며, `frontend/legacy/*`는 fallback로 유지된다.
- 실행은 루트 `run.sh`/`run.bat` 기반 단일 서버 중심 흐름이며, Docker 실행 경로는 `docker-compose.yml`/`docker-compose.gpu.yml`가 제공된다.

## 아키텍처 방향

### 1) 서비스 분리
- **backend 서비스**
  - 책임: 파일 업로드, STT/교정/요약 작업 큐, 진행률/이력 API, 검색 API, WebSocket 상태 알림
  - 포트 예시: `8080`
- **frontend 서비스**
  - 책임: 정적 자산 서빙 + 백엔드 API 호출 UI
  - 포트 예시: `3000`(개발) / `80`(운영)

### 2) 네트워크/라우팅 전략
- 로컬/개발: `docker compose` 내부 네트워크에서 `frontend -> backend` 통신
- 운영: 아래 중 택1
  - (권장) API Gateway/Nginx로 `/api` 및 `/ws`를 backend로 프록시 (CORS 최소화)
  - 또는 frontend 도메인에서 backend 별도 도메인 호출 + CORS 허용

### 3) OpenAPI 중심 계약
- 백엔드 엔드포인트를 OpenAPI 3.x 문서(`openapi.yaml`)로 명세화
- 우선 대상 API
  - `POST /process`
  - `GET /progress/{task_id}`
  - `GET /history`
  - `GET /search`
  - `POST /delete_records` (현행 코드 기준 파괴적 삭제 API)
- WebSocket은 OpenAPI 본문보다 별도 섹션 문서 또는 AsyncAPI 병행 권장

---

## 단계별 실행 계획 + 상태

### Phase 0. 사전 정리 (0.5~1일) — ⚠️ 진행 필요
1. 현재 API/응답 스키마를 코드 기준으로 동결 목록 작성
2. 프론트엔드가 사용하는 실제 필드 의존성 조사
3. 파일 업로드/저장 경로와 볼륨 요구사항 정리

### Phase 1. OpenAPI 초안 작성 (1일) — ❌ 미착수
1. `openapi/openapi.yaml` 신규 작성
2. 핵심 스키마 정의
   - `ProcessResponse`, `TaskProgress`, `HistoryItem`, `SearchResponse`, `ErrorResponse`
3. 예제(Example)와 에러코드(400/404/500) 명시
4. Swagger UI/ReDoc로 사람이 검토 가능한 문서 제공

### Phase 2. 백엔드 컨테이너화 (1~2일) — ⚠️ 부분 완료
1. `sttEngine` 기준 `Dockerfile.backend` 작성
2. 런타임 환경변수 정리
   - 모델명, 업로드 경로, DB 경로, CORS 허용 Origin, 로그 레벨
3. 볼륨 설계
   - 업로드 파일, 결과물, DB, 로그 분리
4. 헬스체크 엔드포인트(`GET /health`) 추가

> 참고: 현재는 통합 `Dockerfile` + 단일 `recordroute` 서비스만 존재.

### Phase 3. 프론트엔드 컨테이너화 (1일) — ⚠️ 부분 완료
1. 프론트 빌드/서빙 방식 결정
   - 정적 파일이면 Nginx/Node 경량 이미지 사용
2. API Base URL을 환경변수로 분리
   - 예: `VITE_API_BASE_URL` 또는 런타임 치환 방식
3. WebSocket URL도 환경별 설정 분리

> 참고: 현재 프론트는 통합 이미지 빌드 단계에서 `npm run build` 후 백엔드가 dist 파일을 서빙.

### Phase 4. Compose 기반 통합 (0.5~1일) — ⚠️ 기반만 존재
1. 루트에 `docker-compose.yml` 추가
2. 서비스 정의
   - `backend`, `frontend` (+ 필요 시 `ollama` 외부 의존 설명)
3. 네트워크, 볼륨, depends_on/healthcheck 연결
4. 로컬 통합 테스트 시나리오 확정

> 참고: 현재 Compose에는 `recordroute` + 선택 profile 서비스(`ollama`, `llamacpp`) 구조만 구현.

### Phase 5. OpenAPI 기반 프론트 연동 고도화 (1~2일) — ❌ 미착수
1. OpenAPI로 타입/클라이언트 생성 도입(선택)
   - JS/TS 클라이언트 자동생성 또는 수동 fetch 래퍼
2. 프론트 직접 경로 하드코딩 제거
3. 계약 테스트(Contract Test) 추가
   - 스펙과 실제 응답 필드 불일치 탐지

### Phase 6. 운영 배포/관측성 (1일) — ⚠️ 초기 수준
1. 로그 수집/회전 정책 컨테이너 환경에 맞춤화
2. 메트릭/알람(에러율, 처리시간, 큐 길이) 최소 구성
3. 롤백 전략
   - 이미지 태그 고정 + 이전 태그 즉시 복귀

---

## 작업 방향 TODO (우선순위 체크박스)

### P0 — 계약 고정 + 서비스 분리 골격
- [ ] `openapi/openapi.yaml` 초안 작성
  - [ ] `POST /process`, `GET /progress/{task_id}`, `GET /history`, `GET /search`, `POST /delete_records` 명세
  - [ ] 공통 오류 필드(`error`, `error_code`, `retryable`, `failed_step`) 반영
  - [ ] WS(8765) 별도 문서(또는 AsyncAPI) 링크 추가
- [ ] `Dockerfile.backend`/`Dockerfile.frontend` 분리
- [ ] `docker-compose.yml`을 `backend` + `frontend` 2서비스로 개편
  - [ ] 기존 `ollama`/`llamacpp` profile 연동 유지
  - [ ] `depends_on` + healthcheck 기반 기동 순서 정리

### P1 — 연동 안정화
- [ ] 백엔드 `GET /health` 구현
- [ ] 프론트 API Base URL 환경변수화
  - [ ] `frontend/src/api/client.ts` 상대 경로 호출 정리
- [ ] 프론트 WS URL 환경변수화
  - [ ] `frontend/src/hooks/useWebSocket.ts`의 `:8765` 하드코딩 제거
- [ ] Reverse proxy(`/api`, `/ws`) 운영 템플릿(Nginx 등) 문서화

### P2 — 계약 검증/운영 고도화
- [ ] OpenAPI 기반 타입/클라이언트 생성 도입 여부 결정 + PoC
- [ ] Contract Test 추가(스펙-실응답 불일치 탐지)
- [ ] README Docker 섹션을 분리 아키텍처 기준으로 갱신
- [ ] 로그/메트릭/롤백 운영 절차 문서화

---

## 리스크와 대응
- **Whisper/Ollama 의존성으로 인한 이미지 비대화**
  - 대응: backend 이미지와 모델 다운로드 시점 분리, 캐시 전략 적용
- **장시간 작업과 타임아웃**
  - 대응: 비동기 작업 큐 유지, 진행률 API/WS 우선, reverse proxy timeout 조정
- **CORS/WS 연결 실패**
  - 대응: `/api`, `/ws` 리버스프록시 단일 도메인 전략 채택
- **OpenAPI와 실제 구현 불일치**
  - 대응: CI에서 스펙 검증 + 샘플 응답 검증 추가

## 산출물 체크리스트 (최신 상태 반영)
- [ ] `openapi/openapi.yaml`
- [ ] `Dockerfile.backend`
- [ ] `Dockerfile.frontend`
- [x] `docker-compose.yml` (현재는 단일 앱 서비스 구조)
- [ ] 프론트 API/WS 환경변수화
- [ ] `/health` 엔드포인트
- [ ] 배포/운영 문서(README 섹션 확장)

## 추천 우선순위
1. OpenAPI 명세 먼저 확정 (프론트/백엔드 계약 고정)
2. 백엔드 컨테이너화
3. 프론트 컨테이너화
4. Compose 통합 및 E2E 점검
5. 운영 배포 자동화(CI/CD)
