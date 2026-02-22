# Rust 신규 백엔드 전환 계획 (Legacy Deprecated 전제)

> 상태 업데이트: 기존 Python 백엔드(`sttEngine/*`)는 deprecated 유지 구역으로 분류.
> 백엔드 전환 전략은 점진 이관(hybrid) 중심에서 **Rust 신규 구축(greenfield)** 중심으로 변경.

## 목표

- Rust 신규 백엔드를 독립적으로 구축/출시
- 레거시는 유지보수 최소화(핫픽스/호환성)
- 프론트/API 계약 호환을 보장하며 단계적 트래픽 전환

## 단계

### Phase 0 — 정책/범위 확정
- [ ] 레거시/신규 경계 ADR 확정
- [ ] API 호환 정책(v2/v3) 확정
- [ ] 릴리즈/롤백 정책 확정

### Phase 1 — Rust 기반 구축
- [ ] `rust-backend/` 스캐폴드 구성
- [ ] config/logging/error 공통 모듈
- [ ] health/metrics/tracing 구성
- [ ] CI(lint/test/build) 구성

### Phase 2 — 핵심 API 재구현
- [ ] read API (`/health`, `/models`, `/history`)
- [ ] 검색 API (`/search`, `/similar`)
- [ ] 업로드/태스크 API (`/upload`, `/tasks`, `/progress`)
- [ ] OpenAPI 문서 동기화

### Phase 3 — 워크플로우/Provider 통합
- [ ] `/process` 상태 전이/취소/재시도 설계
- [ ] STT/LLM/Embedding provider 추상화
- [ ] 오류 규약(`error_code/retryable/failed_step`) 호환
- [ ] `DB/...` alias 경로 호환

### Phase 4 — 병행 운영/전환
- [ ] canary/blue-green 구성
- [ ] 성능/안정성 검증
- [ ] 점진 트래픽 전환
- [ ] 레거시 운영 모드 전환(신규개발 중단)

## 완료 기준

- Rust 백엔드로 핵심 API/워크플로우 제공 가능
- 프론트 E2E 시나리오 통과
- SLO 충족(지연/에러율/안정성)
- 레거시는 deprecated 유지 모드로 전환 완료
