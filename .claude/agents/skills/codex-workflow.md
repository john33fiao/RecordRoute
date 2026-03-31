---
name: codex-workflow
description: >
  Codex CLI를 활용한 코드 작업 위임 워크플로우 스킬.
  사용자 메시지에 "코덱스" 또는 "codex"가 포함되거나,
  코드 구현·수정·디버깅·리뷰·테스트 실패 등 코드 작업 요청이 들어오면
  반드시 이 스킬을 참조하여 Codex에 위임하는 방향으로 진행한다.
  Claude는 오케스트레이션과 컨텍스트 정리만 담당하고,
  실제 코드 실행·수정·검토는 Codex에 위임하는 것을 원칙으로 한다.
---

# Codex Workflow Skill

## 역할 분담 원칙

Claude는 **토큰 제약**이 있으므로 아래 원칙을 엄격히 따른다.

| 역할 | 담당 |
|------|------|
| 요구사항 분석 및 정리 | Claude |
| 작업 방향 설계 및 우선순위 판단 | Claude |
| 실제 코드 구현·수정 | Codex (`/codex:rescue`) |
| 코드 리뷰·검증 | Codex (`/codex:review`) |
| 설계 결정 압박 테스트 | Codex (`/codex:adversarial-review`) |
| 버그 조사 및 수정 시도 | Codex (`/codex:rescue`) |
| 결과 해석 및 다음 액션 판단 | Claude |

---

## 트리거 조건

다음 중 하나라도 해당하면 이 스킬을 적용한다.

- 사용자 메시지에 "코덱스" 또는 "codex" 포함
- 코드 구현, 기능 추가, 리팩터링 요청
- 버그 조사, 테스트 실패 원인 분석 요청
- 커밋 또는 PR 전 리뷰 요청
- 설계 결정의 타당성 검토 요청

---

## 커맨드 레퍼런스

### `/codex:rescue` — 작업 위임

코드 구현·수정·버그 수정을 Codex에 위임한다.

```
/codex:rescue <작업 설명>
/codex:rescue --background <작업 설명>         # 장시간 작업 시 권장
/codex:rescue --resume <이전 작업 이어받기>
/codex:rescue --model gpt-5.4-mini --effort medium <작업 설명>  # 빠른 패스
```

### `/codex:review` — 코드 리뷰

현재 uncommitted 변경사항 또는 브랜치 대비 리뷰.

```
/codex:review
/codex:review --base main                      # main 대비 브랜치 리뷰
/codex:review --background                     # 멀티파일 변경 시 권장
```

### `/codex:adversarial-review` — 설계 압박 리뷰

구현 방향, 트레이드오프, 숨겨진 가정에 의문을 제기한다.

```
/codex:adversarial-review
/codex:adversarial-review --base main <포커스 설명>
/codex:adversarial-review --background <리스크 영역 설명>
```

### 백그라운드 작업 관리

```
/codex:status                                  # 진행 중인 작업 확인
/codex:status <task-id>                        # 특정 작업 상태
/codex:result                                  # 완료된 작업 결과 확인
/codex:cancel                                  # 작업 취소
```

---

## 워크플로우별 절차

### A. 코드 구현 / 기능 추가

1. Claude가 요구사항을 한 단락으로 정리한다.
2. 접근 방향 및 제약 조건을 간략히 명시한다.
3. `/codex:rescue --background <정리된 태스크>` 실행
4. 다른 작업을 병렬 진행하거나 대기
5. `/codex:status` → `/codex:result` 순으로 확인
6. Claude가 결과를 해석하고 후속 액션을 판단한다.

### B. 버그 조사 / 테스트 실패

1. 증상과 재현 조건을 Claude가 정리한다.
2. `/codex:rescue --background 버그 설명 및 재현 조건` 실행
3. 빠른 패스가 필요한 경우:
   `/codex:rescue --model gpt-5.4-mini --effort medium <설명>`
4. `/codex:result`로 원인 분석 결과 확인
5. 수정 적용 여부는 Claude가 판단한다.

### C. 커밋 / PR 전 리뷰

1. `/codex:review --background` 실행 (기본)
2. 설계 결정이 포함된 경우:
   `/codex:adversarial-review --background` 추가 실행
3. `/codex:result`로 이슈 목록 확인
4. 중대한 이슈가 있으면 `/codex:rescue`로 수정 위임

### D. 장시간 작업 관리

```
# 시작
/codex:rescue --background <태스크>

# 진행 확인
/codex:status

# 결과 수령 및 Codex에서 이어받기
/codex:result
codex resume <session-id>
```

---

## Claude의 응답 원칙

- 코드 작업 요청 시 직접 구현하지 않고 Codex 위임 절차를 먼저 제안한다.
- 작업 위임 전 요구사항을 반드시 한 단락으로 요약하여 사용자에게 확인받는다.
- 결과 확인 후 다음 액션(추가 수정 위임, 머지 승인, 재조사 등)을 구체적으로 제안한다.
- 백그라운드 실행 중에는 병렬로 처리 가능한 다른 작업(문서화, 설계 검토 등)을 제안한다.

---

## 설정 참고

프로젝트 루트 `.codex/config.toml`로 기본 모델 및 추론 강도 설정 가능:

```toml
model = "gpt-5.4-mini"
model_reasoning_effort = "xhigh"
```

전역 설정은 `~/.codex/config.toml`에 작성한다.
