# 운영 점검 시나리오 Runbook (장애/복구/부하)

이 문서는 Rust 오케스트레이터 기준 운영 점검 절차를 정의합니다.
목표는 **장애 감지 → 영향 축소 → 복구 확인 → 재발 방지 기록**을 표준화하는 것입니다.

## 0. 범위와 전제

- 대상 서비스: `recordroute-orchestrator` (`:18000`), Swagger 정적 서버(`:14000`)
- 핵심 엔드포인트: `/healthz`, `/readyz`, `/metrics`, `POST /jobs`, `GET /jobs/{job_id}`
- 엔진 구성: `stt`, `summarize`, `embed` (엔진별 bounded queue + worker)
- 상태 전이: `queued|running|completed|failed|timeout|canceled|rejected`
- 리젝션 규약: `429(queue_full|engine_full)` + 리젝션 메트릭(`engine`, `reason`)

### 0-1. 인증/인가/비밀관리 참조

- 저장소 내부 참조(현재 확인 가능 문서)
  - 인증/검증 책임 경계: `docs/architecture.md` (Rust 오케스트레이터의 요청 인증/검증 책임)
  - 배포/운영 분리 원칙: `docs/deployment-asset-policy.md`
- 저장소에는 인증/인가/비밀관리 전용 정책 문서가 분리되어 있지 않습니다.
- 운영 환경 기준 문서 위치(시스템/경로)
  - 시스템명: **Kubernetes (cluster runtime)**
  - 비밀값 원천 경로: `k8s namespace(recordroute)/Secret/*` (환경별 Secret 리소스)
  - 주입 지점 경로: `Deployment.spec.template.spec.containers[].env[].valueFrom.secretKeyRef`

> 주의: 본 문서는 운영 절차(runbook) 가이드입니다. 인증/인가/비밀관리의 최종 판정은 위 보안 기준(저장소 참조 + 운영 환경 시스템 경로)을 우선합니다.

## 1. 공통 사전 점검

1. 배포 버전/커밋 해시 기록
2. 필수 환경변수 확인
   - `RECORDROUTE_API_HOST`, `RECORDROUTE_API_PORT`
   - `RECORDROUTE_*_QUEUE_CAPACITY`, `RECORDROUTE_*_CONCURRENCY`
   - `RECORDROUTE_ENGINE_TIMEOUT_SECS`, `RECORDROUTE_JOB_TIMEOUT_*`
3. 엔진 포트 라우팅 확인 (`18101`, `18102`, `18103`)
4. 점검 시작 시점 기준 메트릭 스냅샷 저장 (`/metrics` 응답 원본)

샘플 점검 명령:

```bash
curl -sS http://127.0.0.1:18000/healthz
curl -sS -i http://127.0.0.1:18000/readyz
curl -sS http://127.0.0.1:18000/metrics | jq .
```

## 2. 시나리오 A — 엔진 장애(프로세스 다운/응답 불가)

### A-1. 목적
- 단일 엔진 장애 시 전체 서비스가 아닌 해당 엔진 경로 중심으로 영향이 제한되는지 확인
- readiness가 `degraded`로 전이되는지 확인

### A-2. 절차
1. 점검 대상 엔진 1개(`stt` 권장) 선택
2. 해당 엔진 프로세스 강제 종료 또는 포트 차단으로 장애 주입
3. 즉시 `/readyz` 상태 코드/바디 확인 (`503 degraded` 기대)
4. `POST /jobs?engine=stt` 요청과 `POST /jobs?engine=summarize` 요청을 각각 수행
5. `/metrics`에서 readiness 및 엔진별 queue/running/rejections 변화 확인

### A-3. 기대 결과
- `/healthz`는 프로세스 생존 기준으로 `200` 유지 가능
- `/readyz`는 `503` + `degraded` 표기
- 장애 엔진은 실패/타임아웃/리젝션이 증가하되, 타 엔진은 접수 가능 상태 유지

### A-4. 복구 판정
- 엔진 복구 후 `/readyz`가 `200 ready`로 회복
- 신규 job이 정상적으로 `queued → running → completed` 전이
- 리젝션 증가 추세가 정상 구간으로 복귀

