# API Audit

이 문서는 RecordRoute에서 Rust가 `ffmpeg`, `whisper.cpp`, `llama.cpp`와 통신하는 방식을 점검한 결과를 정리한다.

기준 범위:
- 외부 모듈과의 통신 방식
- CLI 인자/환경변수로 요청을 전달하는 방식
- CLI 응답값, 표준출력/표준에러, 생성 파일을 어떻게 판정/파싱하는지
- 동일 로직이 CLI와 HTTP API에서 어떻게 재사용되는지

검토 기준 파일:
- `rust/src/app.rs`
- `rust/src/server.rs`
- `rust/src/ffmpeg.rs`
- `rust/src/whisper.rs`
- `rust/src/llama.rs`

## 1. 요약

현재 세 외부 모듈과 Rust의 통신은 모두 `std::process::Command` 기반의 CLI 래핑 방식이다.
직접 링크나 FFI는 사용하지 않는다.

핵심 특징:
- 요청 전달: 명령행 인자와 일부 환경변수로 전달
- 응답 수신: 종료 코드, `stdout`, `stderr`, 생성 파일 존재 여부로 판단
- 구조화 파싱:
  - `ffprobe`만 JSON을 파싱한다
  - `llama-cli`는 `stdout` 텍스트를 후처리해 요약 본문을 추출한다
  - `ffmpeg`, `whisper-cli`는 성공 시 구조화 응답을 파싱하지 않는다
- 실행 진입점:
  - CLI: `recordroute_rust [ffmpeg <input>|stt|summary|prepare-llama-model|server|<input>]`
  - HTTP API: `server.rs`가 동일한 `app.rs` 도메인 함수를 호출

## 2. 호출 진입점

공통적으로 `main_cli()`와 `server.rs`는 외부 툴과 직접 통신하지 않고 `app.rs`를 통해 호출한다.

| 진입점 | 내부 함수 | 외부 모듈 |
| --- | --- | --- |
| CLI `ffmpeg <input>` 또는 `<input>` | `run_with_repo_root()` -> `submit_ffmpeg_job()` / `execute_ffmpeg_job()` | `ffprobe`, `ffmpeg` |
| CLI `stt` | `run_stt_with_repo_root()` | `whisper-cli` |
| CLI `summary` | `run_summary_with_repo_root()` | `llama-cli` |
| CLI `prepare-llama-model` | `prepare_llama_model_with_repo_root()` | `llama-cli` |
| HTTP `POST /jobs` / `POST /jobs/upload` | `submit_ffmpeg_job()` / `execute_ffmpeg_job()` | `ffprobe`, `ffmpeg` |
| HTTP `POST /jobs/{job_id}/stt` | `submit_stt_job()` / `execute_stt_job()` | `whisper-cli` |
| HTTP `POST /jobs/{job_id}/summary` | `submit_summary_job()` / `execute_summary_job()` | `llama-cli` |
| HTTP `POST /models/whisper/prepare` | `submit_model_preparation()` / `execute_model_preparation()` | whisper 모델 다운로드 스크립트 |
| HTTP `POST /models/llama/prepare` | `submit_model_preparation()` / `execute_model_preparation()` | `llama-cli` |

즉, CLI와 HTTP는 동일한 래퍼를 공유하므로 외부 툴 호출 규약은 한 군데에 모여 있다.

## 3. 공통 통신 패턴

### 3.1 도구 탐색

각 래퍼는 먼저 로컬 빌드 산출물을 찾는다.

- FFmpeg: `.build/ffmpeg/<os>-<arch>/install/bin`
- Whisper: `.build/whisper/<os>-<arch>/bin`
- Llama: `.build/llama/<os>-<arch>/bin`

탐색 실패 시에는 빌드 스크립트 경로를 포함한 오류를 반환한다.

### 3.2 성공/실패 판정 공통점

- 기본적으로 프로세스 종료 코드를 먼저 본다.
- 실패 시 `stderr` 우선, 없으면 `stdout`를 오류 메시지로 사용한다.
- 일부 모듈은 종료 코드 외에 산출 파일 존재 여부를 추가로 검사한다.

