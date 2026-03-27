# RecordRoute

RecordRoute는 오디오 파일을 **회의록 작업 흐름에 맞춰 순차적으로 처리**할 수 있게 만든 도구입니다.

- 1단계: 오디오 채널 분리 + 통합 모노 파일 생성
- 2단계: 텍스트 전사(STT)
- 3단계: 요약 생성

사용자는 웹 UI 또는 HTTP API로 같은 흐름을 실행할 수 있고, 이미 처리한 입력은 재사용되어 반복 작업을 줄일 수 있습니다.

---

## 지금 바로 제공되는 사용자 기능 (UX 관점)

### 1) 오디오를 넣으면 결과 파일 세트가 생성됨
- 입력 오디오를 넣으면 채널별 WAV와 `mono_mix.wav`가 생성됩니다.
- 동일 입력을 다시 요청하면 기존 완료 결과를 재사용합니다.

### 2) 기존 작업(Job) 기반으로 STT를 실행함
- STT는 기존 변환이 끝난 Job을 대상으로 실행됩니다.
- `wav`, `mp3`, `flac`, `ogg` 파일을 대상으로 `stt/*.txt`가 생성됩니다.
- 특정 파일만 선택하거나(`audio_files`), `mono_mix.wav`만 대상으로 실행할 수 있습니다.

### 3) STT 결과 기반으로 요약을 생성함
- 요약은 해당 Job의 `stt/*.txt`를 모아 생성됩니다.
- 이미 요약 파일이 있으면 기본적으로 재사용하고, 원할 때 재생성할 수 있습니다.

### 4) 서버 모드에서 비동기 시작 UX 제공
- 서버 모드에서는 작업 요청 후 즉시 accepted 응답을 받고, 상태/결과 조회 엔드포인트로 진행 상태를 확인할 수 있습니다.

---

## 빠른 시작

## 사전 준비

### 공통
- Rust/Cargo 설치
- Git

`setup` 스크립트는 fresh clone에서도 `modules/` 아래 bundled source submodule을 자동으로 초기화합니다. `setup` 중 submodule 초기화가 실패하면 Git 설치 상태 또는 저장소 checkout 상태를 먼저 확인하세요.

### Linux/macOS
```bash
./setup.sh
```

### Windows
```bat
setup.bat
```

`setup` 스크립트는 `package/`에 실행 가능한 런타임 번들을 조립하며(immutable 교체 + 사용자 데이터 보존), 다음을 처리합니다.

- 필요한 submodule 초기화
- `frontend/package.json`이 있을 때 프론트 의존성 설치
- FFmpeg/Whisper/Llama 툴체인 빌드
- Rust release 빌드(런처/서버/CLI)
- package 조립(`RecordRoute(.exe)`, `RecordRouteServer(.exe)`, `.build`, `models`, 런타임 마커)
- package 첫 생성 시 `.env` 시드 복사(`.env` 우선, 없으면 `.env.example`)
- 런타임 모델 준비

---

## 실행 방법

### Linux/macOS
```bash
./run.sh
```

### Windows
```bat
run.bat
```

공식 사용자 실행 경로는 `package/RecordRoute(.exe)`입니다. `run.sh`/`run.bat`는 해당 런처를 호출하는 보조 스크립트입니다.

