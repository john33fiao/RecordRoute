# API TODO

## 목적

현재 Rust 코드는 대부분의 핵심 기능을 CLI와 내부 함수로 제공하고 있고, HTTP 서버는 `POST /server/ping`만 노출한다.

이 문서는 다음 두 가지를 정리한다.

1. 현재 Rust 코드가 실제로 제공하는 기능
2. 그 기능을 전부 API로 제공하려면 어떤 리소스와 엔드포인트가 필요한지

## 현재 구현 상태 요약

- 현재 HTTP API로 동작하는 것은 `POST /server/ping` 하나다.
- 실제 제품 기능은 `rust/src/app.rs`의 CLI 오케스트레이션에 몰려 있다.
- 작업 이력과 산출물 메타데이터는 `db/index.json`이 사실상 저장소 역할을 한다.
- 오디오 처리, STT, 요약은 각각 FFmpeg CLI, Whisper CLI, Llama CLI를 Rust가 래핑해서 실행한다.

## 사용자가 생각한 최소 API 흐름

현재 요구하신 핵심 흐름은 아래와 같다.

1. 오디오 파일 경로를 전달하면 ffmpeg 작업 시작중이라는 메시지 반환
2. 기존에 이미 전달한 오디오 파일이면 작업 완료된 경로 반환
3. ffmpeg 작업 완료된 오디오 파일 목록 요청 및 반환
4. ffmpeg 작업 완료된 오디오 파일의 STT 작업 요청 및 작업 시작 메시지 반환
5. STT 작업 완료된 항목의 텍스트 요청 및 반환
6. 특정 항목의 요약 작업 요청 및 작업 시작 메시지 반환
7. 요약 작업 완료된 항목의 텍스트 요청 및 반환

이 흐름 자체는 맞다. 다만 실제 API로 사용하려면 아래 항목이 추가로 필요하다.

- 작업 상태 조회
  - `시작중`만 반환하면 클라이언트가 완료 여부를 확인할 방법이 없다.
- 실패 상태 및 에러 메시지 조회
  - `failed`와 `error_message`를 API에서 확인할 수 있어야 한다.
- 안정적인 식별자
  - 최초 입력은 `input_path`로 받아도 되지만, 이후 STT/summary 호출은 `job_id` 기준이어야 한다.
- 중복 처리 여부 표시
  - 기존 결과 재사용 시 `reused: true` 같은 필드가 필요하다.
- 텍스트 파일 목록과 본문 조회 분리 여부
  - STT와 summary는 "파일 경로만 반환"보다 "본문까지 반환"하는 API가 별도로 있으면 클라이언트 구현이 단순해진다.

## 현재 Rust 코드가 제공하는 기능

### 1. 서버 ping

- 엔드포인트: `POST /server/ping`
- 기능:
  - JSON body를 검증한다.
  - 성공 시 고정 형식의 환영 메시지를 반환한다.
  - 잘못된 body면 400을 반환한다.

### 2. 오디오 변환 job 실행

- 입력 오디오 파일 경로를 받아 처리한다.
- 입력 경로를 canonical path로 정규화한다.
- 동일한 원본 파일로 완료된 job이 있고 산출물 파일이 남아 있으면 기존 결과를 재사용한다.
- 재사용 가능한 결과가 없으면 새 job 디렉터리를 만든다.
- job 시작 시 `db/index.json`에 `running` 상태를 기록한다.
- 처리 완료 시 `completed`, 실패 시 `failed`로 갱신한다.

### 3. 오디오 probe

- `ffprobe`로 첫 번째 오디오 스트림의 정보를 읽는다.
- 현재 추출하는 메타데이터:
  - `channels`
  - `channel_layout`

### 4. 오디오 분리/변환

- 입력 오디오에서 다음 산출물을 생성한다.
  - 채널별 mono WAV
  - 전체 채널을 합친 mono mix WAV
- 출력 규격은 고정이다.
  - codec: `pcm_s16le`
  - sample rate: `16000`
  - channels: `1`
- 비디오/자막/데이터 스트림은 제외한다.

### 5. job 인덱스 조회/재사용

- `db/index.json`에 다음을 저장한다.
  - `job_id`
  - `status`
  - `started_at`
  - `finished_at`
  - `source_path`
  - `source_file_name`
  - `job_dir`
  - `probe`
  - `split_strategy`
  - `outputs`
  - `error_message`
- 전체 job 목록 조회가 가능하다.
- 같은 `source_path`에 대한 최신 재사용 가능 완료 job 조회가 가능하다.