### 3.3 환경변수 전달

별도 `env()`를 설정하지 않은 경우 부모 Rust 프로세스 환경이 그대로 상속된다.
따라서 `.env` 또는 실행 환경에 설정된 값은 외부 CLI에도 전달된다.

명시적으로 추가하는 환경변수:
- Whisper CPU fallback on macOS: `GGML_METAL=0`, `GGML_METAL_DEVICES=0`
- Llama Hugging Face download/runtime: `LLAMA_CACHE=<cache_dir>`
- Llama CPU fallback on macOS: `GGML_METAL=0`, `GGML_METAL_DEVICES=0`

암묵적으로 상속되는 값:
- 예: `HF_TOKEN`

## 4. FFmpeg / ffprobe 감사

### 4.1 Rust에서 어디서 호출되는가

- `app.rs`
  - `submit_ffmpeg_job()`
  - `execute_ffmpeg_job()`
- `server.rs`
  - `POST /jobs`
  - `POST /jobs/upload`

실제 외부 호출은 `rust/src/ffmpeg.rs`에 있다.

### 4.2 요청 전달 방식

#### 1) 입력 메타데이터 조회: `ffprobe`

Rust는 먼저 `ffprobe`를 실행해 입력 오디오의 채널 수와 채널 레이아웃을 조회한다.

호출 인자:

```text
ffprobe
  -v error
  -select_streams a:0
  -show_entries stream=channels,channel_layout
  -of json
  <input>
```

### 4.3 응답 파싱 방식

`ffprobe`의 `stdout`는 JSON으로 받아 `serde_json::from_slice()`로 파싱한다.

파싱 대상:
- `streams[0].channels`
- `streams[0].channel_layout`

판정 방식:
- 종료 코드가 실패면 `stderr`를 오류 메시지로 사용
- JSON 파싱 실패 시 즉시 오류
- 오디오 스트림이 없거나 채널 수가 없거나 0이면 오류

즉, `ffprobe`는 세 모듈 중 유일하게 구조화된 CLI 응답을 직접 파싱한다.

#### 2) 오디오 변환: `ffmpeg`

`ffprobe` 결과를 바탕으로 `filter_complex` 문자열을 Rust에서 동적으로 생성한 뒤 `ffmpeg`에 전달한다.

호출 인자 개요:

```text
ffmpeg
  -hide_banner
  -loglevel error
  -y
  -i <input>
  -vn -sn -dn
  -filter_complex <generated_filter>
  -map [split01] ... channel_01.wav
  -map [split02] ... channel_02.wav
  ...
  -map [mix] ... mono_mix.wav
```

`filter_complex` 생성 규칙:
- `[mix]`: 모든 채널을 합친 mono mix
- `[splitNN]`: 각 채널을 개별 mono wav로 분리

출력 산출물:
- `db/<job_id>/mono_mix.wav`
- `db/<job_id>/channel_01.wav`
- `db/<job_id>/channel_02.wav`
- ...

### 4.4 응답 사용 방식

`ffmpeg`는 성공 시 `stdout`/`stderr`를 파싱하지 않는다.

판정 방식:
- 종료 코드 성공이면 변환 성공으로 간주
- 종료 코드 실패면 `stderr`를 오류 메시지로 사용

후속 사용:
- `app.rs`가 planned output 경로를 `JobOutputs`에 기록
- 실패 시 `cleanup_partial_files()`로 부분 생성 파일 삭제

### 4.5 점검 메모

- 장점: `ffprobe`는 JSON 기반이라 비교적 안정적이다.
- 주의점: `ffmpeg` 성공 경로에서는 예상 산출 파일이 실제로 모두 생성되었는지 추가 검증하지 않는다.

## 5. whisper.cpp 감사

### 5.1 Rust에서 어디서 호출되는가

- `app.rs`
  - `execute_stt_job()`
  - `run_stt_with_repo_root()`
  - `ensure_model_prepared(..., ModelKind::Whisper)`
