# API / Toolchain Audit (2026-03-28 코드 기준)

이 문서는 RecordRoute가 외부 도구와 통신하는 현재 방식을 점검한 결과를 정리한다.
여기서 "외부 도구"는 `ffmpeg`, `ffprobe`, `whisper-cli`, `llama-cli`, `llama-embedding`을 뜻한다.

핵심 질문:

- Rust가 각 도구에 무엇을 전달하는가
- 성공과 실패를 무엇으로 판정하는가
- CLI와 HTTP API가 같은 경계를 공유하는가
- 현재 구현에서 어떤 취약 지점이 남아 있는가

## 1. 결론 요약

현재 RecordRoute의 런타임 파이프라인은 여전히 CLI 래핑 구조다.

- FFmpeg: `ffprobe` JSON 파싱 + `ffmpeg` 실행
- Whisper: `whisper-cli` 실행 + transcript 파일 존재 확인
- Llama summary: `llama-cli` 실행 + `stdout` 후처리
- Llama embedding: `llama-embedding` 실행 + JSON 벡터 파싱

CLI와 HTTP API는 모두 `app/*` 도메인 로직을 공유하며, 서버 핸들러가 외부 도구를 직접 호출하지 않는다.

예외:

- whisper 모델 다운로드는 더 이상 외부 스크립트에 의존하지 않고 Rust가 직접 파일을 복사하거나 HTTP 다운로드한다.
- llama Hugging Face 모델 확보는 `llama-cli`를 이용한 파일 캐시 관찰 방식이다.

## 2. 공통 호출 경계

공통 진입점:

- CLI
  - `rust/src/main.rs`
  - `rust/src/app/cli.rs`
- HTTP
  - `rust/src/server.rs`
  - `rust/src/server/app_api.rs`

공통 원칙:

- 서버와 CLI는 모두 `app/*`를 통해서만 stage submit / execute를 호출한다.
- 외부 도구 호출 세부사항은 `ffmpeg.rs`, `whisper.rs`, `llama.rs`에 모여 있다.
- 장기 작업은 서버에서 `spawn_blocking`으로 백그라운드 처리된다.

## 3. FFmpeg / ffprobe

### 3.1 도구 탐색

- 탐색 경로: `.build/ffmpeg/<os>-<arch>/install/bin`
- 필수 실행 파일:
  - `ffprobe`
  - `ffmpeg`

### 3.2 ffprobe 요청

Rust는 먼저 `ffprobe`로 입력 메타데이터를 읽는다.

호출 형태:

```text
ffprobe
  -v error
  -select_streams a:0
  -show_entries stream=channels,channel_layout
  -of json
  <input>
```

응답 처리:

- `stdout`를 JSON으로 파싱
- `streams[0].channels`
- `streams[0].channel_layout`

성공 판정:

- 종료 코드 성공
- JSON 파싱 성공
- 채널 수가 1 이상

### 3.3 ffmpeg 요청

`ffprobe` 결과를 바탕으로 Rust가 mono 입력이면 direct mono output을, multi-channel 입력이면
`filter_complex`를 구성한 뒤 `ffmpeg`를 한 번 실행한다.

출력 목표:

- 1채널 입력: merged mono wav (`mono_mix.wav`)만 생성
- 2채널 이상 입력: 채널별 mono wav + merged mono wav

호출 개요:

```text
ffmpeg
  -hide_banner
  -loglevel error
  -y
  -i <input>
  -vn -sn -dn
  # mono input
  -map 0:a:0 ... mono_mix.wav

  # multi-channel input
  -filter_complex <generated_filter>
  -map [split01] ... channel_01.wav
  -map [split02] ... channel_02.wav
  ...
  -map [mix] ... mono_mix.wav
```

성공 판정:

- 종료 코드 성공

실패 처리:

- `stderr` 우선으로 오류 메시지 구성
- 부분 산출물은 cleanup

관찰 포인트:

