# Modules Migration TODO (2026-03-26)

## 목적

이 문서는 RecordRoute 저장소에서 외부 모듈 소스 디렉터리

- `modules/ffmpeg/`
- `modules/whisper.cpp/`
- `modules/llama.cpp/`

를 루트에서 `modules/` 하위로 이동할 때 필요한 작업을 정리한다.

목표 구조는 다음과 같다.

```text
modules/
  ffmpeg/
  whisper.cpp/
  llama.cpp/
```

이번 변경의 핵심은 "런타임 파이프라인 변경"이 아니라
"빌드/도구 경로와 Git 메타데이터 정리"이다.

---

## 현재 확인된 상태

- 런타임 바이너리 탐색은 모듈 소스 경로가 아니라 `.build/...` 기준이다.
- 모델 저장 위치는 `models/whisper/...`, `models/llama/...` 기준이며 이번 변경과 직접 무관하다.
- `ffmpeg`와 `llama.cpp`는 현재 부모 저장소의 submodule 형태로 연결되어 있다.
- `whisper.cpp`는 현재 체크아웃 기준으로 일반적인 submodule 포맷이 아니라 별도 nested Git 작업트리 형태에 가깝다.
- `.gitmodules`는 이미 `modules/*` 경로를 가리키지만, 부모 저장소 index의 gitlink는 아직 `ffmpeg`, `llama.cpp`, `whisper.cpp` 루트 경로를 추적한다.
- `.git/modules/ffmpeg/config`, `.git/modules/llama.cpp/config`의 `core.worktree`는 각각 `../../../ffmpeg`, `../../../llama.cpp`로 남아 있다.
- `modules/ffmpeg`, `modules/llama.cpp`, `modules/whisper.cpp`는 현재 비어 있고, 실제 작업트리는 아직 루트 구 경로에 있다.
- 루트 `whisper.cpp`는 부모 저장소 기준으로는 gitlink이지만 실제 checkout은 자체 `.git/` 디렉터리를 가진 nested repo이며, 부모 `.git/modules/whisper.cpp` 엔트리가 없다.
- 따라서 세 모듈을 같은 절차로 옮기면 안 된다.

정리하면, `modules/` 이동 자체는 가능하며 Git 정리 정책은 "세 모듈 모두 submodule로 통일"로 확정했다.
남은 일은 특히 `whisper.cpp` nested checkout을 부모 저장소 submodule 메타데이터로 흡수한 뒤 새 경로로 옮기는 것이다.

---

## 범위 밖

이번 migration에서 직접 바뀌지 않아야 하는 영역:

- `db/index.json` 구조
- Job/Task 상태 전이 규칙
- API 라우트 구조
- `.build/ffmpeg/...`, `.build/whisper/...`, `.build/llama/...` 산출물 레이아웃
- `models/whisper/...`, `models/llama/...` 캐시 경로

즉, API/CLI의 동작 의미는 유지하고 소스 모듈 위치만 바꾸는 것이 목표다.

---

## 확정된 결정 사항

### 1. `whisper.cpp` Git 관리 방식

결정:

- `whisper.cpp`는 `ffmpeg`, `llama.cpp`와 동일하게 부모 저장소 submodule로 표준화한다.
- nested repo 유지안은 채택하지 않는다.

이유:

- 세 모듈의 clone/bootstrap 절차를 `git submodule update --init --recursive` 하나로 통일할 수 있다.
- `.gitmodules`가 이미 `modules/whisper.cpp`를 목표 경로로 선언하고 있어 최종 상태와도 맞는다.
- CI, 신규 clone, 협업 환경에 별도 예외 절차를 남기지 않는 편이 안전하다.

실행 원칙:

- 현재 부모 저장소 index의 gitlink는 아직 루트 `whisper.cpp`를 가리키므로, `git submodule absorbgitdirs whisper.cpp`를 수행할 때는 submodule 명세와 실제 path가 일치하는 작업 상태를 먼저 만들어야 한다.
- 표준화가 끝난 후 최종 상태는 `modules/whisper.cpp` 경로의 submodule이어야 하며, 독립 `.git/` 디렉터리는 남기지 않는다.

권장 실행 순서:

