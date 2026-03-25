# Architecture

## FFmpeg Architecture

### Overview

이 프로젝트에서 FFmpeg는 애플리케이션에 라이브러리로 링크되지 않는다. 현재 구현은 로컬에 빌드된 `ffmpeg` / `ffprobe` 실행 파일을 Rust 코드가 래핑해서 호출하는 구조다.

즉, 실제 통합 지점은 다음 3개다.

1. Unix의 `scripts/build_ffmpeg.sh` 또는 Windows의 `scripts/build_ffmpeg.bat`
   로컬 머신에서 사용할 FFmpeg CLI 바이너리를 `.build/ffmpeg/<os>-<arch>/install/bin` 아래에 빌드한다.
2. `rust/src/ffmpeg.rs`
   빌드된 바이너리 위치를 찾고, `ffprobe`와 `ffmpeg` 호출 인자를 구성하는 얇은 래퍼다.
3. `rust/src/app.rs`
   입력 검증, 작업 디렉터리 생성, 상태 기록, probe/convert 실행 순서를 오케스트레이션한다.

핵심은 "FFmpeg 소스 트리를 직접 호출하는 것"이 아니라, "프로젝트 내부에 빌드된 FFmpeg CLI 툴체인을 고정된 경로에서 찾아 실행하는 것"이다.

### Integration Model

현재 구조는 아래와 같다.

```mermaid
flowchart LR
    A["CLI entrypoint (`rust/src/main.rs`)"] --> B["`main_cli()`"]
    B --> C["`run_with_repo_root()`"]
    C --> D["Reusable completed job lookup in `db/index.json`"]
    D --> E["Cache hit: return existing output paths"]
    D --> F["Cache miss: `Toolchain::discover()`"]
    F --> G["Local binaries in `.build/ffmpeg/<target>/install/bin`"]
    C --> H["`probe_audio_input()` via `ffprobe`"]
    C --> I["`run_conversion()` via `ffmpeg`"]
    C --> J["`db/index.json` + job output files"]
```

이 모델의 특징은 다음과 같다.

- Rust 바이너리는 `libavcodec`, `libavformat` 같은 FFmpeg 라이브러리를 직접 링크하지 않는다.
- FFmpeg 연동은 전부 `std::process::Command` 기반의 외부 프로세스 실행으로 이루어진다.
- 런타임은 시스템 전역 `ffmpeg`를 찾지 않고, 프로젝트가 빌드한 로컬 툴체인만 사용한다.
- 입력 분석은 `ffprobe`, 실제 변환은 `ffmpeg`로 역할이 분리되어 있다.

### Components

#### 1. Build script

`scripts/build_ffmpeg.sh` / `scripts/build_ffmpeg.bat`는 FFmpeg 툴체인을 현재 OS / CPU 아키텍처 기준 디렉터리에 설치한다.

- 타깃 경로 계산: `.build/ffmpeg/<platform_os>-<platform_arch>/install/bin`
- 캐시 동작: 이미 `ffmpeg`와 `ffprobe` 실행 파일이 있으면 바로 그 경로를 출력하고 종료
- 빌드 대상: `ffmpeg`, `ffprobe`
- 주요 configure 옵션:
  - `--disable-ffplay`
  - `--disable-doc`
  - `--disable-network`
  - `--disable-autodetect`
  - `--disable-debug`
- Windows 전제:
  - `scripts/build_ffmpeg.bat`는 FFmpeg 자체 빌드는 POSIX shell + GNU `make` + `cmp`/`cygpath` 환경(MSYS2 `diffutils` 포함, 또는 Git for Windows 등)에 위임한다.
  - 동시에 MSVC toolchain(`cl`, `nmake`)이 필요하다.
  - 일반 PowerShell / `cmd.exe` 세션에서도 `VsDevCmd.bat`를 자동 로드하고, 현재 MSVC `PATH`를 POSIX shell로 다시 전달해서 `cl.exe`를 찾을 수 있게 만든다.

의미상 이 스크립트는 "서브모듈 관리"가 아니라 "런타임이 사용할 로컬 CLI 툴체인 준비"를 담당한다.

#### 2. Toolchain wrapper

`rust/src/ffmpeg.rs`의 `Toolchain`은 FFmpeg 연동의 진입점이다.

- `Toolchain::discover(repo_root)`
  - 현재 플랫폼에 맞는 `scripts/build_ffmpeg.sh` 또는 `scripts/build_ffmpeg.bat` 위치를 함께 저장한다.
  - `.build/ffmpeg/<target>/install/bin/ffmpeg`
  - `.build/ffmpeg/<target>/install/bin/ffprobe`
  - 두 파일이 모두 존재해야 성공한다.
- 툴체인이 없으면 빌드 스크립트 경로를 포함한 에러를 반환한다.

