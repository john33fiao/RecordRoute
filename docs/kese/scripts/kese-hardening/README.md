# KESE Hardening Draft Pack

이 디렉터리는 RecordRoute 저장소를 기준으로 생성한 1차 하드닝 초안 산출물입니다.

## 상태

- 초안
- 미적용
- 코드/설정 자동 변경 없음

## 생성된 산출물

| 파일 | 이유 |
|---|---|
| `database/postgresql-hardening.sql` | 저장소가 PostgreSQL backend 지원을 명시하고 있어 generic baseline 초안을 만들 수 있음 |

## 미생성 산출물

| 영역 | 미생성 사유 |
|---|---|
| Unix/Linux 스크립트 | 실제 운영 OS, 계정 정책, SSH/PAM 설정 근거가 없어 `해당없음` |
| Windows 스크립트 | 실제 운영 Windows 환경 근거가 없어 `해당없음` |
| 웹 서버(Apache/Nginx/Tomcat) 설정 | 현재 저장소는 axum 앱 기준이며 외부 reverse proxy 구성이 드러나지 않아 `해당없음` |
| 물리/관리 영역 산출물 | 저장소 근거 없음으로 `해당없음` |

## 적용 전에 확인할 사항

1. 실제 운영 DB 엔진이 PostgreSQL인지 확인
2. 실제 role 이름, database 이름, schema 전략 확인
3. SSL/TLS, `pg_hba.conf`, `postgresql.conf`는 SQL만으로 완결되지 않으므로 운영 설정과 함께 검토
4. SQL은 staging에서 먼저 검증

## 이 초안이 다루는 문제

- PUBLIC 권한 축소
- 애플리케이션 role 최소 권한화
- 기본 권한 정리
- search_path 및 statement timeout 같은 기본 안전 설정

## 직접 다루지 않는 문제

- 앱 코드 수준 인증/인가
- reverse proxy TLS 설정
- 보안 이벤트 감사 로깅 체계
- sqlite 파일 권한 정책

## 근거 문서

- `../../reports/check/deployment-readiness.md`
- `../../reports/start/technical/database.md`