- `ffprobe`는 구조화 응답 파싱을 쓰지만 `ffmpeg`는 종료 코드 중심이다.
- `ffmpeg` 성공 후 산출 파일 존재를 개별적으로 다시 검증하지는 않는다.

## 4. whisper.cpp 런타임과 모델 준비

### 4.1 도구 탐색

- 탐색 경로: `.build/whisper/<os>-<arch>/bin/whisper-cli`
- 모델 기본 경로: `models/whisper/ggml-base.bin`
- 모델 환경변수: `RECORDROUTE_WHISPER_MODEL`

### 4.2 STT 요청

Rust는 `whisper-cli`에 다음 인자를 넘긴다.

```text
whisper-cli
  -m <model_path>
  -f <input_audio>
  -l auto
  -otxt
  -np
  -of <output_prefix>
```

출력 규약:

- `stt/<audio_stem>.txt`
- Rust는 prefix만 넘기고 `.txt` 파일 생성을 기대한다

성공 판정:

- 종료 코드 성공
- 기대한 transcript 파일 존재

추가 처리:

- 생성된 transcript에서 연속 중복 라인을 제거한다

### 4.3 fallback / 복구

백엔드 실패 추정 시 CPU fallback:

- Windows: CUDA/NVIDIA 관련 문자열
- macOS: Metal 관련 문자열 또는 `failed to initialize whisper context`

CPU fallback 시:

- `-ng`
- 필요 환경변수 비활성화

추가 복구:

- 관리 대상 whisper 모델 캐시가 손상된 것으로 보이면
- 기존 파일을 삭제하고 모델 확보를 다시 시도한 뒤 1회 재실행

### 4.4 모델 준비 방식

현재 whisper 모델 준비는 외부 다운로드 스크립트가 아니라 Rust 내부 로직이다.

순서:

1. `RECORDROUTE_WHISPER_MODEL_SOURCE_DIR`가 있으면 `ggml-<model>.bin` 복사 시도
2. 없으면 `RECORDROUTE_WHISPER_MODEL_URL_TEMPLATE` 또는 기본 Hugging Face URL로 다운로드
3. `toolchain.model_path`에 파일 저장

즉 whisper 런타임은 CLI 래핑이지만, 모델 확보는 in-process 파일 복사/HTTP 다운로드다.

## 5. llama summary

### 5.1 도구 탐색

- 탐색 경로: `.build/llama/<os>-<arch>/bin/llama-cli`
- summary 모델 환경변수: `RECORDROUTE_LLAMA_MODEL`
- 기본 모델 repo: `ggml-org/gemma-3-4b-it-GGUF`

### 5.2 프롬프트 생성

Rust는 `stt/*.txt`를 읽어 summary 프롬프트 파일을 만든다.

현재 프롬프트 정책:

- 한국어 Markdown만 출력
- 첫 줄은 `## 요약`
- 중복 표현 제거
- transcript 파일명 나열 금지
- 후속 조치가 있으면 마지막에 bullet list

프롬프트 파일은 summary 실행 전 생성되고, 실행 후 삭제된다.

### 5.3 llama-cli 요청

호출 형태:

```text
llama-cli
  --single-turn
  --simple-io
  --no-display-prompt
  --log-disable
  -n 1024
  -f <prompt_file>
  (-m <local_model> | -hf <repo>)
```

Hugging Face repo 사용 시:

- `LLAMA_CACHE=<download_cache_dir>` 설정

### 5.4 응답 처리

요약 생성은 `stdout` 후처리에 의존한다.

처리 개요:

1. echo된 prompt 제거 시도
2. `(truncated)` 뒤쪽만 남기는 보정
3. summary 시작 마커 탐색
4. tail marker 제거
5. 제목 정규화
6. 결과를 `summary/result.md`에 저장

성공 판정:

- 종료 코드 성공
- 추출된 summary 텍스트가 비어 있지 않음

즉 summary 생성은 CLI 출력 포맷 변화에 상대적으로 민감한 편이다.

## 6. llama embedding

### 6.1 도구 탐색