즉, Rust 쪽은 FFmpeg를 "빌드 가능한 소스"가 아니라 "이미 준비된 실행 파일 세트"로 취급한다.

#### 3. Probe wrapper

`probe_audio_input()`은 입력 오디오의 채널 정보를 `ffprobe`로 읽는다.

실행 형태는 개념적으로 아래와 같다.

```bash
ffprobe \
  -v error \
  -select_streams a:0 \
  -show_entries stream=channels,channel_layout \
  -of json \
  <input>
```

이 결과를 JSON으로 파싱해 `ProbeInfo { channels, channel_layout }`로 변환한다.

이 단계의 목적은 두 가지다.

- 실제 채널 수를 기준으로 출력 파일 개수를 결정
- 변환용 `filter_complex` 문자열을 동적으로 생성

#### 4. Conversion wrapper

`run_conversion()`은 `ffmpeg` 명령행을 조립해서 한 번의 실행으로 다음 결과를 만든다.

- 각 입력 채널별 모노 WAV
- 전체 채널을 합친 모노 믹스 WAV

출력 포맷은 고정되어 있다.

- codec: `pcm_s16le`
- sample rate: `16000`
- channels: `1`

입력에서 비오디오 스트림은 명시적으로 제외한다.

- `-vn`
- `-sn`
- `-dn`

### Filter Graph Strategy

변환의 핵심은 `build_filter_complex(channels)`가 만드는 필터 그래프다.

예를 들어 스테레오 입력이면 아래 문자열이 생성된다.

```text
[0:a:0]pan=mono|c0<c0+c1[mix];[0:a:0]pan=mono|c0=c0[split01];[0:a:0]pan=mono|c0=c1[split02]
```

구조는 항상 동일하다.

- `[mix]`
  모든 입력 채널을 더해서 하나의 모노 트랙 생성
- `[splitNN]`
  각 원본 채널을 개별 모노 트랙으로 분리

그 뒤 `-map [splitNN]`과 `-map [mix]`를 사용해 파일로 떨어뜨린다.

이 방식의 장점은 한 번의 `ffmpeg` 실행으로 필요한 산출물을 모두 생성할 수 있다는 점이다.

### Application Flow

실제 실행 순서는 `rust/src/app.rs`에서 관리한다.

1. 입력 파일 경로를 인자 또는 프롬프트로 받는다.
2. 입력 경로가 존재하는 파일인지 검증하고 canonical path로 정규화한다.
3. `db/index.json`에서 같은 `source_path`의 최신 `completed` job을 뒤에서부터 찾는다.
4. 저장된 출력 파일이 모두 남아 있으면 새 job을 만들지 않고 기존 `job_id`, `job_dir`, 출력 경로를 그대로 반환한다.
5. 재사용 가능한 완료 job이 없을 때만 `Toolchain::discover()`로 로컬 FFmpeg 바이너리를 찾는다.
6. `db/<job_id>` 작업 디렉터리를 만들고 `db/index.json`에 running 상태를 먼저 기록한다.
7. `probe_audio_input()`으로 채널 수를 파악한다.
8. `ConversionOutputs::new()`로 예상 출력 파일 경로를 계산한다.
9. `run_conversion()`으로 실제 변환을 수행한다.
10. 성공하면 index를 `completed`로 갱신하고 출력 경로를 저장한다.
11. 실패하면 부분 생성 파일을 삭제하고 index를 `failed`로 갱신한다.

즉, FFmpeg 호출은 독립 함수이지만, 실제 운영 문맥에서는 "완료 결과 재사용 확인 -> miss일 때만 변환" 순서의 job 관리 로직 안에서 수행된다.

### Output Layout

작업이 성공하면 산출물은 job 디렉터리에 저장된다.

```text
db/
  <job_id>/
    mono_mix.wav
    channel_01.wav
    channel_02.wav
    ...
```

인덱스 파일에는 다음 정보가 함께 기록된다.

- job 상태: `running` / `completed` / `failed`
- 입력 원본 경로
- probe 결과: 채널 수, 채널 레이아웃
- split 전략: `per_channel_plus_merged_mono`
- 출력 파일 경로
- 실패 메시지

같은 입력 파일을 다시 요청하면 `IndexStore`가 최신 완료 job부터 거슬러 올라가며 재사용 가능한 출력 세트를 찾는다. 유효한 결과가 있으면 기존 경로만 다시 노출하고, 새 job 디렉터리를 만들지 않는다.

즉, FFmpeg 래퍼는 단순 변환기 역할만 하고, 실행 이력과 결과 추적 및 완료 결과 재사용 판단은 `IndexStore`가 담당한다.

### Failure Handling

현재 래퍼 계층의 실패 처리는 비교적 명확하다.

- 툴체인 없음
  - `.build/.../ffmpeg`, `ffprobe`가 없으면 즉시 실패
  - 에러 메시지에 현재 플랫폼용 `scripts/build_ffmpeg.sh` 또는 `scripts/build_ffmpeg.bat` 경로를 포함