1. `.gitmodules`의 `whisper.cpp` path가 현재 gitlink path(`whisper.cpp`)와 일치하는 작업 상태를 만든다.
   현재 브랜치처럼 이미 `modules/whisper.cpp`로 바뀌어 있다면 일시적으로 되돌리거나, path 변경 이전 커밋에서 작업한 뒤 마지막에 다시 적용한다.
2. 부모 저장소에서 `git submodule absorbgitdirs whisper.cpp`를 실행한다.
3. `whisper.cpp/.git`가 디렉터리에서 파일로 바뀌고 `gitdir: ../.git/modules/whisper.cpp`를 가리키는지 확인한다.
4. `git mv whisper.cpp modules/whisper.cpp`로 worktree와 gitlink path를 함께 이동한다.
5. `.gitmodules`의 `whisper.cpp` path를 `modules/whisper.cpp`로 맞추고 `git add .gitmodules modules/whisper.cpp`를 수행한다.
6. `git submodule sync -- modules/whisper.cpp`로 로컬 submodule 설정을 동기화한다.
7. `modules/whisper.cpp/.git`가 `../../.git/modules/whisper.cpp`를 가리키고, `.git/modules/whisper.cpp/config`의 `core.worktree`가 `../../../modules/whisper.cpp`인지 확인한다.

---

## 작업 TODO

## 1. Git / 저장소 메타데이터

### 2026-03-26 점검 결과

- `.gitmodules`의 선언 경로와 부모 저장소 index의 gitlink 경로가 아직 일치하지 않는다.
- 로컬 `.git/config`에는 `submodule "ffmpeg"`, `submodule "llama.cpp"`만 있고 `whisper.cpp` 엔트리가 없다.
- `ffmpeg/.git`, `llama.cpp/.git`는 각각 `../.git/modules/ffmpeg`, `../.git/modules/llama.cpp`를 가리킨다.
- 신규 clone에서 `git submodule update --init --recursive`를 수행하면 현재 HEAD에서 즉시 실패한다.
  - 재현 예: `fatal: No url found for submodule path 'ffmpeg' in .gitmodules`
  - 원인: 부모 저장소 index의 gitlink는 아직 루트 `ffmpeg`, `llama.cpp`, `whisper.cpp`를 가리키지만 `.gitmodules`는 이미 `modules/*` 경로만 선언한다.

- [x] `.gitmodules`의 submodule path를 다음과 같이 변경
  - `ffmpeg -> modules/ffmpeg`
  - `whisper.cpp -> modules/whisper.cpp`
  - `llama.cpp -> modules/llama.cpp`
- [x] 로컬 `.git/config`와 `.git/modules/*/config`가 새 path 기준과 충돌하는지 점검
  - 점검 결과: `.git/config`에는 `whisper.cpp` 엔트리가 없고, `.git/modules/ffmpeg/config`, `.git/modules/llama.cpp/config`의 `core.worktree`는 아직 루트 구 경로를 가리킨다.
- [ ] `ffmpeg`, `llama.cpp`의 실제 submodule worktree를 `modules/*`로 옮기고 `gitdir` 구조를 새 경로 기준으로 정리
  - 현재 `modules/ffmpeg`, `modules/llama.cpp`는 비어 있고 실제 `.git` 파일은 `ffmpeg/.git`, `llama.cpp/.git`에 있다.
  - 권장 절차: 부모 저장소에서 `git mv ffmpeg modules/ffmpeg`, `git mv llama.cpp modules/llama.cpp`를 수행한 뒤 `modules/<name>/.git`가 `../../.git/modules/<name>`를 가리키는지, `.git/modules/<name>/config`의 `core.worktree`가 `../../../modules/<name>`로 바뀌었는지 확인한다.
  - 필요 시 `git submodule absorbgitdirs modules/<name>`로 gitdir/worktree 메타데이터를 다시 흡수한다.