### 6. STT 실행

- 기존 job 디렉터리 안에서 지원되는 오디오 파일을 찾는다.
- 지원 확장자:
  - `wav`
  - `mp3`
  - `flac`
  - `ogg`
- 선택한 job의 오디오 파일들에 대해 `stt/*.txt`를 생성한다.
- Whisper 모델이 없으면 다운로드를 시도한다.
- 일부 플랫폼에서는 GPU/가속 백엔드 실패 시 CPU fallback을 시도한다.
- 관리 모델이 손상된 것으로 보이면 한 번 삭제 후 재다운로드를 시도한다.

### 7. 요약 생성

- 기존 job의 `stt/*.txt`를 모아 summary 입력으로 사용한다.
- 한국어 회의록 프롬프트를 생성한다.
- 결과를 `summary/*.md`에 저장한다.
- 이미 summary 파일이 있으면 재생성하지 않고 기존 파일을 반환한다.
- Llama 모델이 없으면 준비한다.
- 일부 플랫폼에서는 GPU/가속 백엔드 실패 시 CPU fallback을 시도한다.

### 8. Llama 모델 사전 준비

- 요약 실행 전에 Llama 모델을 미리 준비할 수 있다.
- 로컬 경로 모델과 Hugging Face 기반 모델 캐시를 모두 고려한다.

## API로 노출할 때 필요한 핵심 리소스

현재 코드 구조를 그대로 HTTP에 옮기려면 최소한 다음 리소스 개념이 필요하다.

### 1. Job

- 오디오 변환의 기본 단위
- `db/index.json`의 `JobRecord`가 사실상 원형이다.

권장 필드:

- `job_id`
- `status`
- `started_at`
- `finished_at`
- `source_path`
- `source_file_name`
- `job_dir`
- `probe`
- `split_strategy`
- `outputs`
- `error_message`
- `reused`

### 2. Probe

- `channels`
- `channel_layout`

### 3. Output File

- `kind`
  - `merged_mono_wav`
  - `split_mono_wav`
  - `transcript`
  - `summary`
- `path`
- `file_name`
- `channel_index`
- `download_url`

### 4. STT Result

- `job_id`
- `stt_dir`
- `transcripts`

### 5. Summary Result

- `job_id`
- `summary_dir`
- `summary_file`

### 6. Task Status

- `task_type`
  - `ffmpeg`
  - `stt`
  - `summary`
- `status`
  - `running`
  - `completed`
  - `failed`
- `message`
- `error_message`
- `reused`

### 7. Toolchain / Model Status

- `ffmpeg_available`
- `whisper_available`
- `llama_available`
- `whisper_model_ready`
- `llama_model_ready`

## API TODO 목록

아래 항목은 "현재 코드에 이미 있는 기능"을 API로 옮기기 위해 필요한 항목이다.

### A. 기본/공통 API

- `POST /server/ping`
  - 현재 구현 유지
- `GET /system/status`
  - 목적: 툴체인/모델 준비 상태 확인
  - 응답 예시 필드:
    - `ffmpeg_available`
    - `whisper_available`
    - `llama_available`
    - `whisper_model_ready`
    - `llama_model_ready`
    - `errors`

- `GET /jobs/{job_id}/status`
  - 목적: 현재 job의 ffmpeg/STT/summary 상태 확인
  - 응답 예시 필드:
    - `job_id`
    - `ffmpeg_status`
    - `stt_status`
    - `summary_status`
    - `error_message`

### B. 변환 job API

- `POST /jobs`
  - 목적: 오디오 변환 job 생성 또는 기존 결과 재사용
  - 요청 필드:
    - `input_path`
  - 응답 필드:
    - `job_id`
    - `status`
    - `message`
    - `reused`
    - `started_at`
    - `finished_at`
    - `source_path`
    - `source_file_name`
    - `probe`
    - `outputs`
    - `error_message`
  - 권장 동작:
    - 새 작업이면 `작업 시작중` 메시지 반환
    - 재사용이면 기존 완료 산출물 경로 반환

- `GET /jobs`
  - 목적: 전체 job 목록 조회
  - 응답 필드:
    - `jobs[]`

- `GET /jobs/{job_id}`
  - 목적: 개별 job 상세 조회
  - 응답 필드:
    - `job_id`
    - `status`
    - `started_at`
    - `finished_at`
    - `source_path`
    - `source_file_name`
    - `job_dir`
    - `probe`
    - `split_strategy`
    - `outputs`
    - `error_message`

