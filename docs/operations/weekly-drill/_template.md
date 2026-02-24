# YYYY-MM-DD 주간 운영 점검 결과

## 점검 개요

- 점검 일시: YYYY-MM-DD HH:MM~HH:MM KST
- 점검 버전(커밋): `<commit-hash>`
- 운영 담당(Operator): `<이름/팀>`
- 리뷰어(Reviewer): `<이름/직책>`
- 수행 시나리오: A / B / C (해당 항목 표시)

## 사전 점검

- [ ] 배포 버전/커밋 해시 기록 완료
- [ ] 필수 환경변수 확인 완료
- [ ] 엔진 포트(18101/18102/18103) 라우팅 확인 완료
- [ ] 점검 시작 시점 `/metrics` 스냅샷 저장 완료

## 시나리오 A — 장애 주입 (해당 시 작성)

- 대상 엔진: stt / summarize / embed
- 장애 주입 방법:
- 관찰 요약:
- `/readyz` raw 응답: `HTTP/1.1 <code>` / `<body>`
- `/metrics` 스냅샷 (발췌):
- `POST /jobs` 샘플 요청/응답:
- `GET /jobs/{id}` 샘플 결과:
- ready 복귀 소요 시간: _분 _초
- **판정: PASS / FAIL**
- 판정 근거:

## 시나리오 B — 복구 검증 (해당 시 작성)

- 복구 대상/방법:
- 관찰 요약:
- `/readyz` raw 응답: `HTTP/1.1 <code>` / `<body>`
- `/metrics` 스냅샷 (발췌):
- `POST /jobs` 샘플 요청/응답:
- `GET /jobs/{id}` 샘플 결과:
- stuck job 해소 소요 시간: _분
- 미처리 stuck job 잔여: _건
- **판정: PASS / FAIL**
- 판정 근거:

## 시나리오 C — 부하/배압 (해당 시 작성)

- 부하 방법/규모:
- 관찰 요약:
- `/readyz` raw 응답: `HTTP/1.1 <code>` / `<body>`
- `/metrics` 스냅샷 (발췌):
- `POST /jobs` 샘플 요청/응답:
- `GET /jobs/{id}` 샘플 결과:
- rejection 분포(건수): queue_full=`<n>`, engine_full=`<n>`
- ready 복귀 소요 시간: _분 _초
- **판정: PASS / FAIL**
- 판정 근거:

## 종합

- 실패/이슈:
- 즉시 조치:
- 롤백 여부(실시/미실시 + 근거):
- 롤백 실행 책임자(Owner):
- 롤백 승인자(Approver):
- 보안 영향 검토(있음/없음 + 근거):
- 재실행 예정일:

## 후속 액션

| # | 액션 | 담당 | 기한 | 상태 |
|---|------|------|------|------|
| 1 | | | | |

## 리뷰어 승인

- 리뷰어: `<이름>`
- 승인 일시: YYYY-MM-DD
- 비고:
