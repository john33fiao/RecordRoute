# Rust/C++ 기반 백엔드 재작성 실행 계획 (보강안)

> 아키텍처 기준선: `docs/architecture.md`
> 보조 정책(배포/자산): `docs/deployment-asset-policy.md`

이 문서는 Rust 오케스트레이터 + C++ AI 엔진(`llama.cpp`, `whisper.cpp`) 분리 구조를 **운영 가능한 단일 방향성**으로 구체화한 실행 계획입니다. 핵심은 FFI를 피하고 내부 HTTP 계약으로 결합하되, 실제 운영에서 흔들리는 지점을 선제적으로 막는 것입니다.

## 1) 고정 아키텍처 원칙

- Rust가 외부 트래픽을 받는 단일 진입점이며, 작업 오케스트레이션/상태관리/배압을 담당한다.
- C++ 엔진은 독립 프로세스로 실행하고 Rust와 `127.0.0.1` HTTP로만 통신한다.
- `llama.cpp`는 **요약(text)과 임베딩(embed)을 서로 다른 프로세스**로 분리한다.
- Swagger는 API 서버와 **별도 프로세스(별도 디플로이 단위)**로 분리한다.
- Docker 확장은 후속 단계로 두되, 로컬 프로세스 레벨에서 먼저 안정성을 완성한다.

## 2) 프로세스/포트 토폴로지 (고정안)

- `:18000` — Rust API 오케스트레이터 (외부 공개)
- `:14000` — Swagger UI 서버 (외부 공개, 별도 프로세스)
- `127.0.0.1:18101` — `llama-text` (요약/생성)
- `127.0.0.1:18102` — `llama-embed` (임베딩 전용)
- `127.0.0.1:18103` — `whisper-server` (STT)

> 모든 내부 엔진은 `--host 127.0.0.1`를 강제한다.

## 3) 디렉터리/자산 표준

```text
/
├── Cargo.toml
├── /src
├── /vendor
│   ├── /llama.cpp            # 소스만 보관(바이너리 커밋 금지)
│   └── /whisper.cpp          # 소스만 보관(바이너리 커밋 금지)
├── /build or /target         # 빌드 산출물(gitignore)
└── /models                   # 모델 원본(gitignore)
    ├── /text
    ├── /embed
    └── /stt
```

운영 규칙:
- `/vendor/*`에는 빌드 결과물을 넣지 않는다.
- `/models/**`는 gitignore 처리하고, 대신 `manifest.yml/json`(파일명/체크섬/버전)만 버전관리한다.

## 4) 엔진 역할 분리 (필수)

### 4.1 `llama-text` (요약/교정)
- 모델 경로: `/models/text/*.gguf`
- 기능: chat/completions 계열
- Rust 라우팅: `summarize_queue`를 통해 전달

### 4.2 `llama-embed` (임베딩)
- 모델 경로: `/models/embed/*.gguf`
- 기능: `/v1/embeddings`
- 필수: 임베딩 모델 구동 시 pooling 정책 명시(`none` 금지, 예: `mean`/`cls`)
- 런치 플래그/버전은 pinning하여 재현성 확보

### 4.3 `whisper-server` (STT)
- 모델 경로: `/models/stt/*`
- 기능: HTTP 업로드 기반 STT 엔드포인트
- Rust 라우팅: `stt_queue`를 통해 전달

## 5) 큐/동시성 설계 (단일 큐 금지)

Head-of-Line Blocking 방지를 위해 큐를 엔진별로 분리한다.

- `stt_queue` (bounded)
- `summarize_queue` (bounded)
- `embed_queue` (bounded)

각 큐는 별도 워커를 가지며, 워커 내부에서 엔진별 세마포어로 동시성을 제한한다.

- 예: `stt_concurrency=1~N`, `summarize_concurrency=1~N`, `embed_concurrency=1~N`
- 큐 포화와 엔진 포화를 구분해 `429`를 반환하고 메트릭 라벨을 분리한다.

## 6) 타임아웃 정책 (운영 파라미터화)

고정 근사식 대신 설정 기반 처리율 추정치로 계산한다.

### 6.1 설정 파라미터
- STT: `stt_rtf` (Real-time factor)
- LLM: `summary_tps` (tokens/sec)
- Embedding: `embed_tps` 또는 `chars_per_sec`
- 공통: `min_timeout_sec`, `max_timeout_sec`, `safety_buffer_sec`