- `GET /jobs/by-source`
  - 목적: 동일 원본 파일 기준 재사용 가능한 완료 job 조회
  - 쿼리 파라미터:
    - `source_path`
  - 응답 필드:
    - `job`

- `GET /jobs/completed`
  - 목적: ffmpeg 작업이 완료된 오디오 파일 목록 반환
  - 응답 필드:
    - `jobs[]`
  - 각 항목 권장 필드:
    - `job_id`
    - `source_path`
    - `source_file_name`
    - `outputs`

### C. 산출물 파일 API

- `GET /jobs/{job_id}/files`
  - 목적: 해당 job이 보유한 파일 목록 조회
  - 응답 필드:
    - `files[]`

- `GET /jobs/{job_id}/files/{file_name}`
  - 목적: 실제 산출물 다운로드
  - 대상 예시:
    - `mono_mix.wav`
    - `channel_01.wav`
    - `channel_02.wav`
    - `stt/<name>.txt`
    - `summary/<name>.md`

참고:

- 현재 코드는 파일 시스템 경로만 저장하고 있다.
- 외부 API로 쓰려면 다운로드 응답 또는 signed URL 같은 접근 방식이 추가로 필요하다.

### D. STT API

- `POST /jobs/{job_id}/stt`
  - 목적: 특정 job 폴더의 오디오 파일들에 대해 STT 실행
  - 요청 필드:
    - `audio_files` (선택)
    - `mono_mix_only` (선택, 기본값 `false`)
  - 기본 동작:
    - 지정이 없으면 job 디렉터리의 지원 오디오 파일 전체 처리
    - `mono_mix_only=true`면 `mono_mix.wav`만 처리
    - `mono_mix_only=true`와 `audio_files` 동시 지정은 허용하지 않음
  - 응답 필드:
    - `job_id`
    - `status`
    - `message`
    - `stt_dir`
    - `transcripts[]`
  - 권장 동작:
    - 처리 시작 시 `작업 시작중` 메시지 반환
    - 이미 완료된 transcript가 있으면 상태와 함께 기존 결과 반환 가능

- `GET /jobs/{job_id}/stt`
  - 목적: 기존 transcript 목록 조회
  - 응답 필드:
    - `job_id`
    - `stt_dir`
    - `transcripts[]`

- `GET /jobs/{job_id}/stt/texts`
  - 목적: STT 완료 항목의 텍스트 본문 반환
  - 응답 필드:
    - `job_id`
    - `texts[]`
  - 각 항목 권장 필드:
    - `transcript_id`
    - `source_path`
    - `text_path`
    - `content`

- `GET /jobs/{job_id}/stt/texts/{transcript_id}`
  - 목적: 특정 transcript 본문 반환
  - 응답 필드:
    - `job_id`
    - `transcript_id`
    - `source_path`
    - `content`

- `POST /models/whisper/prepare`
  - 목적: Whisper 모델만 미리 다운로드/준비
  - 응답 필드:
    - `model_path`
    - `ready`
    - `message`

### E. Summary API

- `POST /jobs/{job_id}/summary`
  - 목적: 특정 job의 transcript를 기반으로 요약 생성
  - 요청 필드:
    - 없음 또는 `force_regenerate`
  - 응답 필드:
    - `job_id`
    - `status`
    - `message`
    - `summary_dir`
    - `summary_file`
    - `reused`
  - 권장 동작:
    - 처리 시작 시 `작업 시작중` 메시지 반환
    - 이미 완료된 summary가 있으면 기존 결과 반환 가능

- `GET /jobs/{job_id}/summary`
  - 목적: 기존 summary 파일 상태 조회
  - 응답 필드:
    - `job_id`
    - `summary_dir`
    - `summary_file`
    - `exists`

- `GET /jobs/{job_id}/summary/text`
  - 목적: summary 완료 항목의 텍스트 본문 반환
  - 응답 필드:
    - `job_id`
    - `summary_file`
    - `content`

- `POST /models/llama/prepare`
  - 목적: Llama 모델만 미리 다운로드/준비
  - 응답 필드:
    - `model_source`
    - `cached_model_path`
    - `ready`
    - `message`

## API 설계 시 반드시 반영해야 할 현재 제약

### 1. 현재 STT/summary는 "선택형 CLI" 구조다

- 현재 구현은 job 후보 목록을 보여주고 사용자가 번호를 고르는 방식이다.
- API에서는 이 인터랙션을 그대로 유지할 수 없다.
- 따라서 다음이 필요하다.
  - `job_id` 기반 직접 실행
  - 또는 후보 조회 API와 선택 API 분리