- `ffprobe` 실패
  - stderr를 수집해 상위로 전달
- `ffmpeg` 실패
  - stderr를 수집해 상위로 전달
  - 이미 만들어진 부분 출력 파일은 삭제
- 오디오 스트림/채널 정보 없음
  - probe 결과 검증 단계에서 실패

이 구조 덕분에 FFmpeg 실행 실패가 애플리케이션 상태 기록과 분리되지 않고, job 상태와 함께 일관되게 남는다.

### Current Boundaries

현재 구현 범위는 명확하다.

- FFmpeg는 외부 CLI 프로세스로만 사용한다.
- 시스템 PATH의 `ffmpeg`를 사용하지 않는다.
- FFmpeg 라이브러리 API(`libav*`)를 직접 사용하지 않는다.
- 자동 빌드/자동 bootstrap는 하지 않는다.
  - 툴체인이 없으면 안내 메시지만 반환한다.
- 현재 변환 목적은 "채널별 모노 분리 + 전체 모노 믹스 생성"에 한정된다.

### Summary

현재 아키텍처에서 FFmpeg는 "프로젝트 내부에 빌드해 둔 CLI 툴체인"이며, Rust 애플리케이션은 이를 얇은 프로세스 래퍼로 감싸서 사용한다.

정리하면:

- 준비: Unix는 `scripts/build_ffmpeg.sh`, Windows는 `scripts/build_ffmpeg.bat`가 로컬 바이너리를 만든다.
- 재사용 확인: `IndexStore`가 canonical input path 기준으로 기존 완료 결과를 찾는다.
- 발견: cache miss일 때만 `Toolchain::discover()`가 고정된 설치 경로를 찾는다.
- 분석: `probe_audio_input()`이 `ffprobe` JSON 출력을 파싱한다.
- 변환: `run_conversion()`이 `filter_complex` 기반 단일 `ffmpeg` 실행을 구성한다.
- 운영: `run_with_repo_root()`가 job 디렉터리, index, 완료 결과 재사용을 관리한다.

따라서 현재 RecordRoute의 FFmpeg 연동은 "서브모듈 직접 통합"이 아니라 "프로젝트 로컬 FFmpeg CLI를 사용하는 Rust wrapper architecture"라고 보는 것이 정확하다.

---

## Whisper.cpp Architecture

### Overview

`whisper.cpp`도 FFmpeg와 같은 방향으로 통합되어 있다. Rust 바이너리가 `whisper.cpp` 라이브러리를 직접 링크하거나 FFI로 호출하지 않고, 프로젝트 내부에 빌드된 `whisper-cli` 실행 파일을 래핑해서 사용한다.

현재 Whisper 통합의 핵심 지점은 다음 4개다.

1. Unix의 `scripts/build_whisper.sh` 또는 Windows의 `scripts/build_whisper.bat`
   로컬 머신에서 사용할 `whisper-cli` 바이너리를 `.build/whisper/<os>-<arch>/bin` 아래에 빌드한다.
2. `rust/src/whisper.rs`
   빌드된 `whisper-cli` 위치를 찾고, 모델 경로를 해석하고, 필요한 경우 모델 다운로드를 수행한 뒤 실제 STT 명령행을 실행하는 얇은 래퍼다.
3. `rust/src/app.rs`
   CLI 모드 선택, `db/index.json` 기반 job 폴더 선택, `stt/` 디렉터리 생성, 폴더 내 오디오 파일 일괄 STT 실행을 오케스트레이션한다.
4. `.env`
   CLI 시작 시 자동 로드되며 `RECORDROUTE_WHISPER_MODEL` 같은 런타임 설정을 제공한다.

핵심은 FFmpeg와 동일하게 "프로젝트 내부에 준비된 Whisper CLI 툴체인을 고정된 경로에서 찾아 실행하는 것"이다.

### Integration Model

현재 구조는 아래와 같다.