### 6.2 계산식
- STT job timeout = `clamp(audio_sec * stt_rtf + buffer, min, max)`
- 요약 job timeout = `clamp(predicted_tokens / summary_tps + buffer, min, max)`
- 임베딩 timeout = `clamp(input_size / embed_rate + buffer, min, max)`

### 6.3 타임아웃 계층 분리
- HTTP 레벨 타임아웃(연결/응답)
- Job 레벨 타임아웃(총 예산)

둘을 분리해 장애 원인을 명확히 관측한다.

## 7) 오디오 전처리 위치 (단일 선택 고정)

본 계획은 보안/예측가능성을 위해 **Rust 전처리 고정**을 기본값으로 채택한다.

- Rust가 `symphonia` 크레이트로 오디오 디코딩/리샘플링 후 16kHz, 16-bit mono WAV 규격으로 정규화
- 길이 추출도 Rust 내부 메타데이터/샘플 기반으로 계산(외부 `ffprobe` 의존 제거)
- `whisper-server`는 추론만 수행(변환 책임 제외)

대안(미채택): 외부 `ffmpeg`/`ffprobe` 실행 또는 `whisper-server --convert` 사용. 외부 프로세스 의존성과 공격면을 키우므로 기본안에서 제외한다.

## 8) 프로세스 슈퍼비전 (Drop만으로 불충분)

Rust 오케스트레이터는 단순 launcher가 아니라 supervisor로 동작해야 한다.

### 8.1 시작 시퀀스
1. 포트 점유/경로/실행파일 존재 확인
2. child spawn (`llama-text`, `llama-embed`, `whisper-server`)
3. 각 `/health`(또는 대응 엔드포인트) 통과 전까지 `readyz=false`

### 8.2 실행 중
- child 종료 감지
- 제한된 backoff로 재시작
- 재시작 횟수 상한 초과 시 `degraded` 상태 전이

### 8.3 종료 시
- SIGTERM/SIGINT 핸들링
- child에 정상 종료 시그널 전달
- 유예시간 후 미종료 child는 강제 종료(`kill -9`는 최후수단)

## 9) API/문서 배포 분리

### 9.1 API 서버 (Rust, 18000)
- `POST /jobs` → `202` + `job_id`
- `GET /jobs/{id}` → `queued|running|completed|failed|timeout|canceled`
- `GET /healthz` / `GET /readyz`

### 9.2 Swagger 서버 (14000, 별도 프로세스)
- Swagger UI만 담당
- 스펙 원본: `http://localhost:18000/openapi.json`
- 문서 서버 장애가 API 가용성에 영향을 주지 않도록 분리 운영

## 10) 단계별 실행 계획

### Phase 1 — 엔진 검증/고정
- `llama.cpp`, `whisper.cpp` 버전 pin
- `llama-text`/`llama-embed`/`whisper-server` 단독 실행 검증
- 임베딩 endpoint와 pooling 정책 동작 검증

### Phase 2 — Rust 코어 + 엔진별 큐
- `axum`, `tokio`, `reqwest`, `utoipa` 스캐폴딩
- 엔진별 bounded queue + semaphore 구현
- `POST /jobs`, `GET /jobs/{id}` 구현

### Phase 3 — 전처리/타임아웃/오류계약
- Rust `symphonia` 기반 전처리/길이 추출 파이프라인 구현
- 처리율 기반 timeout 계산 도입
- HTTP vs Job timeout 분리 및 에러코드 체계화

### Phase 4 — 슈퍼비전/복구
- child lifecycle 감시 루프
- 재시작 backoff/상한
- graceful shutdown + 강제 종료 fallback

### Phase 5 — Swagger 분리 배포/관측성
- Swagger 별도 프로세스(`:14000`) 운영
- 큐 포화/엔진 포화/timeout/재시작 메트릭 노출
- 운영 점검 시나리오(부하/장애/복구) 문서화

## 11) 즉시 실행 체크리스트

- [ ] `/models/{text,embed,stt}` 생성 + manifest 관리
- [ ] `llama-text`(18101), `llama-embed`(18102), `whisper-server`(18103) 실행 검증
- [ ] 임베딩 모델 pooling 정책/런치 플래그 고정
- [ ] Rust 엔진별 큐(`stt/summarize/embed`) + semaphore 구현
- [ ] Rust `symphonia` 전처리 및 길이 추출 적용
- [ ] 처리율 기반 timeout 파라미터 파일 작성
- [ ] supervisor(헬스체크/재시작/종료) 구현
- [ ] Swagger 별도 프로세스(14000) 배포 구성
