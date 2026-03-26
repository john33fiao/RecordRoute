---
name: rtd-before
description: Use when planning any code or documentation change in this repository before editing files, including features, bug fixes, refactors, API doc updates, or MCP PoC tasks. Define scope, DoD, assumptions, test strategy, rollback, and remove over-engineering before implementation starts.
---

# RTD Before

구현 전에 계획의 품질부터 닫아 두는 저장소 전용 사전 게이트입니다.
작업 범위, 성공 기준, 테스트 전략, 가정, 롤백 경로를 먼저 정리하고 과도한 설계를 제거하십시오.

## 저장소 전제 확인

- 먼저 트리를 확인하고, 이 저장소에 아직 `Cargo.toml`, `src/`, 실행 가능한 MCP 서버가 없을 수 있다는 점을 전제로 시작하십시오.
- API 의미를 건드리는 작업이면 `docs/API_Doc.md`를 먼저 읽고 기준 문서로 사용하십시오. `openapi.yaml`은 파생 문서이므로 충돌 시 `docs/API_Doc.md`를 우선하십시오.
- 이 PoC는 read-only 범위를 벗어나지 않습니다. 쓰기/수정/삭제 MCP 도구를 계획에 넣지 마십시오.
- 문서 변경은 한국어 스타일을 유지하십시오.

## 작업 순서

### 1. 계획을 세우십시오

- 목표(What), 범위(In/Out), 성공 기준(DoD), 제약(시간/성능/호환성/보안)을 한 번에 정리하십시오.
- 변경 예상 파일/모듈, 테스트 전략, 롤백 전략을 포함하십시오.
- PASS 기준: “무엇을/왜/어떻게/어떻게 검증”이 한 장에 들어갑니다.

### 2. 계획을 검토하십시오

- 엣지 케이스, 에러 처리, 마이그레이션, 호환성, 모니터링 누락을 찾으십시오.
- 레포 현 상태에서 실제로 가능한 검증인지 확인하십시오. 없는 빌드/테스트를 가정하지 마십시오.
- PASS 기준: 누락이 없거나 보완된 수정 계획이 제시됩니다.

### 3. 검토한 것이 맞는지 다시 검토하십시오

- Step 2가 DoD와 Out-of-scope를 제대로 반영했는지 다시 보십시오.
- “가정이 사실이 아니면 무엇이 깨지는가”를 3개 이내로 명시하십시오.
- PASS 기준: 재검토 결과가 계획에 반영됩니다.

### 4. 계획이 과도하지 않은지 검토하십시오

- YAGNI와 단순성 관점에서 불필요한 구조, 추상화, 범위 확장을 제거하십시오.
- Dooray MCP PoC의 read-only, `Project` 우선, 문서 우선 방향과 충돌하는 설계는 걷어내십시오.
- PASS 기준: 지금 필요한 최소 변경으로 압축됩니다.

## 출력 형식

- 각 단계는 아래 형식으로 기록하십시오.
- ` [Step N] 제목 - PASS/FAIL `
- 핵심 근거는 최대 5줄로 제한하십시오.
- 필요할 때만 변경/조치 목록과 다음 단계 리스크를 덧붙이십시오.

## 종료 조건

- Step 1-4가 모두 PASS이면 구현을 시작하십시오.
- 구현이 끝나면 같은 저장소의 `rtd-after` 스킬로 넘어가 후속 품질 게이트를 수행하십시오.
