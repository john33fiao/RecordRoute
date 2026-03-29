---
name: kese-guide
description: KESE AI 도구용 시큐어 코딩 프롬프트와 가이드를 생성합니다. KISA CII 가이드라인과 CWE 매핑 취약점 패턴을 따르는 보안 코드 작성을 위한 복사-붙여넣기 가능한 프롬프트를 생성합니다. "KESE 보안 가이드 생성", "AI 보안 프롬프트", "시큐어코딩 가이드", "AI에게 안전한 코드 요청" 시 사용하세요.
---

# KESE Wrapper: Guide

이 스킬은 KESE-KIT 한국어 `guide` 스킬의 로컬 래퍼입니다.

## 우선 적용 규칙

- 이 저장소에서는 먼저 [`../../../AGENTS.md`](../../../AGENTS.md)를 따르십시오.
- 코드나 문서를 수정하는 흐름이면 먼저 [`../rtd-before/SKILL.md`](../rtd-before/SKILL.md)를 적용하고, 변경 후에는 [`../rtd-after/SKILL.md`](../rtd-after/SKILL.md)를 적용하십시오.
- 저장소 규칙과 upstream KESE 지침이 충돌하면 저장소 규칙을 우선하십시오.

## Source Of Truth

- 상세 프롬프트 생성 절차와 예시는 [`../../../modules/KESE-KIT/skills-ko/guide/SKILL.md`](../../../modules/KESE-KIT/skills-ko/guide/SKILL.md)를 읽고 그대로 따르십시오.
- 추가 배경이나 항목별 보안 근거가 더 필요할 때만 [`../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md`](../../../modules/KESE-KIT/authorkit/KESE-KIT-완전판.md)를 읽으십시오.
- upstream markdown만으로 부족할 때만 `../../../modules/KESE-KIT/문서/*.pdf`를 참고하십시오.

## Wrapper Notes

- 원본이 정의한 출력 구조와 프롬프트 의미를 바꾸지 마십시오.
- `modules/KESE-KIT` 내부 파일은 사용만 하고 수정하지 마십시오. 별도 요청이 있을 때만 수정합니다.