- [x] `whisper.cpp`는 nested repo가 아니라 submodule로 표준화하기로 확정하고, 실제 이동 절차를 문서화
  - 현재 로컬 checkout은 full `.git/` 디렉터리를 가진 nested repo이며, 부모 `.git/modules/whisper.cpp` 메타데이터가 없다.
  - 채택안: 루트 `whisper.cpp` nested checkout을 부모 저장소 submodule 메타데이터로 흡수한 뒤 `modules/whisper.cpp`로 이동한다.
  - 실행 전제: 현재 `.gitmodules`는 이미 목표 경로(`modules/whisper.cpp`)를 가리키지만 index gitlink는 아직 `whisper.cpp`이므로, `git submodule absorbgitdirs whisper.cpp`를 실행할 수 있도록 path를 잠시 다시 맞춰야 한다.
  - 권장 절차:
    1. `.gitmodules`의 `whisper.cpp` path를 일시적으로 `whisper.cpp`로 맞추거나, path 변경 이전 상태에서 작업 시작
    2. `git submodule absorbgitdirs whisper.cpp`
    3. `whisper.cpp/.git`가 `gitdir: ../.git/modules/whisper.cpp` 파일로 바뀌었는지 확인
    4. `git mv whisper.cpp modules/whisper.cpp`
    5. `.gitmodules`의 path를 `modules/whisper.cpp`로 다시 맞추고 `git submodule sync -- modules/whisper.cpp`
    6. `modules/whisper.cpp/.git`와 `.git/modules/whisper.cpp/config`의 `core.worktree`를 새 경로 기준으로 검증
- [ ] 문서화된 절차대로 `whisper.cpp` nested checkout을 부모 저장소 submodule 메타데이터로 흡수하고 `modules/whisper.cpp`로 실제 이동
- [ ] 신규 클론 환경에서 `git submodule update --init --recursive`가 정상 동작하는지 검증
  - 2026-03-26 신규 clone 재현 결과, 현재 HEAD에서는 아래 오류로 즉시 실패했다.
    - `fatal: No url found for submodule path 'ffmpeg' in .gitmodules`
  - 현재 HEAD는 gitlink path가 아직 루트 `ffmpeg`, `llama.cpp`, `whisper.cpp`에 남아 있으므로, 실제 경로 이전이 끝나기 전에는 `modules/*` 기준 검증을 통과할 수 없다.

영향 파일:

- `.gitmodules`
- `.git/config` (로컬 상태 점검용)
- `modules/ffmpeg/.git`
- `modules/llama.cpp/.git`
- `modules/whisper.cpp/.git` 또는 관련 Git 메타데이터

## 2. 빌드 스크립트 경로 수정

현재 빌드 스크립트는 모듈 소스 경로를 루트 기준으로 직접 참조한다.

- [x] `scripts/build_ffmpeg.sh`의 `source_dir`를 `modules/ffmpeg`로 변경
- [x] `scripts/build_ffmpeg.bat`의 `source_dir`를 `modules\\ffmpeg`로 변경
- [x] `scripts/build_whisper.sh`의 `source_dir`를 `modules/whisper.cpp`로 변경
- [x] `scripts/build_whisper.bat`의 `source_dir`를 `modules\\whisper.cpp`로 변경
- [x] `scripts/build_llama.sh`의 `source_dir`를 `modules/llama.cpp`로 변경
- [x] `scripts/build_llama.bat`의 `source_dir`를 `modules\\llama.cpp`로 변경

주의:

- `.build/...` 출력 경로는 유지하는 편이 안전하다.
- 이번 migration은 "소스 위치 이동"이지 "빌드 산출물 구조 변경"이 아니다.

## 3. Rust 프로덕션 코드 경로 수정

`ffmpeg`와 `llama.cpp`는 Rust 프로덕션 코드에서 소스 경로를 직접 거의 쓰지 않지만,
`whisper.cpp`는 다운로드 스크립트 위치를 직접 참조한다.

- [x] `rust/src/whisper.rs`의 다운로드 스크립트 경로를 `modules/whisper.cpp/models/...`로 변경
- [ ] 필요하면 모듈 루트 경로를 반환하는 헬퍼 함수를 추가해 경로 문자열 하드코딩을 줄임
- [ ] 경로 관련 에러 메시지가 새 구조를 반영하는지 확인

권장 리팩터링:

- `repo_root.join("modules").join("<module-name>")` 형태를 공통 함수로 감싸기
- `whisper.cpp/models/download-ggml-model.*` 같은 문자열 하드코딩을 한 곳으로 모으기

## 4. 테스트 코드 / fixture 경로 수정

현재 테스트는 `whisper.cpp/models` 경로를 fixture 디렉터리처럼 여러 번 직접 만든다.