- `server.rs`
  - `POST /jobs/{job_id}/stt`
  - `POST /models/whisper/prepare`

실제 외부 호출은 `rust/src/whisper.rs`에 있다.

### 5.2 모델 결정 및 준비 방식

모델 경로 결정:
- 환경변수 `RECORDROUTE_WHISPER_MODEL`이 있으면 우선 사용
- 없으면 기본값 `models/whisper/ggml-base.bin`

준비 방식:
- 모델 파일이 이미 있으면 바로 사용
- 없으면 다운로드 스크립트를 실행

다운로드 스크립트 호출:

```text
<download_script>
  <model_name>
  <model_dir>
```

현재 코드상 다운로드 스크립트 위치:
- `whisper.cpp/models/download-ggml-model.sh`
- `whisper.cpp/models/download-ggml-model.cmd`

성공 판정:
- 종료 코드 성공
- 그리고 `toolchain.model_path` 파일이 실제로 존재해야 함

실패 시:
- `stderr` 우선, 없으면 `stdout`를 오류 메시지로 사용

### 5.3 STT 요청 전달 방식

Rust는 `whisper-cli`에 다음 인자를 전달한다.

```text
whisper-cli
  -m <model_path>
  -f <input_audio>
  -l auto
  -otxt
  -np
  -of <output_prefix>
```

의미:
- `-m`: 모델 파일 경로
- `-f`: 입력 오디오 파일
- `-l auto`: 언어 자동 감지
- `-otxt`: 텍스트 파일 출력
- `-np`: 진행률 출력 억제
- `-of`: 출력 prefix 지정

출력 파일 규약:
- Rust는 `stt/<stem>.txt`를 최종 산출물로 기대한다
- `whisper-cli`에는 확장자를 제외한 prefix만 넘긴다
- 예: `-of db/<job_id>/stt/channel_01` -> 기대 결과 파일은 `channel_01.txt`

### 5.4 응답 파싱 방식

`whisper-cli` 성공 시 Rust는 텍스트 내용을 직접 파싱하지 않는다.

판정 방식:
- 종료 코드 성공
- 그리고 기대한 `.txt` 파일이 실제로 생성되었는지 검사

실패 시:
- `stderr` 우선, 없으면 `stdout`를 오류 메시지로 사용

즉, whisper 통합은 "CLI 텍스트 응답 파싱"보다 "산출 파일 생성 여부"에 의존한다.

### 5.5 재시도/복구 로직

백엔드 실패 추정 시 CPU fallback:
- Windows: CUDA/NVIDIA 관련 문자열 탐지
- macOS: Metal 관련 문자열 또는 `failed to initialize whisper context`

CPU fallback 시 추가 인자/환경:
- `-ng`
- macOS에서는 `GGML_METAL=0`, `GGML_METAL_DEVICES=0`

추가 복구:
- 오류 메시지에 `failed to initialize whisper context`가 있고
- 관리 대상 모델 캐시(`models/whisper/...`)를 쓰는 경우
- 기존 모델 파일을 삭제하고 다운로드 스크립트를 다시 실행한 뒤 1회 재시도

### 5.6 점검 메모

- 장점: 성공 판정에 파일 존재 확인이 포함되어 있어 종료 코드만 보는 방식보다 안전하다.
- 주의점: 생성된 transcript의 내용 품질이나 비어 있는지 여부는 검사하지 않는다.

## 6. llama.cpp 감사

### 6.1 Rust에서 어디서 호출되는가

- `app.rs`
  - `execute_summary_job()`
  - `run_summary_with_repo_root()`
  - `prepare_llama_model_with_repo_root()`
  - `ensure_model_prepared(..., ModelKind::Llama)`
- `server.rs`
  - `POST /jobs/{job_id}/summary`
  - `POST /models/llama/prepare`

실제 외부 호출은 `rust/src/llama.rs`에 있다.

### 6.2 모델 소스 결정 방식

환경변수:
- `RECORDROUTE_LLAMA_MODEL`