```mermaid
flowchart LR
    A["CLI entrypoint (`rust/src/main.rs`)"] --> B["`main_cli()`"]
    B --> C["Load repo `.env`"]
    C --> D{"arg-based mode?"}
    D -->|"ffmpeg <input>"| E["Existing FFmpeg flow"]
    D -->|"stt"| F["STT flow"]
    D -->|"no args"| G["Prompt: `1. ffmpeg` / `2. stt` / `3. summary`"]
    G --> E
    G --> F
    F --> H["`IndexStore::list_jobs()`"]
    H --> I["Filter existing `job_dir` with supported audio files"]
    I --> J["Prompt numbered folder selection"]
    J --> K["`Whisper Toolchain::discover()`"]
    K --> L["Local `whisper-cli` in `.build/whisper/<target>/bin`"]
    K --> M["Model path resolution + optional download"]
    F --> N["`run_transcription()` per audio file"]
    N --> O["`db/<job_id>/stt/*.txt`"]
```

이 모델의 특징은 다음과 같다.

- Rust 바이너리는 `whisper.cpp`의 C API를 직접 사용하지 않는다.
- Whisper 연동은 전부 `std::process::Command` 기반의 외부 프로세스 실행이다.
- 런타임은 시스템 PATH의 `whisper-cli`를 사용하지 않고, 프로젝트가 빌드한 로컬 바이너리만 사용한다.
- STT는 `db/index.json`을 "후보 폴더 인덱스"로 사용하지만, 결과 텍스트는 index에 다시 기록하지 않고 파일 시스템에만 저장한다.

### Components

#### 1. Build script

`scripts/build_whisper.sh` / `scripts/build_whisper.bat`는 Whisper 툴체인을 현재 OS / CPU 아키텍처 기준 디렉터리에 준비한다.

- 타깃 경로 계산: `.build/whisper/<platform_os>-<platform_arch>/bin`
- 캐시 동작: 이미 `whisper-cli` 실행 파일이 있으면 바로 그 경로를 출력하고 종료
- 빌드 방식: `cmake -S whisper.cpp -B .build/.../build`
- 빌드 대상: `whisper-cli`
- 주요 CMake 옵션:
  - `-DCMAKE_BUILD_TYPE=Release`
  - `-DBUILD_SHARED_LIBS=OFF`
  - `-DWHISPER_BUILD_TESTS=OFF`
  - `-DWHISPER_BUILD_SERVER=OFF`
  - `-DWHISPER_BUILD_EXAMPLES=ON`
- Windows 구현:
  - `scripts/build_whisper.bat`는 MSVC + CMake(`NMake Makefiles`) 기준으로 동일 산출물을 만든다.

의미상 이 스크립트는 "Whisper 소스를 직접 호출"하는 것이 아니라 "런타임이 사용할 로컬 `whisper-cli` 준비"를 담당한다.

#### 2. Toolchain wrapper

`rust/src/whisper.rs`의 `Toolchain`은 Whisper 연동의 진입점이다.

- `Toolchain::discover(repo_root)`
  - 현재 플랫폼에 맞는 `scripts/build_whisper.sh` 또는 `scripts/build_whisper.bat` 위치를 저장한다.
  - Unix는 `whisper.cpp/models/download-ggml-model.sh`, Windows는 `whisper.cpp/models/download-ggml-model.cmd` 위치를 저장한다.
  - `.build/whisper/<target>/bin/whisper-cli`
  - 모델 경로를 함께 해석한다.
- 모델 경로 우선순위:
  - `RECORDROUTE_WHISPER_MODEL`
  - 기본값 `models/whisper/ggml-base.bin`
- shorthand 지원:
  - `models/whisper/large-v3-turbo` 같은 값도 허용한다.
  - 이 경우 내부적으로 `models/whisper/ggml-large-v3-turbo.bin` 형태로 정규화해 해석한다.
- 툴체인이 없으면 빌드 스크립트 경로를 포함한 에러를 반환한다.

즉, Rust 쪽은 Whisper도 "이미 준비된 CLI 실행 파일 + 모델 파일" 조합으로 취급한다.

#### 3. Model bootstrap

모델 파일은 `Toolchain::ensure_model()`이 관리한다.

- 설정된 모델 파일이 이미 있으면 그대로 사용한다.
- 모델 파일이 없으면 Unix는 `whisper.cpp/models/download-ggml-model.sh`, Windows는 `whisper.cpp/models/download-ggml-model.cmd`를 실행해 다운로드를 시도한다.
- 다운로드 대상 디렉터리는 모델 파일의 부모 디렉터리다.
- 환경변수에 shorthand가 들어온 경우에도 실제 다운로드 대상 파일명은 `ggml-<model>.bin` 규칙으로 정규화된다.

예를 들어 기본 설정이면 아래 위치가 사용된다.

```text
models/whisper/ggml-base.bin
```

또는 아래처럼 shorthand를 써도 같은 방식으로 해석된다.

```text
RECORDROUTE_WHISPER_MODEL=models/whisper/large-v3-turbo
-> models/whisper/ggml-large-v3-turbo.bin
```

즉, 기본 동작은 "repo-local model cache"를 유지하되, 없으면 자동으로 채워 넣는 방식이다.

#### 4. Transcription wrapper

`run_transcription()`은 `whisper-cli` 명령행을 조립해서 단일 오디오 파일의 텍스트 산출물을 만든다.

개념적으로는 아래와 같은 형태다.

```bash
whisper-cli \
  -m <model-path> \
  -f <input-audio> \
  -l auto \
  -otxt \
  -np \
  -of <output-prefix>
```

여기서 실제 결과 파일은 `<output-prefix>.txt`다.

현재 고정 동작은 다음과 같다.

- 언어 설정: `auto`
- 출력 포맷: plain text (`-otxt`)
- 콘솔 출력 최소화: `-np`
- 기존 같은 이름의 `.txt`가 있으면 먼저 제거하고 새 결과로 덮어쓴다.

### Application Flow

STT 실행 순서는 `rust/src/app.rs`에서 관리한다.

1. CLI 시작 시 repo root 아래 `.env`를 자동 로드한다.
2. CLI 인자를 해석한다.
3. 인자가 없으면 콘솔에서 `1. ffmpeg 작업`, `2. stt 작업`, `3. summary 작업`을 숫자로 묻는다.
4. `stt` 모드가 선택되면 `db/index.json`의 전체 job을 읽는다.
5. 각 job에 대해 `job_dir`가 실제 디렉터리인지 확인한다.
6. `job_dir` 바로 아래에서 지원 오디오 파일만 수집한다.
   - 현재 지원 확장자: `wav`, `mp3`, `flac`, `ogg`
7. 오디오 파일이 하나 이상 있는 job만 선택 후보로 표시한다.
8. 콘솔에 번호, `job_id`, 원본 파일명, 폴더 경로를 출력하고 숫자 입력을 받는다.
9. 선택된 `job_dir` 아래에 `stt/` 디렉터리를 만든다.
10. 각 오디오 파일마다 `run_transcription()`을 한 번씩 실행한다.
11. 성공하면 `stt/<audio-basename>.txt`가 생성된다.

즉, STT 모드는 "index를 조회해 기존 job 폴더를 선택하고, 그 폴더 안의 오디오 파일들을 일괄 후처리"하는 구조다.

### CLI Boundaries

현재 CLI 규칙은 다음과 같다.

- `recordroute_rust ffmpeg <input>`
  - FFmpeg 변환을 즉시 실행
- `recordroute_rust stt`
  - STT 폴더 선택 흐름을 즉시 실행
- `recordroute_rust summary`
  - summary 폴더 선택 흐름을 즉시 실행
- `recordroute_rust <input>`
  - 기존 호환성을 위해 bare input은 FFmpeg 입력 파일로 해석
- `recordroute_rust`
  - `ffmpeg` / `stt` / `summary` 모드 선택 프롬프트를 띄운다

즉, STT 추가 이후에도 기존 FFmpeg 단일 파일 진입 방식은 유지된다.

### Output Layout

STT가 성공하면 결과는 선택된 job 디렉터리 아래에 저장된다.

```text
db/
  <job_id>/
    mono_mix.wav
    channel_01.wav
    channel_02.wav
    stt/
      mono_mix.txt
      channel_01.txt
      channel_02.txt
```

파일명 규칙은 단순하다.

- 입력 파일 basename 유지
- 확장자만 `.txt`로 교체

예:

- `channel_01.wav` -> `stt/channel_01.txt`
- `mono_mix.wav` -> `stt/mono_mix.txt`

중요한 점은 STT 산출물은 현재 `db/index.json`에 기록되지 않는다는 것이다. 인덱스는 선택 후보를 제공할 뿐이며, STT 결과 추적은 파일 시스템 경로 자체가 담당한다.

### Failure Handling

현재 Whisper 계층의 실패 처리는 다음과 같다.

- 툴체인 없음
  - `.build/.../whisper-cli`가 없으면 즉시 실패
  - 에러 메시지에 현재 플랫폼용 `scripts/build_whisper.sh` 또는 `scripts/build_whisper.bat` 경로를 포함
- 모델 없음
  - 설정된 모델 파일이 없으면 다운로드를 시도
  - 다운로드 스크립트가 없거나 다운로드 실패 시 에러 반환
- 모델 경로 정규화 실패
  - 파일명이 비어 있거나 부모 디렉터리를 결정할 수 없으면 기대한 다운로드 대상 경로를 만들 수 없어 실패할 수 있다
- 후보 폴더 없음
  - `db/index.json` 안에 실제 오디오 파일을 가진 `job_dir`가 하나도 없으면 실패
- `whisper-cli` 실패
  - stderr/stdout를 수집해 상위로 전달
  - 해당 파일의 `.txt`가 생성되지 않으면 전체 STT 실행을 실패로 처리

현재 STT는 index 상태를 갱신하지 않으므로, 실패 기록은 콘솔 반환값에 남고 부분적으로 이미 생성된 다른 `.txt` 파일은 그대로 유지될 수 있다.

### Current Boundaries

현재 구현 범위는 명확하다.

- Whisper는 외부 CLI 프로세스로만 사용한다.
- `whisper.cpp` 라이브러리 API를 직접 사용하지 않는다.
- 시스템 PATH의 `whisper-cli`를 사용하지 않는다.
- 자동 모델 다운로드는 지원하지만, 자동 빌드는 하지 않는다.
  - 툴체인이 없으면 안내 메시지만 반환한다.
- STT 대상은 `db/index.json`에 있는 job 디렉터리 중 실제 오디오 파일이 있는 폴더다.
- 오디오 탐색은 현재 job 디렉터리의 바로 아래 파일만 대상으로 하며, 재귀 탐색은 하지 않는다.
- STT 결과는 `db/index.json`에 기록하지 않는다.

### Summary

현재 아키텍처에서 Whisper는 "프로젝트 내부에 빌드해 둔 `whisper-cli` + repo-local model cache"이며, Rust 애플리케이션은 이를 얇은 프로세스 래퍼로 감싸서 사용한다.

정리하면:

- 준비: Unix는 `scripts/build_whisper.sh`, Windows는 `scripts/build_whisper.bat`가 로컬 `whisper-cli`를 만든다.
- 설정: `.env`가 `RECORDROUTE_WHISPER_MODEL`을 공급한다.
- 발견: `Toolchain::discover()`가 고정된 설치 경로와 모델 경로를 찾는다.
- 모델 보장: `ensure_model()`이 필요 시 기본 경로나 shorthand 설정을 정규화한 경로에 모델을 다운로드한다.
- 선택: `run_stt_with_repo_root()`가 `db/index.json`을 읽어 후보 폴더를 구성한다.
- 변환: `run_transcription()`이 파일별 `whisper-cli` 실행을 구성한다.
- 저장: 결과는 각 job 디렉터리 아래 `stt/*.txt`로 저장된다.

따라서 현재 RecordRoute의 Whisper 연동도 FFmpeg와 동일하게 "서브모듈 직접 통합"이 아니라 "프로젝트 로컬 Whisper CLI를 사용하는 Rust wrapper architecture"라고 보는 것이 정확하다.

## llama.cpp Architecture

### Overview

`llama.cpp`도 FFmpeg와 Whisper와 같은 패턴으로 통합되어 있다. Rust 바이너리가 `libllama`를 직접 링크하거나 FFI로 호출하지 않고, 프로젝트 내부에 빌드된 `llama-cli` 실행 파일을 래핑해서 요약 작업에 사용한다.

요약 기능의 실제 통합 지점은 다음 4개다.

1. Unix의 `scripts/build_llama.sh` 또는 Windows의 `scripts/build_llama.bat`
   로컬 머신에서 사용할 `llama-cli` 바이너리를 `.build/llama/<os>-<arch>/bin` 아래에 빌드한다.
2. `rust/src/llama.rs`
   빌드된 `llama-cli` 위치를 찾고, 모델 설정을 해석하고, 실제 요약 명령행을 실행하는 얇은 래퍼다.
3. `rust/src/app.rs`
   `summary` 모드 선택, `db/index.json` 기반 폴더 선택, 프롬프트 구성, `summary/` 디렉터리 생성, 결과 저장을 오케스트레이션한다.
4. `.env`
   CLI 시작 시 자동 로드되며 `RECORDROUTE_LLAMA_MODEL`, `HF_TOKEN` 같은 요약 관련 환경변수를 제공한다.

핵심은 "llama.cpp 라이브러리 API 직접 연동"이 아니라, "프로젝트 내부에 빌드된 `llama-cli`를 고정된 경로에서 찾아 subprocess로 실행"하는 것이다.

### Integration Model

현재 구조는 아래와 같다.

```mermaid
flowchart LR
    A["CLI entrypoint (`rust/src/main.rs`)"] --> B["`main_cli()`"]
    B --> C["Load repo `.env`"]
    C --> D["`summary` mode in `rust/src/app.rs`"]
    D --> E["`IndexStore::list_jobs()` from `db/index.json`"]
    E --> F["Eligible folders with `stt/*.txt`"]
    F --> G["Numeric folder selection"]
    G --> H["`llama::Toolchain::discover()`"]
    H --> I["Local `llama-cli` in `.build/llama/<target>/bin`"]
    G --> J["Combined prompt file from `stt/*.txt`"]
    J --> K["`run_summary_generation()`"]
    K --> L["`db/<job_id>/summary/<source-stem>.md`"]
```

이 모델의 특징은 다음과 같다.

- Rust 바이너리는 `llama.cpp` 라이브러리를 직접 호출하지 않는다.
- 요약은 전부 `std::process::Command` 기반의 외부 프로세스 실행으로 이루어진다.
- 런타임은 시스템 PATH의 `llama-cli`를 쓰지 않고, 프로젝트가 빌드한 로컬 바이너리만 사용한다.
- `db/index.json`은 여전히 "선택 후보 인덱스" 역할만 하고, 요약 결과 자체는 파일 시스템에만 저장된다.

### Components

#### 1. Build script

`scripts/build_llama.sh` / `scripts/build_llama.bat`는 Llama 툴체인을 현재 OS / CPU 아키텍처 기준 디렉터리에 준비한다.

- 타깃 경로 계산: `.build/llama/<platform_os>-<platform_arch>/bin`
- 캐시 동작: 이미 `llama-cli` 실행 파일이 있으면 바로 그 경로를 출력하고 종료
- 빌드 방식: `cmake -S llama.cpp -B .build/.../build`
- 빌드 대상: `llama-cli`
- 주요 옵션:
  - `-DBUILD_SHARED_LIBS=OFF`
  - `-DLLAMA_BUILD_TOOLS=ON`
  - `-DLLAMA_BUILD_TESTS=OFF`
  - `-DLLAMA_BUILD_SERVER=ON`
  - `-DLLAMA_BUILD_EXAMPLES=OFF`
- Windows 구현:
  - `scripts/build_llama.bat`는 MSVC + CMake(`NMake Makefiles`) 기준으로 동일 산출물을 만든다.

의미상 이 스크립트는 "llama.cpp 서브모듈 관리"가 아니라 "런타임이 사용할 로컬 `llama-cli` 준비"를 담당한다.

#### 2. Toolchain wrapper

`rust/src/llama.rs`의 `Toolchain`은 요약 연동의 진입점이다.

- `Toolchain::discover(repo_root)`
  - 현재 플랫폼에 맞는 `scripts/build_llama.sh` 또는 `scripts/build_llama.bat` 위치를 저장한다.
  - `.build/llama/<target>/bin/llama-cli`
  - 실행 파일이 없으면 빌드 스크립트 경로를 포함한 에러를 반환한다.
- 모델 해석
  - `RECORDROUTE_LLAMA_MODEL`이 설정되지 않으면 기본값 `ggml-org/gemma-3-4b-it-GGUF`를 사용한다.
  - 설정값이 실제 파일이면 `-m <path>`로 호출한다.
  - 설정값이 실제 파일이 아니면 Hugging Face repo로 간주하고 `-hf <repo>`로 호출한다.
- 다운로드 캐시
  - Hugging Face 다운로드는 시스템 전역 캐시 대신 `models/llama/hf/.cache/<repo-key>` 아래에서 진행한다.
  - 다운로드가 끝나면 최종 모델 파일을 `models/llama/hf/<repo-key>.gguf`로 이동한다.
- 인증
  - `HF_TOKEN`은 child process 환경으로 그대로 전달되며, gated/private Hugging Face 모델 접근에 사용된다.

즉, Rust 쪽은 Llama를 "이미 준비된 CLI 실행 파일 + 환경변수 기반 모델 설정"으로 취급한다.

#### 3. Prompt builder

요약 프롬프트는 `rust/src/app.rs`에서 만든다.

- 대상: 선택된 job 디렉터리의 `stt/*.txt`
- 정렬: 파일명을 기준으로 lexicographic sort
- 포함 방식:
  - `[channel_01.txt]`
  - `[channel_02.txt]`
  - `[mono_mix.txt]`
- 고정 지시:
  - 동일 오디오의 중복 STT를 병합
  - 추측 금지
  - 한국어 회의록 형식
  - 섹션: `개요`, `핵심 논의`, `결정/합의`, `후속 조치`

프롬프트는 길이 안전성을 위해 명령행 문자열이 아니라 임시 파일로 만들어 `llama-cli -f <prompt-file>`에 전달한다.

#### 4. Generation wrapper

`run_summary_generation()`은 `llama-cli` 명령행을 조립해서 단일 요약 텍스트를 생성한다.

개념적 실행 형태는 아래와 같다.

```bash
llama-cli \
  --single-turn \
  --simple-io \
  --no-display-prompt \
  --log-disable \
  -n 1024 \
  -m models/llama/hf/ggml-org__gemma-3-4b-it-GGUF.gguf \
  -f <prompt-file> \
  > <summary-file>
```

실제 구현에서는 `stdout`을 Rust가 받아 후처리한 뒤 summary 파일로 저장한다. `llama-cli`가 배너나 prompt echo를 섞어 출력할 수 있기 때문에, Rust 쪽에서 회의록 본문만 추출해 저장한다.

### Application Flow

실제 실행 순서는 `rust/src/app.rs`에서 관리한다.

1. CLI 시작 시 repo root 아래 `.env`를 자동 로드한다.
2. `summary` 모드가 선택되면 `db/index.json`의 전체 job을 읽는다.
3. 각 job에 대해 `job_dir/stt` 안의 `.txt` 파일 존재 여부를 확인한다.
4. 전사 파일이 있는 폴더만 후보로 보여주고 숫자 입력으로 하나를 선택한다.
5. 선택된 job 아래 `summary/<source-stem>.md`가 이미 있으면 그 경로를 그대로 반환하고 종료한다.
6. 결과 파일이 없을 때만 `llama::Toolchain::discover()`로 로컬 `llama-cli`를 찾는다.
7. 선택된 job 아래 `summary/` 디렉터리를 만든다.
8. `stt/*.txt` 전체를 읽어 하나의 프롬프트 파일로 합친다.
9. `run_summary_generation()`으로 요약을 실행한다.
10. 성공하면 프롬프트 임시 파일을 지우고 `summary/<source-stem>.md`만 남긴다.

즉, Llama 호출은 독립 함수이지만, 운영 문맥에서는 "후보 폴더 선택 -> 프롬프트 합성 -> 요약 생성 -> summary 저장" 순서의 job 후처리 로직 안에서 실행된다.

### Output Layout

요약이 성공하면 결과는 선택된 job 디렉터리 아래에 저장된다.

```text
db/
  <job_id>/
    stt/
      channel_01.txt
      channel_02.txt
      mono_mix.txt
    summary/
      input.md
```

파일명 규칙은 원본 오디오 파일 basename의 stem을 그대로 사용하고 확장자만 `.md`로 바꾼다.

예:

- `input.wav` -> `summary/input.md`
- `meeting.m4a` -> `summary/meeting.md`

중요한 점은 요약 산출물도 현재 `db/index.json`에 기록되지 않는다는 것이다. 인덱스는 선택 후보를 제공할 뿐이며, 요약 결과 추적은 파일 경로 자체가 담당한다.

동일한 job에 대해 같은 summary를 다시 요청하면, 기존 `summary/<source-stem>.md`가 있으면 Llama를 다시 실행하지 않고 그 파일 경로를 그대로 반환한다.

### Failure Handling

현재 Llama 계층의 실패 처리는 다음과 같다.

- 툴체인 없음
  - `.build/.../llama-cli`가 없으면 즉시 실패
  - 에러 메시지에 현재 플랫폼용 `scripts/build_llama.sh` 또는 `scripts/build_llama.bat` 경로를 포함
- 후보 폴더 없음
  - `db/index.json` 안에 실제 `stt/*.txt`가 있는 `job_dir`가 하나도 없으면 실패
- 프롬프트 파일 생성 실패
  - `summary/` 아래 임시 프롬프트 파일을 만들지 못하면 실패
- `llama-cli` 실패
  - stderr/stdout를 수집해 상위로 전달
  - 부분 생성된 summary 파일이 있으면 삭제
- 출력 파일 누락
  - `llama-cli`가 성공 종료했더라도 최종 요약 파일이 없으면 실패

현재 요약도 index 상태를 갱신하지 않으므로, 실패 기록은 콘솔 반환값에 남고 파일 시스템 산출물만 결과 상태를 표현한다.

### Current Boundaries

현재 구현 범위는 명확하다.

- Llama는 외부 CLI 프로세스로만 사용한다.
- `llama.cpp` 라이브러리 API를 직접 사용하지 않는다.
- 시스템 PATH의 `llama-cli`를 사용하지 않는다.
- 자동 Hugging Face 다운로드는 `llama.cpp` 기본 동작에 맡기고, 별도 다운로드 래퍼는 만들지 않는다.
- 요약 대상은 `db/index.json`에 있는 job 디렉터리 중 실제 `stt/*.txt`가 있는 폴더다.
- 프롬프트는 단일 패스이며, chunking/multi-pass summarization은 아직 없다.
- 요약 결과는 `db/index.json`에 기록하지 않는다.

### Summary

현재 아키텍처에서 Llama는 "프로젝트 내부에 빌드해 둔 `llama-cli` + env 기반 모델 설정"이며, Rust 애플리케이션은 이를 얇은 프로세스 래퍼로 감싸서 사용한다.

정리하면:

- 준비: Unix는 `scripts/build_llama.sh`, Windows는 `scripts/build_llama.bat`가 로컬 `llama-cli`를 만든다.
- 설정: `.env`가 `RECORDROUTE_LLAMA_MODEL`, `HF_TOKEN`을 공급한다.
- 발견: `Toolchain::discover()`가 고정된 설치 경로와 모델 소스를 해석한다.
- 선택: `run_summary_with_repo_root()`가 `db/index.json`을 읽어 `stt/*.txt`가 있는 후보를 구성한다.
- 프롬프트: transcript 파일들을 하나로 합쳐 회의록 지시문과 함께 prompt file을 만든다.
- 생성: `run_summary_generation()`이 `llama-cli` 실행을 구성한다.
- 저장: 결과는 각 job 디렉터리 아래 `summary/<source-stem>.md`로 저장된다.

따라서 현재 RecordRoute의 Llama 연동도 FFmpeg, Whisper와 동일하게 "프로젝트 로컬 Llama CLI를 사용하는 Rust wrapper architecture"라고 보는 것이 정확하다.
