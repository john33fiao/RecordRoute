# KESE 배포 준비 보고서

대상: RecordRoute 저장소  
평가 기준: 코드/문서/설정 예시만 사용  
평가 일시: 저장소 기준 1차 정적 평가

## 최종 판정

- 배포 결정: 조건부 차단

이번 평가는 실제 운영 인프라 없이 저장소만으로 수행했기 때문에 많은 운영/물리 항목은 `해당없음`입니다.  
그럼에도 현재 코드에서 직접 확인되는 고위험 이슈가 있어 외부 공개 배포 관점에서는 즉시 승인하기 어렵습니다.

## 긴급 이슈

### 1. API 인증/인가 부재

- 상태: 차단
- 근거: `rust/src/server.rs:44-115`
- 설명:
  - 주요 API와 파일 다운로드, 모델 준비, dictionary 변경 엔드포인트가 동일한 라우터에 등록돼 있습니다.
  - 인증/인가 middleware 또는 endpoint guard는 코드상 확인되지 않습니다.
- 영향:
  - 외부 노출 시 무단 사용, 데이터 조회, 운영 기능 호출 위험이 있습니다.

## 높음 이슈

### 2. 앱 내부 TLS 부재 및 외부 HTTPS 종단 불명확

- 상태: 높음
- 근거: `rust/src/server.rs:38`, `rust/src/server.rs:127-134`
- 설명:
  - 기본 바인드는 loopback이라 개발 안전성은 있으나, 앱 자체 HTTPS는 제공하지 않습니다.
  - 외부 배포 시 reverse proxy/TLS termination 설계가 별도 문서로 확인되지 않습니다.

### 3. 보안 감사 로깅 정책 부재

- 상태: 높음
- 근거: `docs/architecture.md:15`, 저장소 내 별도 보안 감사 정책 문서 부재
- 설명:
  - 로그 디렉터리와 서버 로그 개념은 보이지만, 민감 API 호출, 관리자 동작, 다운로드/업로드 행위에 대한 감사 로깅 정책은 확인되지 않습니다.

## 중간 이슈

### 4. 외부 CLI 결과 판정의 민감도

- 상태: 중간
- 근거: `docs/API_Audit.md:118-119`, `docs/API_Audit.md:229-245`
- 설명:
  - ffmpeg는 종료 코드 중심이고 산출 파일 존재를 모두 재검증하지 않습니다.
  - summary는 stdout 포맷 변화에 민감합니다.

### 5. CORS 정책 명시 부재

- 상태: 중간
- 근거: `rust/src/server.rs:44-115`
- 설명:
  - 현재 same-origin 구조에서는 즉시 취약으로 보지 않지만, 외부 프런트엔드 분리 시 명시적 정책이 필요합니다.

### 6. PostgreSQL 운영 hardening baseline 부재

- 상태: 중간
- 근거: `.env.example:10-12`, `docs/architecture.md:48-67`
- 설명:
  - postgres 지원은 명시돼 있으나 운영 권한/SSL/감사 기준은 저장소에서 확인되지 않습니다.

## 양호 항목

1. 업로드 크기 제한과 빈 파일 거부가 있습니다.
2. multipart `file` 필드는 단일 허용입니다.
3. 다운로드 경로 sanitize가 구현돼 있습니다.
4. `.env` 외부화와 `.gitignore` 제외가 적용돼 있습니다.
5. 메타데이터에 절대 경로를 저장하지 않습니다.

## 해당없음 항목

- Unix/Linux 운영 하드닝
- Windows 서버 하드닝
- 네트워크 장비/보안 장비
- 물리 보안
- 조직 정책과 승인 절차

## 배포 전 선행 조건

1. API 인증/인가 계층 설계 및 적용
2. 외부 배포 시 TLS 종단 구조 문서화
3. 보안 감사 로깅 기준 정의
4. PostgreSQL 운영 baseline 수립

## 관련 문서

- `../evidence-inventory.md`
- `../start/summary.md`
- `../start/technical/web-service.md`
- `../start/technical/database.md`