- 탐색 경로: `.build/llama/<os>-<arch>/bin/llama-embedding`
- embedding 모델 환경변수: `RECORDROUTE_LLAMA_EMBEDDING_MODEL`
- 기본 모델 repo: `Qwen/Qwen3-Embedding-4B-GGUF`

### 6.2 llama-embedding 요청

호출 형태:

```text
llama-embedding
  --pooling mean
  --embd-normalize 2
  --embd-output-format array
  -p <summary_or_query_text>
  (-m <local_model> | -hf <repo>)
```

Hugging Face repo 사용 시:

- `LLAMA_CACHE=<download_cache_dir>` 설정

### 6.3 응답 처리

성공 판정:

- 종료 코드 성공
- `stdout`에서 JSON 벡터를 파싱할 수 있음

허용 형식:

- `Vec<f32>`
- `Vec<Vec<f32>>`
- `{ "data": [ { "embedding": [...] } ] }`

파싱 실패 시:

- `"expected JSON embedding data"` 오류

이 경로는 summary 생성보다 출력 구조가 단순해서 휴리스틱 의존도가 낮다.

## 7. llama 모델 다운로드

summary / embedding 모델이 Hugging Face repo 문자열로 설정된 경우, RecordRoute는 `llama-cli`를 이용해 GGUF를 확보한다.

핵심 방식:

1. `LLAMA_CACHE`가 가리키는 다운로드 캐시 디렉터리 준비
2. `llama-cli --no-warmup --no-mmproj -n 0 -hf <repo> -p /exit` 실행
3. 다운로드 캐시 아래 새 `.gguf` 파일 생성 감시
4. 발견 시 최종 캐시 위치 `models/llama/hf/*.gguf`로 이동

즉 다운로드 성공 판정은 구조화된 CLI 응답이 아니라 파일 시스템 관찰 기반이다.

## 8. 비교 요약

| 경계 | 요청 전달 | 성공 판정 | 응답 파싱 |
| --- | --- | --- | --- |
| `ffprobe` | CLI 인자 | 종료 코드 + JSON 파싱 성공 | `stdout` JSON 파싱 |
| `ffmpeg` | CLI 인자 | 종료 코드 | 실패 시 텍스트 오류만 사용 |
| `whisper-cli` | CLI 인자 | 종료 코드 + transcript 파일 존재 | 본문 파싱 없음 |
| whisper 모델 확보 | Rust 파일 복사 또는 HTTP 다운로드 | 모델 파일 저장 성공 | 외부 CLI 없음 |
| `llama-cli` summary | CLI 인자 + `LLAMA_CACHE` | 종료 코드 + summary 텍스트 추출 성공 | `stdout` 휴리스틱 후처리 |
| `llama-embedding` | CLI 인자 + `LLAMA_CACHE` | 종료 코드 + 벡터 JSON 파싱 성공 | `stdout` JSON 파싱 |
| llama 모델 확보 | `llama-cli` 실행 + 캐시 감시 | `.gguf` 발견 또는 최종 캐시 존재 | 파일 시스템 관찰 중심 |

## 9. 현재 리스크와 주의점

1. `ffmpeg` 성공 이후 산출 파일 존재를 개별 검증하지 않는다.
2. `whisper-cli`는 transcript 파일 존재만 보며 내용 품질이나 비어 있는지는 검사하지 않는다.
3. summary 생성은 `llama-cli`의 `stdout` 포맷 변화에 가장 취약하다.
4. llama Hugging Face 다운로드는 파일 캐시 관찰 기반이라 도구 출력만으로 상태를 설명하기 어렵다.
5. embedding task 완료와 index metadata 저장이 완전히 한 번의 업데이트로 묶여 있지는 않다.

## 10. 감사 기준 문서

현재 이 문서를 갱신해야 하는 변화:

- 외부 실행 파일 이름 또는 탐색 경로 변경
- whisper 모델 준비 방식 변경
- summary/embedding 호출 인자 변경
- summary 출력 파일명 변경
- embedding 출력 파싱 형식 변경
