# Rust/C++ 백엔드 재작성 TODO 체크리스트

## 1) 고정 아키텍처 원칙
- [ ] Rust를 외부 트래픽 단일 진입점으로 고정하고 오케스트레이션/상태관리/배압 책임을 명시한다.
- [ ] C++ 엔진(`llama.cpp`, `whisper.cpp`)을 독립 프로세스로 실행하고 Rust와 `127.0.0.1` HTTP 계약으로만 통신한다.
- [ ] `llama.cpp`를 `llama-text`(요약)와 `llama-embed`(임베딩) 프로세스로 분리한다.
- [ ] Swagger를 API 서버와 별도 프로세스(별도 배포 단위)로 분리한다.
- [ ] Docker 확장은 후속 단계로 두고 로컬 프로세스 레벨 안정성을 먼저 확보한다.

## 2) 프로세스/포트 토폴로지
- [ ] `:18000` Rust API 오케스트레이터 포트를 고정한다.
- [ ] `:14000` Swagger UI 서버 포트를 고정한다.
- [ ] `127.0.0.1:18101` `llama-text` 포트를 고정한다.
- [ ] `127.0.0.1:18102` `llama-embed` 포트를 고정한다.
- [ ] `127.0.0.1:18103` `whisper-server` 포트를 고정한다.
- [ ] 모든 내부 엔진에 `--host 127.0.0.1` 강제를 적용한다.

## 3) 디렉터리/자산 표준
- [ ] 루트 구조(`Cargo.toml`, `/src`, `/vendor`, `/models`)를 표준안대로 구성한다.
- [ ] `/vendor/llama.cpp`, `/vendor/whisper.cpp`에는 소스만 보관한다(바이너리 커밋 금지).
- [ ] 빌드 산출물(`/build`, `/target`)을 `.gitignore`로 관리한다.
- [ ] `/models/{text,embed,stt}` 디렉터리를 만들고 모델 원본을 `.gitignore`로 관리한다.
- [ ] 모델 원본 대신 `manifest.yml` 또는 `manifest.json`(파일명/체크섬/버전)만 버전관리한다.

## 4) 엔진 역할 분리
### 4.1 llama-text (요약/교정)
- [ ] 모델 경로(`/models/text/*.gguf`)를 적용한다.
- [ ] chat/completions 계열 기능만 담당하도록 분리한다.
- [ ] Rust에서 `summarize_queue` 라우팅을 적용한다.

### 4.2 llama-embed (임베딩)
- [ ] 모델 경로(`/models/embed/*.gguf`)를 적용한다.
- [ ] `/v1/embeddings` 전용 엔드포인트로 운영한다.
- [ ] pooling 정책을 명시한다(`none` 금지, 예: `mean`/`cls`).
- [ ] 런치 플래그/버전을 pinning해 재현성을 확보한다.

### 4.3 whisper-server (STT)
- [ ] 모델 경로(`/models/stt/*`)를 적용한다.
- [ ] HTTP 업로드 기반 STT 엔드포인트를 고정한다.
- [ ] Rust에서 `stt_queue` 라우팅을 적용한다.

## 5) 큐/동시성 설계(단일 큐 금지)
- [ ] `stt_queue`(bounded)를 구현한다.
- [ ] `summarize_queue`(bounded)를 구현한다.
- [ ] `embed_queue`(bounded)를 구현한다.
- [ ] 각 큐에 별도 워커를 두고 엔진별 semaphore 동시성 제한을 적용한다.
- [ ] `stt_concurrency`, `summarize_concurrency`, `embed_concurrency` 운영값을 정의한다.
- [ ] 큐 포화와 엔진 포화를 구분해 `429`와 메트릭 라벨을 분리한다.

## 6) 타임아웃 정책
### 6.1 설정 파라미터
- [ ] STT 처리율(`stt_rtf`) 파라미터를 정의한다.
- [ ] 요약 처리율(`summary_tps`) 파라미터를 정의한다.
- [ ] 임베딩 처리율(`embed_tps` 또는 `chars_per_sec`) 파라미터를 정의한다.
- [ ] 공통 파라미터(`min_timeout_sec`, `max_timeout_sec`, `safety_buffer_sec`)를 정의한다.

### 6.2 계산식
- [ ] STT timeout 계산식(`clamp(audio_sec * stt_rtf + buffer, min, max)`)을 적용한다.
- [ ] 요약 timeout 계산식(`clamp(predicted_tokens / summary_tps + buffer, min, max)`)을 적용한다.
- [ ] 임베딩 timeout 계산식(`clamp(input_size / embed_rate + buffer, min, max)`)을 적용한다.