- 런처(`RecordRoute`)는 기존 서버를 `/server/ping`으로 확인 후 재사용하거나, 없으면 `RecordRouteServer`를 spawn합니다.
- 런처가 spawn한 서버 로그는 `package/logs/server.log`에 append됩니다.
- 서버는 기본적으로 `127.0.0.1:38080`에 바인딩되며 웹 UI는 [http://127.0.0.1:38080/](http://127.0.0.1:38080/)에서 접근합니다.

---

## 서버 API 사용 예시

## 1) 헬스 체크

```bash
curl -X POST http://127.0.0.1:38080/server/ping \
  -H 'Content-Type: application/json' \
  -d '{"code":"100","message":"hello"}'
```

## 2) 오디오 처리 시작

```bash
curl -X POST http://127.0.0.1:38080/jobs \
  -H 'Content-Type: application/json' \
  -d '{"input_path":"/absolute/path/to/input.wav"}'
```

## 3) 작업 목록/단건 조회

```bash
curl http://127.0.0.1:38080/jobs
curl http://127.0.0.1:38080/jobs/<job_id>
```

## 4) STT 실행/조회

```bash
# 전체 지원 오디오 파일 대상
curl -X POST http://127.0.0.1:38080/jobs/<job_id>/stt \
  -H 'Content-Type: application/json' \
  -d '{}'

# mono_mix.wav만 대상
curl -X POST http://127.0.0.1:38080/jobs/<job_id>/stt \
  -H 'Content-Type: application/json' \
  -d '{"mono_mix_only":true}'

# 상태 조회
curl http://127.0.0.1:38080/jobs/<job_id>/stt
```

## 5) 요약 실행/조회

```bash
# 기본: 기존 요약 있으면 재사용
curl -X POST http://127.0.0.1:38080/jobs/<job_id>/summary \
  -H 'Content-Type: application/json' \
  -d '{}'

# 강제 재생성
curl -X POST http://127.0.0.1:38080/jobs/<job_id>/summary \
  -H 'Content-Type: application/json' \
  -d '{"force_regenerate":true}'

# 상태 조회
curl http://127.0.0.1:38080/jobs/<job_id>/summary
```

## 6) 산출물 파일 조회/다운로드

```bash
curl http://127.0.0.1:38080/jobs/<job_id>/files
curl http://127.0.0.1:38080/jobs/<job_id>/files/stt/mono_mix.txt
```

---

## 기본 설정(.env)

패키지 실행 시 기준 `.env`는 `package/.env`입니다(최초 setup에서 자동 시드). 개발 모드에서는 repo 루트 `.env` fallback이 유지됩니다.

- `RECORDROUTE_WHISPER_MODEL`
  - Whisper 모델 경로(또는 shorthand)
- `RECORDROUTE_LLAMA_MODEL`
  - 로컬 GGUF 경로 또는 Hugging Face repo
- `HF_TOKEN`
  - gated/private 모델 다운로드 시 사용

예시:

```env
RECORDROUTE_WHISPER_MODEL=models/whisper/ggml-base.bin
RECORDROUTE_LLAMA_MODEL=ggml-org/gemma-3-4b-it-GGUF
HF_TOKEN=
```

---

## 결과물 위치

패키지 실행 기준 결과는 `package/db/<job_id>/` 아래에 쌓입니다(개발 모드 fallback은 repo `db/`).

- `mono_mix.wav`
- `channel_01.wav`, `channel_02.wav`, ...
- `stt/*.txt`
- `summary/*.md`

전체 Job 메타데이터와 상태는 `db/index.json`에 저장됩니다.

---

## 트러블슈팅

### 1) "환경 준비가 필요합니다" 또는 "toolchain not found"류 에러
원인:
- 로컬 빌드 산출물(`.build`)이 없거나 깨짐
- 모델 캐시가 없거나 준비 중 실패함

해결:
- `setup`을 다시 실행한 뒤 `run`으로 서버를 다시 시작

### 2) STT/요약 메뉴에서 선택할 항목이 없음
원인:
- 먼저 ffmpeg 단계가 완료된 Job이 없거나,
- STT 대상 오디오 파일 또는 요약 대상 transcript가 없음

해결:
1. 웹 콘솔 또는 `POST /jobs`로 오디오 처리를 먼저 시작
2. `GET /jobs/<job_id>`로 상태가 `completed`인지 확인
3. 필요 시 `GET /jobs/<job_id>/files`로 실제 파일 존재 확인

### 3) STT 요청이 400으로 실패 (`audio_files cannot be combined with mono_mix_only`)
원인:
- STT 요청에서 `audio_files`와 `mono_mix_only=true`를 동시에 보냄

해결:
- 둘 중 하나만 사용

### 4) 서버 응답이 안 옴
점검:
- 서버 프로세스가 떠 있는지 확인
- 포트 충돌 확인(`127.0.0.1:38080`)
- 요청 body가 JSON 형식인지 확인 (잘못된 body는 `invalid request body`)

### 5) 요약 품질 또는 속도가 기대와 다름
점검:
- `RECORDROUTE_LLAMA_MODEL`이 의도한 모델인지 확인
- 모델 다운로드 또는 캐시가 정상인지 확인
- 모델이나 설정을 바꾼 뒤에는 `setup`을 다시 실행

---

## 권장 사용 순서

1. `setup`으로 환경 준비
2. `run`으로 서버 실행
3. 웹 콘솔 또는 API로 오디오 처리 시작
4. STT 실행
5. 요약 실행
6. `db/<job_id>` 산출물 확인

이 순서를 따르면 가장 적은 시행착오로 전체 워크플로를 경험할 수 있습니다.

---

## 개발자 참고

일반 사용자 흐름은 `setup`과 `run` 두 파일만 사용하면 됩니다. 아래 내용은 개발 또는 디버깅 용도입니다.

```bash
cargo run --manifest-path rust/Cargo.toml --release -- <mode>
```

대표 모드:
- `ffmpeg <input>`
- `stt`
- `summary`
- `embed-summaries`
- `search-summaries <query>`
- `prepare-models`
- `prepare-llama-model`
- `server`

테스트:

```bash
cargo test --manifest-path rust/Cargo.toml
```

관련 문서:
- 아키텍처: `docs/architecture.md`
- OpenAPI: `docs/openapi.yaml`
- API 설계 TODO: `docs/API_TODO.md`

