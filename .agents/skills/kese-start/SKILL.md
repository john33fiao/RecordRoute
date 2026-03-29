---
name: kese-start
description: KESE 주요정보통신기반시설(CII) 취약점 분석평가를 수행합니다. KISA 기반 기술적 취약점(424항목), 관리적 취약점(127항목), 물리적 취약점(9항목)을 평가합니다. "KESE 보안 점검 시작", "KESE 취약점 분석", "기반시설 감사", "KISA 점검", "보안 평가" 시 사용하세요.
---

# KESE Wrapper: Start

이 스킬은 KESE-KIT 한국어 `start` 스킬의 로컬 래퍼입니다.

## 우선 적용 규칙

- 이 저장소에서는 먼저 [`../../../AGENTS.md`](../../../AGENTS.md)를 따르십시오.
- 코드나 문서를 수정하는 흐름이면 먼저 [`../rtd-before/SKILL.md`](../rtd-before/SKILL.md)를 적용하고, 변경 후에는 [`../rtd-after/SKILL.md`](../rtd-after/SKILL.md)를 적용하십시오.
- 저장소 규칙과 upstream KESE 지침이 충돌하면 저장소 규칙을 우선하십시오.

## Source Of Truth

- 상세 절차와 실제 판단 기준은 [`../../../modules/KESE-KIT/skills-ko/start/SKILL.md`](../../../modules/KESE-KIT/skills-ko/start/SKILL.md)를 읽고 그대로 따르십시오.
- 추가 배경이나 항목별 해설이 더 필요할 때만 [`../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md`](../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md)를 읽으십시오.
- upstream markdown만으로 부족할 때만 `../../../modules/KESE-KIT/문서/*.pdf`를 참고하십시오.

## Wrapper Notes

- 이 저장소에서는 upstream `reports/kese/`를 `docs/kese/reports/`로 재해석하십시오.
- 보고서와 관련 산출물은 모두 `docs/kese/reports/` 하위에 생성하십시오.
- `modules/KESE-KIT` 내부 파일은 사용만 하고 수정하지 마십시오. 별도 요청이 있을 때만 수정합니다.
