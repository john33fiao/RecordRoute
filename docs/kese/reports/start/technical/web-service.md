# KESE 1차 기술 평가 - Web/API 서비스

## 판정 요약

| 항목 | 판정 | 근거 |
|---|---|---|
| API 인증/인가 | 취약 | `rust/src/server.rs:44-115` |
| 업로드 제한/입력 검증 | 양호 | `rust/src/server/upload.rs:15-20`, `rust/src/server/upload.rs:79-107`, `rust/src/server/upload.rs:149-178` |
| 다운로드 경로 검증 | 양호 | `rust/src/server/files.rs:109-144`, `rust/src/server/files.rs:161-176` |
| TLS/HTTPS | 부분이행 | `rust/src/server.rs:38`, `rust/src/server.rs:127-134` |
| CORS 정책 | 부분이행 | `rust/src/server.rs:44-115`, 별도 CORS middleware 부재 |
| 에러/실패 처리 | 부분이행 | `rust/src/server.rs:137-160`, `docs/API_Audit.md:111-119`, `docs/API_Audit.md:229-245` |

## 상세 관찰

### 1. 인증/인가

- 라우터는 `/jobs/upload`, `/jobs/{job_id}/files/{*file_name}`, `/models/*/prepare`, `/dictionary/keywords*`, `/summary/search` 등을 직접 노출합니다.
- 코드상 auth middleware, token/session 검증, role check는 보이지 않습니다.
- 따라서 로컬 바인딩을 벗어난 배포에서는 중요한 취약점입니다.

### 2. 업로드 처리

- 업로드 최대 크기는 512MB입니다.
- multipart의 `file` 필드는 한 번만 허용됩니다.
- 빈 파일 업로드는 거부됩니다.
- 임시 파일은 spool에 저장했다가 publish 후 cleanup 합니다.

### 3. 다운로드 처리

- 다운로드는 logical file 이름만 허용합니다.
- `..`, 절대경로, 역슬래시는 거부합니다.
- summary/stt/audio artifact를 명시적으로 분기해 읽습니다.

### 4. TLS/HTTPS와 바인딩

- 기본 바인드는 `127.0.0.1:38080`입니다.
- 앱 내부 TLS는 없습니다.
- 즉 기본 개발 모드는 로컬 한정이라는 점은 긍정적이지만, 외부 배포 시 reverse proxy 또는 별도 TLS 종단 설계가 필요합니다.

### 5. CORS와 브라우저 경계

- 저장소에서 명시적 CORS 허용 정책은 보이지 않습니다.
- 현재 UI는 동일 서버가 `/`, `/app.js`, `/app.css`를 직접 제공하므로 same-origin 전제에서는 즉시 취약으로 보지는 않습니다.
- 다만 별도 프런트엔드 호스팅 또는 외부 API 사용을 계획하면 정책 명시가 필요합니다.

## 1차 권고

1. API 인증/인가 계층을 우선 설계합니다.
2. 외부 배포가 있다면 TLS 종단 구조와 허용 origin 정책을 문서화합니다.
3. ffmpeg/llama 단계의 결과 검증을 종료 코드 외 파일/형식 기준으로 보강합니다.