해석 규칙:
- 값이 파일 경로이고 실제 파일이 있으면 `LocalPath`
- 아니면 Hugging Face repo 문자열로 간주
- 환경변수가 없으면 기본 repo `ggml-org/gemma-3-4b-it-GGUF`

Hugging Face repo인 경우:
- 캐시 파일 경로: `models/llama/hf/<cache_key>.gguf`
- 다운로드 임시 캐시: `models/llama/hf/.cache/<cache_key>/`

### 6.3 요약 요청 전달 방식

요약 전 Rust는 transcript 여러 개를 읽어 하나의 prompt 파일로 합친다.

prompt 파일 내용:
- 한국어 회의록 생성 지시문
- transcript 파일별 본문
- 섹션 규칙: `개요`, `핵심 논의`, `결정/합의`, `후속 조치`

이후 `llama-cli`를 호출한다.

공통 인자:

```text
llama-cli
  --single-turn
  --simple-io
  --no-display-prompt
  --log-disable
  -n 1024
  ...
  -f <prompt_file>
```

모델 지정 방식은 두 가지다.

#### 1) 로컬 모델 파일 사용

```text
llama-cli ... -m <model_path> -f <prompt_file>
```

#### 2) Hugging Face repo 사용

```text
LLAMA_CACHE=<download_cache_dir> llama-cli ... -hf <repo> -f <prompt_file>
```

여기서 `HF_TOKEN`은 코드에서 직접 설정하지 않고 부모 환경에서 상속된다.

### 6.4 요약 응답 파싱 방식

`llama-cli`의 `stdout`를 직접 파싱한다.
이 부분이 세 모듈 중 가장 "응답값 파싱" 비중이 큰 로직이다.

처리 순서:
1. 원본 prompt 전체가 `stdout`에 echo된 경우 그 이후만 남김
2. 전체 prompt가 일치하지 않으면 `(truncated)` 마커 뒤를 사용
3. 다음 마커 중 가장 앞에 있는 요약 시작점을 찾음
   - `## 회의록`
   - `**개요**`
   - `개요`
   - `**핵심 논의**`
   - `핵심 논의`
4. 아래 tail marker 이후는 제거
   - `[ Prompt:`
   - `Exiting...`
5. 제목 정규화
   - `**개요**` -> `개요`
   - `**핵심 논의**` -> `핵심 논의`
   - `**결정/합의**` -> `결정/합의`
   - `**후속 조치**` -> `후속 조치`
6. 최종 텍스트를 `summary/<source_stem>.md`로 저장

실패 판정:
- 종료 코드 실패
- 또는 종료 코드는 성공했지만 추출된 요약 텍스트가 비어 있음

실패 시 오류 메시지 선택:
- `stderr` 우선
- 없으면 `stdout`

### 6.5 모델 다운로드 요청 방식

Llama 모델 준비는 별도 다운로드 스크립트가 아니라 `llama-cli` 자체를 이용한다.

호출 형태:

```text
LLAMA_CACHE=<download_cache_dir> llama-cli
  --single-turn
  --simple-io
  --no-display-prompt
  --log-disable
  --no-warmup
  --no-mmproj
  -n 0
  -hf <repo>
  -p ""
```

실행 방식:
- `spawn()`으로 자식 프로세스를 띄움
- `stdout`/`stderr`는 pipe로 받음
- Rust가 `download_cache_dir`를 polling 하면서 새 `.gguf` 파일 생성 여부를 감시
- 새 파일이 보이면 프로세스를 종료시키고 최종 캐시 위치로 `rename()`

즉, 다운로드 성공 판정은 CLI 출력 파싱이 아니라 파일 시스템 관찰 기반이다.

프로세스 종료 후 처리:
- 종료 코드 성공이면 cache 디렉터리에서 `.gguf` 파일을 찾아 최종 경로로 이동
- 종료 코드 실패면 `stderr` 우선, 없으면 `stdout`를 오류 메시지로 사용

### 6.6 재시도 로직