### 2. 현재 서버는 비동기 job 큐가 없다

- 변환, STT, 요약은 모두 실행 함수가 끝날 때까지 동기적으로 처리된다.
- API로 옮길 때 선택지가 두 가지다.
  - 우선은 동기 응답으로 그대로 노출
  - 이후 백그라운드 job 모델로 확장

현재 코드 기준으로는 "동기 API"가 구현 난이도가 가장 낮다.

다만 사용자가 기대하는 "작업 시작중 메시지 반환" UX는 비동기 작업 모델과 더 잘 맞는다.
따라서 다음 중 하나를 먼저 결정해야 한다.

- 동기 API
  - 요청이 끝날 때 결과까지 같이 반환
- 비동기 API
  - 시작 메시지를 즉시 반환
  - 이후 `status` 조회 API로 완료 여부 확인

### 3. 현재 입력은 파일 경로 기반이다

- 변환 시작 입력은 `input_path`다.
- 즉, 외부 사용자가 직접 파일 업로드를 보내는 API는 아직 구현 기반이 없다.
- 따라서 1차 API 범위는 다음 중 하나로 정해야 한다.
  - 서버가 접근 가능한 로컬 경로만 받기
  - 별도 업로드 API를 추가해 파일을 저장한 뒤 그 경로를 내부적으로 연결하기

### 4. 현재 파일 제공 방식은 경로 저장만 한다

- `db/index.json`에는 파일 경로만 저장된다.
- API 사용성을 위해서는 최소 하나가 필요하다.
  - 파일 다운로드 엔드포인트
  - 정적 파일 서빙
  - 외부 스토리지 URL 매핑

### 5. 현재 toolchain 빌드는 API 기능이 아니다

- Rust 코드는 FFmpeg, Whisper, Llama 툴체인이 없으면 에러를 반환한다.
- 즉, "빌드/설치"는 현재 런타임 API가 아니라 운영 준비 단계다.
- 따라서 1차 API 범위에서는 다음이 더 현실적이다.
  - build API는 제외
  - 대신 status API에서 미설치 상태를 명확히 반환

## 우선순위 제안

### 1차 구현

- `POST /server/ping`
- `GET /system/status`
- `POST /jobs`
- `GET /jobs`
- `GET /jobs/{job_id}`
- `GET /jobs/{job_id}/status`
- `GET /jobs/completed`
- `GET /jobs/{job_id}/files`
- `GET /jobs/{job_id}/files/{file_name}`
- `POST /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt`
- `GET /jobs/{job_id}/stt/texts`
- `GET /jobs/{job_id}/stt/texts/{transcript_id}`
- `POST /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/summary`
- `GET /jobs/{job_id}/summary/text`
- `POST /models/llama/prepare`

### 2차 구현

- `POST /models/whisper/prepare`
- `GET /jobs/by-source`
- summary 재생성 옵션
- transcript 개별 조회/다운로드 API
- 비동기 작업 큐
- 작업 단계별 세분화 상태 모델

## 사용자 시나리오 기준 최종 체크리스트

아래 항목이 있으면 사용자가 제안한 흐름을 실제 API로 끊김 없이 사용할 수 있다.

### 필수

- 오디오 파일 경로로 ffmpeg 작업 시작 요청
- 기존 완료 job 재사용 시 즉시 완료 결과 반환
- ffmpeg 완료 목록 조회
- 특정 job의 STT 시작 요청
- STT 결과 목록 조회
- STT 결과 본문 조회
- 특정 job의 summary 시작 요청
- summary 결과 본문 조회
- 각 단계의 상태 조회
- 각 단계의 실패 메시지 조회

### 있으면 좋은 항목

- transcript 단건 조회
- summary 재생성 옵션
- Whisper 모델 준비 API
- 파일 다운로드 API
- 시스템 상태 API

## 결론

현재 코드가 실제로 제공하는 기능은 다음 5개 축으로 정리된다.

1. ping
2. 오디오 변환 및 산출물 캐시 재사용
3. STT
4. 요약 생성
5. job/모델 상태 관리

이 기능을 전부 API로 제공하려면 단순히 새 라우트 몇 개를 추가하는 수준이 아니라, 다음 세 가지를 함께 정리해야 한다.

1. `JobRecord` 중심의 HTTP 응답 스키마
2. `job_id` 기반 실행 방식으로의 인터랙션 전환
3. 파일 경로를 실제 API 소비 가능한 다운로드 방식으로 노출하는 계층
