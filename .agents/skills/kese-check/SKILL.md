---
name: kese-check
description: KESE 배포 전 CII 컴플라이언스 체크리스트를 실행합니다. KISA 가이드라인 기반으로 계정 관리, 접근 제어, 암호화, 로깅, 인프라 하드닝을 대화형으로 검증합니다. "KESE 배포 전 점검", "CII 컴플라이언스 체크", "배포 준비 완료?", "보안 체크리스트" 시 사용하세요.
---

# KESE Wrapper: Check

이 스킬은 KESE-KIT 한국어 `check` 스킬의 로컬 래퍼입니다.

## 우선 적용 규칙

- 이 저장소에서는 먼저 [`../../../AGENTS.md`](../../../AGENTS.md)를 따르십시오.
- 코드나 문서를 수정하는 흐름이면 먼저 [`../rtd-before/SKILL.md`](../rtd-before/SKILL.md)를 적용하고, 변경 후에는 [`../rtd-after/SKILL.md`](../rtd-after/SKILL.md)를 적용하십시오.
- 저장소 규칙과 upstream KESE 지침이 충돌하면 저장소 규칙을 우선하십시오.

## Source Of Truth

- 상세 체크리스트와 심각도 기준은 [`../../../modules/KESE-KIT/skills-ko/check/SKILL.md`](../../../modules/KESE-KIT/skills-ko/check/SKILL.md)를 읽고 그대로 따르십시오.
- 추가 배경이나 판정 근거가 더 필요할 때만 [`../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md`](../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md)를 읽으십시오.
- upstream markdown만으로 부족할 때만 `../../../modules/KESE-KIT/문서/*.pdf`를 참고하십시오.

## Wrapper Notes

- 원본이 정의한 출력 형식과 보고서 의미를 바꾸지 마십시오.
- `modules/KESE-KIT` 내부 파일은 사용만 하고 수정하지 마십시오. 별도 요청이 있을 때만 수정합니다.