백엔드 실패 추정 시 CPU fallback:
- Windows: CUDA/NVIDIA 관련 문자열 탐지
- macOS: Metal 관련 문자열 탐지

CPU fallback 시 추가 인자:

```text
-ngl 0
--device none
--no-op-offload
--no-kv-offload
--no-mmproj-offload
```

macOS 추가 환경:
- `GGML_METAL=0`
- `GGML_METAL_DEVICES=0`

### 6.7 점검 메모

- 장점: 요약 본문만 추출하려는 후처리가 구현되어 있다.
- 주의점: `stdout` 포맷이 바뀌면 `extract_summary_text()` 휴리스틱이 깨질 수 있다.
- 주의점: Hugging Face 다운로드 성공도 결국 파일 생성 규약에 의존한다.

## 7. 모듈별 응답 처리 비교

| 모듈 | 요청 전달 | 성공 판정 | 응답 파싱 |
| --- | --- | --- | --- |
| `ffprobe` | CLI 인자 | 종료 코드 + JSON 파싱 성공 | `stdout` JSON을 `serde`로 파싱 |
| `ffmpeg` | CLI 인자 | 종료 코드 | 실패 시 `stderr` 텍스트만 사용 |
| `whisper-cli` | CLI 인자 | 종료 코드 + `.txt` 파일 존재 | 실패 시 `stderr`/`stdout`만 사용 |
| whisper 모델 다운로드 스크립트 | CLI 인자 | 종료 코드 + 모델 파일 존재 | 실패 시 `stderr`/`stdout`만 사용 |
| `llama-cli` 요약 | CLI 인자 + 일부 env | 종료 코드 + 추출된 요약 텍스트 비어 있지 않음 | `stdout` 텍스트를 휴리스틱으로 후처리 |
| `llama-cli` 모델 다운로드 | CLI 인자 + `LLAMA_CACHE` env | `.gguf` 파일 발견 또는 종료 코드 성공 후 파일 검출 | 실패 시 `stderr`/`stdout`, 성공은 파일 시스템 기준 |

## 8. 점검 결과 및 리스크

### 8.1 확인된 구현 특성

- 세 모듈 모두 Rust와의 경계가 명확한 CLI 래핑 구조다.
- API 서버와 로컬 CLI가 같은 도메인 로직을 공유한다.
- 외부 툴과의 계약은 대부분 "명령행 인자 + 파일 생성 규약" 형태다.

### 8.2 주의가 필요한 지점

1. `ffmpeg` 성공 시 산출 파일 존재를 후속 검증하지 않는다.
2. `whisper-cli`는 transcript 파일 존재만 확인하고 본문 유효성은 확인하지 않는다.
3. `llama-cli`는 `stdout` 포맷 변화에 취약한 휴리스틱 파서를 사용한다.
4. Llama Hugging Face 다운로드는 구조화 응답이 아니라 캐시 디렉터리 관찰에 의존한다.

### 8.3 문서 불일치

현재 구현과 `docs/architecture.md` 사이에 whisper 모델 다운로드 경로 설명 차이가 있다.

- `docs/architecture.md`: `scripts/download_whisper_model.{sh|bat}`
- 실제 구현: `whisper.cpp/models/download-ggml-model.{sh|cmd}`

따라서 아키텍처 문서 쪽 설명은 현재 코드 기준으로 갱신이 필요하다.

## 9. 결론

현재 RecordRoute의 외부 모듈 연동은 일관된 CLI 래핑 구조로 정리되어 있다.
다만 모듈별 응답 처리 방식은 서로 다르다.

- FFmpeg: `ffprobe`만 JSON 파싱, `ffmpeg` 자체는 종료 코드 중심
- Whisper: 결과 파일 생성 여부 중심
- Llama: `stdout` 후처리 + 파일 캐시 관찰 중심

즉, "CLI 응답값을 직접 파싱"하는 정도는
`ffprobe`와 `llama-cli`에서 높고,
`ffmpeg`와 `whisper-cli`는 대부분 종료 코드와 파일 시스템 규약에 의존한다.