- [x] `rust/src/whisper.rs` 테스트의 `whisper.cpp/models` 경로를 `modules/whisper.cpp/models`로 변경
- [x] `rust/src/app.rs` 테스트 fixture의 `whisper.cpp/models` 경로를 `modules/whisper.cpp/models`로 변경
- [x] `rust/src/server.rs` 테스트 fixture의 `whisper.cpp/models` 경로를 `modules/whisper.cpp/models`로 변경
- [x] 테스트 내부 `download-ggml-model.{cmd,sh}` 경로 helper를 새 구조 기준으로 수정

비고:

- 실제 수정량은 프로덕션 코드보다 테스트 코드가 더 많다.
- migration 누락은 주로 테스트 실패로 먼저 드러날 가능성이 높다.

## 5. 문서 갱신

- [x] `README.md`에 모듈 위치 변경 사실 반영
- [ ] `README.md`의 submodule 초기화 가이드가 새 구조에서도 유효한지 검증
- [x] `docs/API_Audit.md`의 `whisper.cpp/models/download-ggml-model.*` 경로를 `modules/whisper.cpp/models/...` 기준으로 갱신
- [x] `docs/architecture.md`에 외부 모듈 소스 위치를 별도 명시
- [x] 이 문서(`docs/migration_todo.md`)를 실제 migration 진행 상황에 맞게 갱신

비고:

- API 문서 자체는 바뀌지 않지만, 외부 모듈 경로 설명은 코드 기준으로 맞춰야 한다.

## 6. 실행 검증

- [x] Windows에서 `setup.bat` 실행 검증
- [ ] Linux/macOS에서 `setup.sh` 실행 검증
- [x] `cargo test --manifest-path rust/Cargo.toml` 실행
- [ ] 최소 1회 `ffmpeg -> stt -> summary` 흐름을 실제 또는 fixture 기반으로 점검
- [x] `prepare-llama-model` 실행 검증
- [x] HTTP API의 모델 준비 엔드포인트 동작 검증
  - `POST /models/whisper/prepare`
  - `POST /models/llama/prepare`

검증 목표:

- 소스 모듈 위치만 바뀌고, 기존 파이프라인 동작은 유지되는지 확인
- Windows 워크스페이스에서 `cargo run --manifest-path rust/Cargo.toml -- prepare-llama-model` 종료 코드 0 확인
- 기존 llama 캐시(`models/llama/hf/ggml-org__gemma-3-4b-it-GGUF.gguf`)를 정상 인식했고 `db/index.json`의 `model_preparations.llama.status`가 `completed`로 유지/갱신됨

---

## 우선순위 제안

1. `whisper.cpp` nested checkout을 부모 저장소 submodule로 흡수
2. 세 모듈의 실제 디렉터리 이동과 gitlink 경로 정리
3. 빌드 스크립트 수정
4. Rust 프로덕션 코드 수정
5. 테스트 fixture 수정
6. 문서 갱신
7. 전체 검증

---

## 예상 리스크

### 높음

- `whisper.cpp`가 아직 다른 두 모듈과 Git 구조가 달라, 표준화 완료 전까지 이동 절차가 비대칭적임
- submodule path 변경 후 로컬 작업트리가 꼬일 수 있음

### 중간

- 테스트 fixture의 `whisper.cpp/models` 경로 누락
- Windows 배치 스크립트와 Unix 셸 스크립트의 경로 구분자 차이

### 낮음

- `.build/...`와 `models/...`를 유지하면 런타임 동작 의미 자체가 바뀔 가능성은 낮음

---

## 권장 구현 원칙

- 경로 문자열 하드코딩을 최소화한다.
- `modules/<name>` 경로를 공통 헬퍼로 모은다.
- `.build/...`, `models/...`, `db/...`는 이번 작업에서 가급적 그대로 둔다.
- Git 정리와 코드 경로 수정은 한 커밋에 섞기보다 단계적으로 나누는 편이 안전하다.

---

## 완료 기준

다음 조건을 만족하면 migration 완료로 본다.

- 저장소 루트에 `ffmpeg/`, `whisper.cpp/`, `llama.cpp/`가 더 이상 없고 `modules/` 아래로 정리됨
- 신규 클론 후 submodule 초기화가 정상 동작함
- `setup.sh`, `setup.bat`, `cargo test`가 성공함
- `whisper` 모델 다운로드와 `llama` 모델 준비가 새 경로에서 정상 동작함
- 기존 API/CLI 의미와 상태 저장 규칙이 깨지지 않음