## 3. 시나리오 B — 큐 포화/부하(백프레셔 검증)

### B-1. 목적
- 과부하 시 무제한 적재가 아닌 bounded queue + `429` 리젝션이 동작하는지 확인
- `queue_full` vs `engine_full` reason 분리가 관측 가능한지 확인

### B-2. 절차
1. 테스트 시간 동안 특정 엔진으로 burst 트래픽 송신
2. `POST /jobs?engine=<target>`의 응답 코드/에러 바디 수집
3. `/metrics`에서 `rejections(engine, reason)` 스냅샷 수집
4. 부하 중단 후 queue_depth/running이 정상 수렴하는지 관찰

### B-3. 기대 결과
- 포화 구간에서 `429`가 발생하며 reason이 `queue_full|engine_full`로 구분
- 타 엔진 큐는 독립적으로 처리되어 연쇄 포화가 제한됨
- 부하 종료 후 큐 깊이 및 실행 수치가 정상치로 복귀

## 4. 시나리오 C — 타임아웃/장시간 작업

### C-1. 목적
- HTTP 타임아웃과 Job 타임아웃 계층이 혼선 없이 분리되는지 확인
- STT `audio_ms` 기반 timeout budget 산정식이 과도/과소하지 않은지 점검

### C-2. 절차
1. 짧은 입력/긴 입력 샘플 각각으로 STT job 실행
2. `GET /jobs/{job_id}` 폴링으로 최종 상태와 error code 수집
3. 타임아웃 발생 시 `timeout` 상태와 메시지 확인
4. 관련 환경변수(`RECORDROUTE_JOB_TIMEOUT_*`, `RECORDROUTE_STT_TIMEOUT_*`) 기록

### C-3. 기대 결과
- 정상 길이 입력은 완료, 과도 길이/지연은 timeout으로 수렴
- timeout이 발생해도 worker/queue가 누수 없이 다음 job 처리 가능

## 5. 장애 대응 체크리스트 (운영자용)

- [ ] 영향 범위(엔진/기능/API) 1차 분류
- [ ] `/readyz`, `/metrics`, 최근 에러 로그 수집
- [ ] 리젝션 reason 분포(`queue_full`, `engine_full`) 확인
- [ ] 최근 설정 변경/배포 이력 대조
- [ ] 임시 완화(트래픽 제한, 특정 엔진 격리, 재시작) 수행
- [ ] 복구 검증(ready 복귀 + 주요 사용자 시나리오 성공)
- [ ] 사후 보고(원인/영향/재발방지/롤백 포인트) 기록

## 6. 롤백/완화 기준

> RTD 주석: 이 섹션의 체크 항목은 **RTD Step 7(보안/크리티컬 이슈)** 및 **RTD Step 17(배포 준비도 판정)** 에 직접 사용됩니다.

- 즉시 롤백 조건
  - `/readyz`가 `503 degraded` 상태로 **연속 10분 초과** 지속
  - `rejections_total(engine, reason)` 5분 합계가 최근 1시간 중앙값 대비 **3배 이상**으로 **2개 연속 구간** 급증
  - 부하 중단 후에도 `queue_depth`가 엔진별 최대 큐 깊이의 **30% 이상으로 10분 내 미수렴**
  - 인증/인가/비밀관리 이상(권한 오판정, 비밀 노출 의심) 1건 이상 확인 시 즉시 롤백 또는 서비스 격리
- 완화 우선순위
  1. 트래픽 셰이핑/임시 rate limit
  2. 문제 엔진만 격리(타 엔진 서비스 유지)
  3. 직전 안정 버전으로 롤백

### 6-1. 롤백 실행 책임 체계 (필수 필드)

- 롤백 실행 책임자(Owner): `<이름/팀>`
- 롤백 승인자(Approver): `<이름/직책>`
- 실행/승인 시각: `<YYYY-MM-DD HH:MM TZ>`

## 7. 점검 결과 템플릿

### 7-1. 저장 규칙 (필수)

- 정례 점검 결과 문서는 아래 경로에 **고정 저장**합니다.
  - `docs/operations/weekly-drill/<YYYY-MM-DD>.md`
  - 예: `docs/operations/weekly-drill/2026-03-02.md`
