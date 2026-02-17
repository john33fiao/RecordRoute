# 백엔드·프론트엔드 분리 배포 + OpenAPI 연동 계획

## 목표
- 기존 RecordRoute를 **백엔드 API 서버**와 **프론트엔드 웹 앱**으로 분리 배포한다.
- 두 서비스를 각각 독립 Docker 이미지/컨테이너로 운영한다.
- OpenAPI 스펙을 기준으로 계약(Contract) 기반 연동을 구현한다.

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
  - `DELETE /history` (존재 시)
- WebSocket은 OpenAPI 본문보다 별도 섹션 문서 또는 AsyncAPI 병행 권장

## 단계별 실행 계획

### Phase 0. 사전 정리 (0.5~1일)
1. 현재 API/응답 스키마를 코드 기준으로 동결 목록 작성
2. 프론트엔드가 사용하는 실제 필드 의존성 조사
3. 파일 업로드/저장 경로와 볼륨 요구사항 정리

### Phase 1. OpenAPI 초안 작성 (1일)
1. `openapi/openapi.yaml` 신규 작성
2. 핵심 스키마 정의
   - `ProcessResponse`, `TaskProgress`, `HistoryItem`, `SearchResponse`, `ErrorResponse`
3. 예제(Example)와 에러코드(400/404/500) 명시
4. Swagger UI/ReDoc로 사람이 검토 가능한 문서 제공

### Phase 2. 백엔드 컨테이너화 (1~2일)
1. `sttEngine` 기준 `Dockerfile.backend` 작성
2. 런타임 환경변수 정리
   - 모델명, 업로드 경로, DB 경로, CORS 허용 Origin, 로그 레벨
3. 볼륨 설계
   - 업로드 파일, 결과물, DB, 로그 분리
4. 헬스체크 엔드포인트(`GET /health`) 추가

### Phase 3. 프론트엔드 컨테이너화 (1일)
1. 프론트 빌드/서빙 방식 결정
   - 정적 파일이면 Nginx/Node 경량 이미지 사용
2. API Base URL을 환경변수로 분리
   - 예: `VITE_API_BASE_URL` 또는 런타임 치환 방식
3. WebSocket URL도 환경별 설정 분리

### Phase 4. Compose 기반 통합 (0.5~1일)
1. 루트에 `docker-compose.yml` 추가
2. 서비스 정의
   - `backend`, `frontend` (+ 필요 시 `ollama` 외부 의존 설명)
3. 네트워크, 볼륨, depends_on/healthcheck 연결
4. 로컬 통합 테스트 시나리오 확정

### Phase 5. OpenAPI 기반 프론트 연동 고도화 (1~2일)
1. OpenAPI로 타입/클라이언트 생성 도입(선택)
   - JS/TS 클라이언트 자동생성 또는 수동 fetch 래퍼
2. 프론트 직접 경로 하드코딩 제거
3. 계약 테스트(Contract Test) 추가
   - 스펙과 실제 응답 필드 불일치 탐지

### Phase 6. 운영 배포/관측성 (1일)
1. 로그 수집/회전 정책 컨테이너 환경에 맞춤화
2. 메트릭/알람(에러율, 처리시간, 큐 길이) 최소 구성
3. 롤백 전략
   - 이미지 태그 고정 + 이전 태그 즉시 복귀

## 리스크와 대응
- **Whisper/Ollama 의존성으로 인한 이미지 비대화**
  - 대응: backend 이미지와 모델 다운로드 시점 분리, 캐시 전략 적용
- **장시간 작업과 타임아웃**
  - 대응: 비동기 작업 큐 유지, 진행률 API/WS 우선, reverse proxy timeout 조정
- **CORS/WS 연결 실패**
  - 대응: `/api`, `/ws` 리버스프록시 단일 도메인 전략 채택
- **OpenAPI와 실제 구현 불일치**
  - 대응: CI에서 스펙 검증 + 샘플 응답 검증 추가

## 산출물 체크리스트
- [ ] `openapi/openapi.yaml`
- [ ] `Dockerfile.backend`
- [ ] `Dockerfile.frontend`
- [ ] `docker-compose.yml`
- [ ] 프론트 API/WS 환경변수화
- [ ] `/health` 엔드포인트
- [ ] 배포/운영 문서(README 섹션 확장)

## 추천 우선순위
1. OpenAPI 명세 먼저 확정 (프론트/백엔드 계약 고정)
2. 백엔드 컨테이너화
3. 프론트 컨테이너화
4. Compose 통합 및 E2E 점검
5. 운영 배포 자동화(CI/CD)
