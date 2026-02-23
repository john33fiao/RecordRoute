# `main.rs` 비대화 대응 분할 가이드

이 문서는 RecordRoute Rust 오케스트레이터에서 `src/main.rs`가 커질 때 적용할 **실무형 분할 기준**을 정리합니다.

핵심 원칙은 API 엔드포인트 단위 분할보다, **도메인 책임 + 런타임 경계** 기준 분할입니다.

## 왜 API 단위 분할만으로는 부족한가

- 현재 API 수(`GET /healthz`, `GET /readyz`, `POST /jobs`, `GET /jobs/{job_id}`) 자체는 많지 않습니다.
- 복잡성의 중심은 엔드포인트 개수보다 **잡 상태 전이/큐 압력 제어/엔진 호출 타임아웃/재시도 정책**에 있습니다.
- API별 파일 분할만 먼저 하면 파일 수는 늘어나도 도메인 응집도가 떨어질 수 있습니다.

## 권장 분할 축 (우선순위)

1. **Domain (잡 모델/상태 전이)**
   - `Job`, `JobId`, `JobStatus`, `JobStore`와 전이 규칙
2. **Engine Integration (엔진 경계)**
   - `EngineKind`, `EngineClient`, HTTP 호출, timeout/retry/backoff
3. **Runtime/Orchestration (실행 경계)**
   - dispatcher/worker 조립, 앱 상태 구성, 서버 부팅
4. **Transport (입출력 경계)**
   - 라우팅, 요청 파싱, 응답 직렬화
5. **Config (설정 경계)**
   - env 파싱, 기본값, 검증/주소 계산

> 즉, “API를 쪼갠다”가 아니라 “운영 책임을 쪼갠다”가 기준입니다.

## 추천 구조 예시

```text
src/
  main.rs                     # 최소 진입점: tracing + run_server 호출
  app/
    mod.rs
    state.rs                  # AppState
    runtime.rs                # run_server / run_blocking_server
  config/
    mod.rs                    # AppConfig + env helpers
  domain/
    mod.rs
    job.rs                    # Job/JobId/JobStore/JobStatus
  engine/
    mod.rs
    client.rs                 # EngineClient trait + EngineError/Result
    http_client.rs            # call_engine_http + endpoint parse
    manager.rs                # (기존 engine_manager.rs 연계)
  transport/
    mod.rs
    http.rs                   # route_request + response builders
  workers/
    mod.rs                    # (기존 workers.rs 연계)
```

## 단계적 이전 순서 (저위험)

1. **순수 타입 이동**: `Job*`, `Engine*`, `AppConfig`를 먼저 이동
2. **엔진 HTTP 클라이언트 이동**: endpoint parse/timeout/retry를 `engine/http_client`로 분리
3. **라우팅 이동**: `route_request`/응답 빌더를 transport로 이동 (핸들러는 얇게)
4. **런타임 이동**: `run_server`/`run_blocking_server`/`handle_connection` 이동
5. **main.rs 슬림화**: 진입점만 남기기

각 단계는 `cargo test`, `cargo clippy`를 통과한 뒤 다음 단계로 진행합니다.

## 파일 폭증 방지 규칙

- 파일 생성 기준: 책임이 2개 이상이거나 파일 길이가 약 300 LOC를 넘기 시작할 때
- 핸들러에서 비즈니스 로직 금지: 파싱 → 서비스 호출 → 응답 매핑만 유지
- 공개 범위 최소화: `pub`보다 `pub(crate)` 우선
- import churn 완화: `mod.rs`에 re-export를 모아 단계적 이전 비용 감소

## 당장 적용 추천 (첫 분리 타깃)

- 1순위: `domain::job` 분리
- 2순위: `engine::http_client` 분리

이 두 영역은 API 개수와 무관하게 장기적으로 계속 커지는 축이므로, 조기 분리 효과가 가장 큽니다.