### 6.3 타임아웃 계층 분리
- [ ] HTTP 레벨 타임아웃(연결/응답)을 분리해 적용한다.
- [ ] Job 레벨 타임아웃(총 예산)을 분리해 적용한다.

## 7) 오디오 전처리(고정안)
- [ ] Rust `symphonia` 기반 디코딩/리샘플링으로 16kHz, 16-bit mono WAV 정규화를 적용한다.
- [ ] 오디오 길이 추출을 Rust 내부 메타데이터/샘플 기반으로 구현한다.
- [ ] `whisper-server`는 추론만 담당하도록 역할을 제한한다.
- [ ] 외부 `ffmpeg`/`ffprobe` 및 `whisper-server --convert` 의존을 기본 경로에서 제외한다.

## 8) 프로세스 슈퍼비전
### 8.1 시작 시퀀스
- [ ] 포트 점유/경로/실행파일 존재를 사전 점검한다.
- [ ] `llama-text`, `llama-embed`, `whisper-server` child spawn을 구현한다.
- [ ] 각 엔진 health 통과 전까지 `readyz=false`를 유지한다.

### 8.2 실행 중
- [ ] child 종료 감지 루프를 구현한다.
- [ ] 제한된 backoff 재시작을 구현한다.
- [ ] 재시작 상한 초과 시 `degraded` 상태 전이를 구현한다.

### 8.3 종료 시
- [ ] SIGTERM/SIGINT graceful shutdown을 구현한다.
- [ ] child 정상 종료 신호 전파를 구현한다.
- [ ] 유예시간 이후 강제 종료 fallback을 구현한다(`kill -9` 최후수단).

## 9) API/문서 배포 분리
### 9.1 API 서버(18000)
- [ ] `POST /jobs` → `202` + `job_id` 계약을 구현한다.
- [ ] `GET /jobs/{id}` 상태(`queued|running|completed|failed|timeout|canceled`) 계약을 구현한다.
- [ ] `GET /healthz`, `GET /readyz`를 구현한다.

### 9.2 Swagger 서버(14000)
- [ ] Swagger UI 전용 서버를 별도 프로세스로 분리한다.
- [ ] 스펙 원본을 `http://localhost:18000/openapi.json`으로 연결한다.
- [ ] 문서 서버 장애가 API 가용성에 영향을 주지 않도록 분리 운영한다.

## 10) 단계별 실행(Phase)
### Phase 1 — 엔진 검증/고정
- [ ] `llama.cpp`, `whisper.cpp` 버전을 pin한다.
- [ ] `llama-text`, `llama-embed`, `whisper-server` 단독 실행 검증을 완료한다.
- [ ] 임베딩 endpoint/pooling 정책 동작을 검증한다.

### Phase 2 — Rust 코어 + 엔진별 큐
- [ ] `axum`, `tokio`, `reqwest`, `utoipa` 스캐폴딩을 구성한다.
- [ ] 엔진별 bounded queue + semaphore를 구현한다.
- [ ] `POST /jobs`, `GET /jobs/{id}`를 구현한다.

### Phase 3 — 전처리/타임아웃/오류계약
- [ ] Rust `symphonia` 전처리/길이 추출 파이프라인을 구현한다.
- [ ] 처리율 기반 timeout 계산을 도입한다.
- [ ] HTTP vs Job timeout 분리 및 에러코드 체계를 정리한다.

### Phase 4 — 슈퍼비전/복구
- [ ] child lifecycle 감시 루프를 구현한다.
- [ ] 재시작 backoff/상한 정책을 구현한다.
- [ ] graceful shutdown + 강제 종료 fallback을 구현한다.

### Phase 5 — Swagger 분리 배포/관측성
- [ ] Swagger 별도 프로세스(`:14000`) 운영 구성을 완료한다.
- [ ] 큐 포화/엔진 포화/timeout/재시작 메트릭을 노출한다.
- [ ] 부하/장애/복구 운영 점검 시나리오를 문서화한다.

## 11) 즉시 실행 체크리스트(원문 반영)
- [ ] `/models/{text,embed,stt}` 생성 + manifest 관리
- [ ] `llama-text`(18101), `llama-embed`(18102), `whisper-server`(18103) 실행 검증
- [ ] 임베딩 모델 pooling 정책/런치 플래그 고정
- [ ] Rust 엔진별 큐(`stt/summarize/embed`) + semaphore 구현
- [ ] Rust `symphonia` 전처리 및 길이 추출 적용
- [ ] 처리율 기반 timeout 파라미터 파일 작성
- [ ] supervisor(헬스체크/재시작/종료) 구현
- [ ] Swagger 별도 프로세스(14000) 배포 구성