- 정례 점검 정책(주기/역할/합격 기준)은 `docs/operations/weekly-drill/README.md`를 참조합니다.
- 결과 템플릿은 `docs/operations/weekly-drill/_template.md`를 복사해 사용합니다.
- 점검 결과 본문에는 아래 **필수 첨부 항목**을 반드시 포함합니다.
  1. `/readyz` raw 응답(상태 코드 + 바디 원문)
  2. `/metrics` 스냅샷(점검 시점 원문 또는 JSON 발췌)
  3. 대표 API 샘플 1쌍: `POST /jobs` 요청/응답 + 대응 `GET /jobs/{id}` 결과

### 7-2. 작성 필드 (필수)

- 아래 필드는 모든 시나리오에서 필수입니다.
  - `즉시 조치`
  - `롤백 여부(실시/미실시 + 근거)`
  - `롤백 실행 책임자(Owner)`
  - `롤백 승인자(Approver)`
  - `보안 영향 검토(있음/없음 + 근거)`
  - `재실행 예정일`
- 시나리오 B(부하/포화)에서는 아래 필드를 추가로 필수 기록합니다.
  - `rejection 분포(건수): queue_full=<n>, engine_full=<n>`
  - 필요 시 비율 병기: `queue_full=<n>(<p>%), engine_full=<n>(<p>%)`
- 시나리오 B가 아닌 경우 해당 필드는 `N/A`로 명시합니다.

```markdown
- 점검 일시:
- 점검 버전(커밋):
- 시나리오: A/B/C
- 관찰 요약:
- 수집 증거(readyz/metrics/job 샘플):
- 실패/이슈:
- 즉시 조치:
- 롤백 여부(실시/미실시 + 근거):
- 롤백 실행 책임자(Owner):
- 롤백 승인자(Approver):
- 보안 영향 검토(있음/없음 + 근거):
- 재실행 예정일:
- rejection 분포(건수): queue_full=<n>, engine_full=<n>  # 시나리오 B 필수, 그 외 N/A
- 후속 액션(담당/기한):
```

### 7-3. 템플릿 예시 (편차 최소화용)

> 정례 점검 시에는 `docs/operations/weekly-drill/_template.md`를 복사해 사용하세요.
> 아래는 단일 시나리오 기록 시 최소 필드 예시입니다.

```markdown
# 2026-03-02 주간 운영 점검 결과

- 점검 일시: 2026-03-02 14:00~14:25 KST
- 점검 버전(커밋): abcdef1
- 시나리오: C
- 관찰 요약: burst 500req 구간에서 429 발생, 부하 종료 후 3분 내 queue_depth 정상화
- 수집 증거(readyz/metrics/job 샘플):
  - /readyz raw: `HTTP/1.1 200 OK` / `{"status":"ready"}`
  - /metrics 스냅샷: `rejections{engine="stt",reason="queue_full"}=37`, `rejections{engine="stt",reason="engine_full"}=12`
  - POST /jobs 샘플: `POST /jobs?engine=stt` -> `429 {"error":"queue_full"}`
  - GET /jobs/{id} 샘플: `GET /jobs/3f...` -> `200 {"status":"rejected"}`
- 실패/이슈: 피크 구간에서 queue_full 비중이 예상보다 높음(37건)
- 즉시 조치: STT 입력 burst를 20% 감쇠하도록 임시 rate limit 적용
- 롤백 여부(실시/미실시 + 근거): 미실시(ready 유지, 큐 수렴 확인됨)
- 롤백 실행 책임자(Owner): oncall-ops-1
- 롤백 승인자(Approver): sre-lead
- 보안 영향 검토(있음/없음 + 근거): 없음(인증/인가/비밀값 경로 변경 없음, 오류는 부하 제어 범주)
- 재실행 예정일: 2026-02-26
- rejection 분포(건수): queue_full=37(75.5%), engine_full=12(24.5%)
- 후속 액션(담당/기한): 큐 용량/worker 재튜닝안 작성 (ops-team, 2026-02-27)
```
