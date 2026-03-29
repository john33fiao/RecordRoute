# KESE 1차 증거 인벤토리

이 문서는 현재 저장소만 기준으로 KESE 4종 산출물에 사용할 근거를 정리합니다.
이번 1차 범위는 코드, 문서, 설정 예시, 실행 스크립트에 한정합니다.

## 범위와 한계

- 포함:
  - `rust/`
  - `docs/`
  - `.env.example`
  - `.gitignore`
  - `run.sh`
- 제외:
  - 실제 운영 인프라
  - 실제 `.env` 값의 내용
  - 네트워크 장비/보안 장비/물리 보안
  - 조직 절차와 운영 정책 문서

## 주요 근거 파일

| 범주 | 근거 | 관찰 내용 |
|---|---|---|
| API 라우팅 | `rust/src/server.rs:44-115` | 라우터가 주요 API를 직접 등록하며 인증/인가 또는 CORS 계층은 코드상 보이지 않습니다. |
| 업로드 제어 | `rust/src/server/upload.rs:15-20`, `rust/src/server/upload.rs:79-107`, `rust/src/server/upload.rs:149-178` | 업로드 파일 최대 크기는 512MB이며, multipart의 `file` 필드는 1회만 허용하고 빈 파일은 거부합니다. |
| 업로드 임시 파일 처리 | `rust/src/server/upload.rs:50-59`, `rust/src/server/upload.rs:135-147`, `rust/src/server/upload.rs:196-218` | 업로드는 spool의 임시 파일로 받았다가 publish 후 cleanup 합니다. |
| 파일 다운로드 경계 | `rust/src/server/files.rs:109-144`, `rust/src/server/files.rs:161-176` | `..`, 절대경로, 역슬래시를 차단해 파일명 경로 조작을 방어합니다. |
| 서버 바인딩 | `rust/src/server.rs:38`, `rust/src/server.rs:127-134` | 서버는 기본적으로 `127.0.0.1:38080`에 바인딩되며 앱 내부 TLS는 없습니다. |
| 런타임 저장 구조 | `docs/architecture.md:48-78`, `docs/architecture.md:103-129` | 메타DB와 오디오 저장소가 분리되어 있고 절대 경로를 영속 메타데이터에 저장하지 않습니다. |
| 환경 변수 로드 | `rust/src/app.rs:245-253` | 런타임 루트의 `.env`를 읽습니다. |
| 환경 변수 예시 | `.env.example:8-44` | sqlite/postgres, 오디오 경로, 모델 경로, `HF_TOKEN` 같은 외부 설정 항목이 정의돼 있습니다. |
| 비밀 파일 제외 | `.gitignore:7-8` | `.env`는 Git 추적에서 제외됩니다. |
| 외부 도구 경계 | `docs/API_Audit.md:15-27`, `docs/API_Audit.md:42-45`, `docs/API_Audit.md:118-119`, `docs/API_Audit.md:245` | ffmpeg/whisper/llama는 CLI 래핑이며 일부 성공 판정은 종료 코드나 stdout 포맷에 의존합니다. |

## 관찰된 사실

### 1. 인증/인가

- API 라우터에는 auth middleware, session, token 검증 계층이 보이지 않습니다.
- `/jobs/upload`, `/jobs/{job_id}/files/{*file_name}`, `/models/*/prepare`, `/dictionary/keywords*` 같은 민감 엔드포인트가 동일한 방식으로 노출됩니다.

### 2. 파일 업로드/다운로드

- 업로드는 최대 512MB 제한이 있습니다.
- multipart에서 `file` 필드는 정확히 1개만 허용됩니다.
- 빈 업로드는 거부됩니다.
- 다운로드는 파일명 sanitize를 통해 경로 역참조를 차단합니다.

### 3. 저장소/DB 경계

- 기본 메타데이터 저장소는 sqlite이고 postgres backend도 지원합니다.
- 오디오 저장소는 content-addressed 구조를 사용합니다.
- 절대 경로 대신 logical file name과 storage key를 저장합니다.

### 4. 비밀/설정

- 설정은 `.env`에서 읽고 `.env`는 Git에서 제외됩니다.
- 예시 설정은 실제 비밀값을 포함하지 않습니다.
- `HF_TOKEN` 같은 비밀은 외부화 전제를 가집니다.

### 5. 전송 보안

- 앱 내부 TLS 구현은 보이지 않습니다.
- 기본 서버 바인드는 loopback입니다.
- 외부 공개 배포를 전제로 한 HTTPS 종단 방식은 저장소에서 확인되지 않습니다.

### 6. 로깅/모니터링

- 아키텍처 문서와 런처 구현을 보면 `logs/` 디렉터리와 서버 로그 파일 사용이 전제됩니다.
- 다만 보안 이벤트 수준의 감사 로깅 정책은 저장소에서 확인되지 않습니다.

### 7. 외부 CLI 호출

- ffmpeg/whisper/llama 호출은 Rust 앱 내부에서 래핑됩니다.
- 일부 단계는 종료 코드 중심 성공 판정 또는 stdout 후처리에 의존합니다.

## 1차 판정에 쓰는 기본 규칙

- 저장소에서 직접 관찰 가능한 사실만 `양호`, `부분이행`, `취약`으로 판정합니다.
- 운영 인프라, 물리 보안, 조직 절차처럼 저장소 근거가 없는 항목은 `해당없음`으로 둡니다.
- `취약` 또는 `부분이행` 항목은 반드시 위 표의 근거 파일로 역추적 가능해야 합니다.
