---
name: kese-fix
description: KESE CII 시스템에서 발견된 보안 취약점을 자동 수정합니다. KISA 가이드라인에 따라 Unix/Linux, Windows, 웹 서버, 데이터베이스용 하드닝 스크립트를 생성합니다. "KESE 취약점 수정", "시스템 보안", "보안 수정 적용", "서버 하드닝", "KISA 하드닝" 시 사용하세요.
---

# KESE Wrapper: Fix

이 스킬은 KESE-KIT 한국어 `fix` 스킬의 로컬 래퍼입니다.

## 우선 적용 규칙

- 이 저장소에서는 먼저 [`../../../AGENTS.md`](../../../AGENTS.md)를 따르십시오.
- 코드나 문서를 수정하는 흐름이면 먼저 [`../rtd-before/SKILL.md`](../rtd-before/SKILL.md)를 적용하고, 변경 후에는 [`../rtd-after/SKILL.md`](../rtd-after/SKILL.md)를 적용하십시오.
- 저장소 규칙과 upstream KESE 지침이 충돌하면 저장소 규칙을 우선하십시오.

## Source Of Truth

- 상세 수정 절차와 하드닝 스크립트 규칙은 [`../../../modules/KESE-KIT/skills-ko/fix/SKILL.md`](../../../modules/KESE-KIT/skills-ko/fix/SKILL.md)를 읽고 그대로 따르십시오.
- 추가 배경이나 플랫폼별 보안 해설이 더 필요할 때만 [`../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md`](../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md)를 읽으십시오.
- upstream markdown만으로 부족할 때만 `../../../modules/KESE-KIT/문서/*.pdf`를 참고하십시오.

## Wrapper Notes

- 원본이 정의한 출력 경로와 의미를 바꾸지 마십시오. 예: `scripts/kese-hardening/`
- `modules/KESE-KIT` 내부 파일은 사용만 하고 수정하지 마십시오. 별도 요청이 있을 때만 수정합니다.
