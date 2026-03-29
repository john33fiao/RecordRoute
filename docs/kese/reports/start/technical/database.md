# KESE 1차 기술 평가 - 데이터베이스

## 판정 요약

| 항목 | 판정 | 근거 |
|---|---|---|
| DB 구성 외부화 | 양호 | `.env.example:8-18`, `rust/src/app.rs:245-253` |
| sqlite/postgres 지원 구조 | 양호 | `.env.example:10-12`, `docs/architecture.md:48-67` |
| 절대 경로 비노출 저장 | 양호 | `docs/architecture.md:103-129` |
| PostgreSQL 하드닝 기준 | 부분이행 | postgres 지원은 있으나 보안 baseline 문서 부재 |
| DB 감사 정책 | 부분이행 | 저장소 수준에서 DB 감사/감사로그 정책 근거 부재 |

## 상세 관찰

### 1. 저장소 설정

- 기본 메타데이터 저장소는 sqlite입니다.
- 환경 변수로 postgres backend 전환이 가능합니다.
- 오디오 저장소는 DB와 분리되어 있습니다.

### 2. 비밀 관리

- postgres URL은 `.env`에서 주입하는 구조입니다.
- `.env`는 Git 추적에서 제외됩니다.
- 예시 파일에 실제 비밀값은 없습니다.

### 3. 데이터 경계

- 메타데이터에는 절대 경로 대신 logical file name과 storage key를 저장합니다.
- 이는 내부 파일 시스템 레이아웃 노출을 줄이는 방향입니다.

### 4. 남은 과제

- PostgreSQL을 실제 운영에서 사용할 경우 역할 분리, PUBLIC 권한 제거, SSL 강제, SCRAM, 감사 로깅 같은 baseline이 별도로 필요합니다.
- sqlite 사용 시 파일 권한, 백업 보호, 런타임 디렉터리 권한 정책은 저장소에서 확인되지 않습니다.

## 1차 권고

1. PostgreSQL용 baseline hardening SQL과 운영 체크리스트를 분리합니다.
2. sqlite 운용 시 파일 권한과 백업 접근 제어 기준을 별도 문서화합니다.
