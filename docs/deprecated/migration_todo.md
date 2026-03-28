# Deprecated: Modules Migration TODO

이 문서는 외부 모듈 소스 디렉터리를 `modules/*` 아래로 이동하던 시기의 작업 로그다.
현재 저장소 기준으로 migration 자체는 완료된 상태이며, 더 이상 활성 TODO가 아니다.

## 최종 상태

외부 모듈 소스 위치:

- `modules/ffmpeg`
- `modules/whisper.cpp`
- `modules/llama.cpp`

런타임 실행 파일은 계속 `.build/...` 산출물을 사용한다.

## 현재 기준 문서

- 모듈/런타임 구조: `docs/architecture.md`
- 외부 도구 연동 방식: `docs/API_Audit.md`

## 보존 목적

이 문서는 Git/submodule 정리와 경로 이동 과정의 이력 보존용이다.
새 경로 정책이나 현재 운영 규칙을 설명하는 기준 문서로 사용하지 않는다.
