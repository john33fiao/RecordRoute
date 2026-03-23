# AGENTS.md - rust/ subtree guide

이 문서는 `rust/` 하위에서 작업하는 에이전트를 위한 전용 규칙입니다.
상위 저장소 루트 `AGENTS.md`의 지시를 **그대로 상속**하며, 이 파일은 `rust/` 범위에서만 추가 규칙을 제공합니다.

---

## 1. 문서 우선순위
- 제품/운영 관점 기준: 루트 `README.md`
- Rust 재작성 기준 사양: `rust/SPEC.md`
- Rust 실행 백로그: `rust/TODO.md`
- 저장소 전체 규칙 원본: 루트 `AGENTS.md`

작업 시작 전 최소 확인 순서:
1. 루트 `AGENTS.md`
2. `rust/SPEC.md`
3. `rust/TODO.md`
4. 필요 시 루트 `README.md`, `docs/current-codebase-overview.md`, `docs/openapi.yaml`

---

## 2. rust/ 작업 목표
- Python 백엔드를 Rust로 대체하기 위한 설계/구현/실험은 `rust/` 아래에서 진행한다.
- 아직 cutover 전이므로, 기존 루트 코드베이스를 무분별하게 뒤집지 말고 **명시적 마이그레이션 경로**를 남긴다.
- `rust/` 문서는 “왜 이런 결정을 했는가”까지 남겨야 한다. 단순 TODO 목록만 추가하지 않는다.

---

## 3. 아키텍처 원칙
- 기본 서버 프레임워크는 `axum`을 우선 검토한다. 다른 선택을 하면 이유를 문서화한다.
- `whisper.cpp`, `llama.cpp`는 git submodule로 관리하는 방향을 기본값으로 둔다.
- 초기 통합은 **가장 단순하고 운영 가능한 방식**을 우선한다.
  - 예: 무리한 FFI보다 CLI/server wrapper를 먼저 채택 가능
- 기존 API 계약과 `DB/...` path alias는 특별한 합의가 없는 한 유지한다.
- Windows 설치 경로를 항상 1급 시나리오로 취급한다.

---

## 4. 문서 수정 규칙
다음 변경 시에는 문서도 함께 갱신한다.
- crate 구조 변경 → `rust/SPEC.md`, `rust/TODO.md`
- 모델 설치 경로/검증 방식 변경 → `rust/SPEC.md`
- 작업 우선순위/마일스톤 변경 → `rust/TODO.md`
- Rust 하위 작업 규칙 변경 → `rust/AGENTS.md`

문서 작성 원칙:
- “하자/검토 필요”보다 “누가 봐도 실행 가능한 기준”으로 적는다.
- 미정 사항은 반드시 `결정 필요`, `옵션`, `권장안`을 나눠 적는다.
- 기존 Python 구현과의 계약 차이가 생기면 반드시 차이를 명시한다.

---

## 5. 구현 규칙
- `rust/` 아래 코드는 workspace crate 단위로 책임을 분리한다.
- 공통 타입은 `recordroute-core` 같은 기초 crate로 모은다.
- path 변환, config 로딩, error mapping은 초기에 중앙화한다.
- API 계층에서 임시 문자열 조합으로 JSON을 만들지 않는다. `serde` 구조체를 사용한다.
- 모델/프로세스 실행 오류는 종료 코드, stderr, timeout을 구조적으로 분류한다.
- 설치/부트스트랩 로직은 서버 바이너리와 분리된 CLI crate에 둔다.

---

## 6. 테스트 규칙
Rust 작업은 아래 검증을 우선한다.
- 단위 테스트: 경로 alias, payload 정규화, 에러 매핑
- 통합 테스트: health/history/search/process 흐름
- 계약 테스트: 루트 `docs/openapi.yaml` 및 현행 JSON payload 호환 여부
- 설치 테스트: submodule 없음, 모델 없음, ffmpeg 없음, 빌드 도구 없음

권장 명령 예시(구현 이후):
- `cargo fmt --all`
- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo test -p recordroute-server --test api_contract`
- `cargo run -p recordroute-cli -- verify-models`

---

## 7. 주의할 리스크
- diarization은 현재 Rust 목표 아키텍처에서 공백 영역일 수 있으므로, 미정 상태를 숨기지 않는다.
- submodule 빌드는 OS/툴체인 편차가 크므로 “내 환경에서는 됨”을 완료 기준으로 삼지 않는다.
- Python 제거가 목표더라도, 초기 단계에서 계약 검증용 fixture나 비교 기준은 적극적으로 재사용한다.
- 모델 다운로드 자동화는 사용자 네트워크/라이선스 제약을 고려해야 한다.

---

## 8. 최종 산출물 기준
rust/ 하위 작업이 완료되었다고 말하려면 최소한 다음이 있어야 한다.
- 사양서(`rust/SPEC.md`)가 현재 구현과 모순되지 않는다.
- 백로그(`rust/TODO.md`)가 추상적 슬로건이 아니라 실행 가능한 작업 단위다.
- 설치/모델 검증 흐름이 문서와 코드 양쪽에 존재한다.
- 기존 프론트/API 계약과의 차이가 있으면 문서화되어 있다.
