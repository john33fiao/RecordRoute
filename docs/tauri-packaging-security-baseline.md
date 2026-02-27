# Tauri 패키징/보안 기준선 (WBS 9.3)

이 문서는 WBS 9.3(패키징/보안 체계 수립)의 기준선 문서입니다.

목표는 **Rust 오케스트레이터 + C++ 엔진 독립 프로세스** 아키텍처를 유지하면서,
Tauri 전환 시 권한/설정/종료 복구 정책을 최소 권한 원칙으로 고정하는 것입니다.

## 1. `tauri.conf.json` 최소 권한 정책

`src-tauri/tauri.conf.json` 작성/갱신 시 아래 정책을 기본값으로 적용합니다.

- `allowlist`는 deny-by-default로 시작하고, 실제 사용 기능만 명시적으로 허용합니다.
  - `shell`: 엔진 실행에 필요한 실행 파일 목록만 스코프 지정
  - `fs`: 앱 데이터 디렉터리(`$APPDATA/recordroute` 등) + 로그/모델 manifest 경로만 허용
  - `http`: 로컬 오케스트레이터(`http://127.0.0.1:<port>`) 및 내부 Swagger 포트만 허용
  - `window`: 멀티 윈도우/임의 URL 네비게이션 금지
- CSP는 `default-src 'self'`를 기준으로 시작하고, 개발 모드 외 외부 origin을 금지합니다.
  - `connect-src`는 `http://127.0.0.1:<orchestrator_port> ws://127.0.0.1:<orchestrator_port>`만 허용
  - `frame-src`/`child-src`는 `'none'`
  - `script-src`는 번들된 자산(`'self'`)만 허용, `unsafe-eval` 금지
- 파일/프레임 권한은 “필요 시 추가, 기본은 비활성” 원칙을 유지합니다.
  - 파일 선택 대화상자/드래그&드롭은 사용자 업로드 경로에 한정
  - 임의 로컬 파일 탐색/전체 디스크 접근 금지

## 2. 환경변수/비밀 관리 전략

런타임 설정은 `RECORDROUTE_*` 네임스페이스로 통일합니다.

- 허용 범주
  - 네트워크: `RECORDROUTE_API_HOST`, `RECORDROUTE_API_PORT`
  - 큐/동시성: `RECORDROUTE_*_QUEUE_CAPACITY`, `RECORDROUTE_*_CONCURRENCY`
  - 타임아웃: `RECORDROUTE_ENGINE_TIMEOUT_SECS`, `RECORDROUTE_JOB_TIMEOUT_*`, `RECORDROUTE_STT_TIMEOUT_*`
  - 엔진 URL: `RECORDROUTE_*_ENGINE_URL`
  - 로그: `RECORDROUTE_LOG_LEVEL`
  - 모델: `RECORDROUTE_MODEL_ROOT` + `models/**/manifest.yml|yaml|json` 참조
- 주입 경로
  - 개발: `.env.local`(git 추적 제외) → Tauri 런처가 whitelist 키만 전달
  - CI/배포: OS keychain/CI secret store → 런타임 프로세스 환경으로 주입
- 비밀 관리
  - API 토큰/인증 정보는 `RECORDROUTE_SECRET_*` 접두사로 분리하고 로그/metrics 출력 금지
  - 비밀값은 프론트로 전달 금지(백엔드 프로세스 경계 내에서만 사용)
  - 장애 분석 로그에는 비밀 마스킹 규칙(`***`)을 적용

## 3. 종료/재시작/업그레이드 재진입과 `engine_manager` 정합성

`src/engine_manager.rs` 동작을 기준으로 운영 상태를 아래처럼 고정합니다.

- 상태 모델
  - `Booting`: 엔진 spawn + readiness probe 대기
  - `Ready`: health check 통과, 작업 수락 가능
  - `Degraded`: readiness 실패 또는 비정상 종료로 backoff 재기동 중
  - `Stopping`: `shutdown` 시그널 수신 후 graceful 종료 진행
  - `Stopped`: supervisor task join 완료
- 이벤트 정합성
  - 앱 종료(`shutdown`) 요청 시 `Stopping → Stopped`가 완료될 때까지 Tauri 프로세스 종료를 지연
  - 엔진 비정상 종료 시 `Degraded`로 전이하고 exponential backoff 재시작 정책 유지
  - 업그레이드 재진입 시 이전 인스턴스가 `Stopped`가 아니면 신규 인스턴스가 엔진 spawn을 시작하지 않음
- 운영 가드
  - `Ready` 상태가 아니면 `/readyz`는 `503 degraded`를 유지
  - 재시작 루프/포트 충돌 발생 시 런처 로그 경로(`artifacts/tauri-lifecycle/<ts-os-pid>/`)에 원인 기록
  - 롤백 시 이전 버전 바이너리 재기동 전에 `shutdown_grace` + 강제 종료 fallback을 동일하게 적용

## 4. 수용 기준(Definition of Done)

WBS 9.3은 아래 3개를 모두 충족해야 완료로 판정합니다.

1. `tauri.conf.json` allowlist/CSP/파일·프레임 권한이 본 문서의 최소 권한 정책과 일치한다.
2. `RECORDROUTE_*` 환경변수 주입 경로, 모델 경로, 비밀 관리(마스킹/비노출) 전략이 운영 문서에 반영된다.
3. `shutdown`/장애 재시작/업그레이드 재진입 절차가 `engine_manager` 상태 전이(`Booting/Ready/Degraded/Stopping/Stopped`)와 모순되지 않는다.
