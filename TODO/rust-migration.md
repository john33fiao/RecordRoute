# RecordRoute Python → Rust 마이그레이션 가이드

## 목차
1. [개요](#개요)
2. [현재 아키텍처 분석](#현재-아키텍처-분석)
3. [마이그레이션 가능 영역](#마이그레이션-가능-영역)
4. [마이그레이션 어려운/불가능 영역](#마이그레이션-어려운불가능-영역)
5. [마이그레이션 단계별 전략](#마이그레이션-단계별-전략)
6. [기술 스택 매핑](#기술-스택-매핑)
7. [위험 요소 및 완화 전략](#위험-요소-및-완화-전략)
8. [결론 및 권고사항](#결론-및-권고사항)

---

## 개요

RecordRoute는 현재 **Electron (프론트엔드) + Python 백엔드 (sttEngine)** 구조로 동작합니다.
본 문서는 Python 백엔드를 Rust로 전환하는 것의 **실현 가능성, 난이도, 단계별 전략**을 제시합니다.

### 마이그레이션 목표
- Python 의존성 최소화 또는 완전 제거
- 성능 향상 (특히 서버 처리 속도, 메모리 사용량)
- 배포 단순화 (단일 바이너리 지향)
- 타입 안정성 강화

---

## 현재 아키텍처 분석

### 통신 구조
```
Electron (main.js)
    ↓ spawn
Python Backend (server.py:8000)
    ↓ HTTP/REST API
    ↓ WebSocket (:8765) - 실시간 진행상황 업데이트
Frontend (upload.html)
```

### Python 백엔드 주요 역할
1. **HTTP 서버**: 파일 업로드, 다운로드, 처리 요청 처리
2. **STT 엔진**: Whisper 모델을 이용한 음성→텍스트 변환
3. **LLM 추론**: llama.cpp 기반 로컬 텍스트 요약/교정
4. **벡터 검색**: sentence-transformers로 임베딩 생성 및 유사도 검색
5. **파일 처리**: PDF 텍스트 추출, 미디어 파일 처리 (ffmpeg)
6. **WebSocket**: 실시간 작업 진행상황 브로드캐스트

### 핵심 Python 의존성
```python
openai-whisper         # STT (Whisper 모델)
llama-cpp-python       # 로컬 LLM 추론 (GGUF 모델)
sentence-transformers  # 텍스트 임베딩 (BERT 계열)
torch                  # PyTorch (Whisper, sentence-transformers 공통 의존성)
pypdf                  # PDF 텍스트 추출
websockets            # WebSocket 서버
```

---

## 마이그레이션 가능 영역

### ✅ 상대적으로 쉬운 영역

#### 1. HTTP/WebSocket 서버 (난이도: ★☆☆☆☆)
- **현재**: `http.server.ThreadingHTTPServer` + `websockets`
- **Rust 대안**:
  - `axum` / `actix-web`: HTTP 서버 (고성능, 비동기)
  - `tokio-tungstenite` / `axum::extract::ws`: WebSocket
- **장점**:
  - Rust의 비동기 런타임 (tokio)은 Python보다 훨씬 빠름
  - 메모리 안전성 보장
  - 타입 안전한 라우팅

#### 2. 파일 처리 및 관리 (난이도: ★☆☆☆☆)
- **현재**: `pathlib`, `shutil`, 파일 업로드/다운로드 로직
- **Rust 대안**:
  - 표준 라이브러리 (`std::fs`, `std::path`)
  - `tokio::fs` (비동기 파일 I/O)
  - `multipart` (파일 업로드 파싱)
- **장점**:
  - 표준 라이브러리만으로 충분히 구현 가능
  - 컴파일 타임에 경로 오류 방지 가능

#### 3. 설정 관리 (난이도: ★☆☆☆☆)
- **현재**: `config.py` (환경변수, .env 파일 읽기)
- **Rust 대안**:
  - `serde` + `toml` / `dotenv`: 설정 직렬화
  - `config` crate: 다층 설정 관리
- **장점**:
  - 타입 안전한 설정 구조체
  - 컴파일 타임 검증

#### 4. JSON/데이터 처리 (난이도: ★☆☆☆☆)
- **현재**: `json` 모듈, 히스토리 파일 관리
- **Rust 대안**:
  - `serde_json`: JSON 직렬화/역직렬화
- **장점**:
  - 타입 안전성
  - 성능 우수

#### 5. FFmpeg 연동 (난이도: ★★☆☆☆)
- **현재**: `subprocess`로 ffmpeg 실행
- **Rust 대안**:
  - `std::process::Command` (subprocess 동일)
  - 또는 `ffmpeg-next` crate (FFmpeg C 라이브러리 바인딩)
- **장점**:
  - 프로세스 관리 더 안전하고 명확

---

## 마이그레이션 어려운/불가능 영역

### ⚠️ 난이도가 높은 영역

#### 1. STT - Whisper 모델 (난이도: ★★☆☆☆)
- **현재**: `openai-whisper` (Python 전용 라이브러리 + PyTorch)
- **채택 방안**: **whisper.cpp 사용** (확정)
  - ✅ **whisper.cpp**
    - C++ 구현 (llama.cpp와 동일한 저자 ggerganov)
    - 검증된 성능과 안정성
    - GGML 모델 포맷 지원 (양자화 가능)
  - ✅ **Rust 통합 옵션**:
    - **Option 1**: whisper.cpp를 subprocess로 실행 (가장 간단)
    - **Option 2**: [whisper-rs](https://github.com/tazz4843/whisper-rs) 바인딩 사용 (Rust 네이티브 통합)
    - **Option 3**: FFI로 직접 호출 (고급 사용자용)

**장점**:
- llama.cpp와 동일한 생태계 (일관된 모델 관리)
- C++ 네이티브 속도, 메모리 효율적
- 크로스 플랫폼 지원 우수
- 양자화 모델로 배포 크기 감소

**단점**:
- 모델 변환 필요 (PyTorch → GGML)
- 양자화 시 정확도 미세 감소 가능 (실사용에서는 무시 가능)

#### 2. 텍스트 임베딩 - sentence-transformers (난이도: ★★☆☆☆)
- **현재**: `sentence-transformers` (HuggingFace transformers + PyTorch)
- **채택 방안**: **llama.cpp 사용** (확정)
  - ✅ **llama.cpp 임베딩 지원**
    - llama.cpp는 LLM 추론뿐만 아니라 **임베딩 모델도 지원**
    - GGUF 포맷의 임베딩 모델 사용 가능
    - 예: `nomic-embed-text`, `all-MiniLM-L6-v2` 등을 GGUF로 변환하여 사용
  - ✅ **Rust 통합**:
    - `llama-cpp-rs` 바인딩으로 임베딩 생성 API 호출
    - LLM과 임베딩을 동일한 라이브러리로 통합 관리

**장점**:
- **일관된 기술 스택**: LLM과 임베딩 모두 llama.cpp로 통합
- 단일 라이브러리 의존성 (빌드 복잡도 감소)
- GGUF 모델 생태계 활용 (허깅페이스 모델 변환 도구 풍부)
- 메모리 효율적 (양자화 지원)

**참고**:
- llama.cpp 임베딩 예제: https://github.com/ggerganov/llama.cpp/tree/master/examples/embedding
- GGUF 변환 도구: `convert.py` (llama.cpp 제공)

#### 3. LLM 추론 - llama.cpp (난이도: ★☆☆☆☆)
- **현재**: `llama-cpp-python` (Python 바인딩)
- **채택 방안**: **llama-cpp-rs 사용** (확정)
  - ✅ **llama-cpp-rs**
    - [llama-cpp-rs](https://github.com/utilityai/llama-cpp-rs): Rust 바인딩
    - llama.cpp C++ 라이브러리를 FFI로 호출
    - 가장 안정적이고 llama.cpp와 1:1 호환

**장점**:
- 현재 사용 중인 GGUF 모델 **그대로 사용 가능** (마이그레이션 위험 최소)
- llama.cpp 최신 기능 즉시 사용 가능
- GPU 가속 지원 (CUDA, Metal, ROCm)
- 검증된 성능과 안정성

**단점**:
- FFI 오버헤드 (실제로는 무시할 수 있는 수준)

#### 4. PDF 처리 - pypdf (난이도: N/A)
- **현재**: `pypdf` (PDF 텍스트 추출)
- **채택 방안**: **기능 삭제** (확정)
  - ❌ **PDF 처리 기능 제거**
    - 현재 사용 빈도가 낮거나 핵심 기능이 아님
    - 마이그레이션 복잡도 감소를 위해 제거
  - 🔮 **추후 필요시 재추가 옵션**:
    - C++ 기반 PDF 라이브러리 (예: poppler, mupdf) FFI 바인딩
    - 또는 Rust PDF 라이브러리 (`lopdf`, `pdf-extract`) 사용

**장점**:
- 마이그레이션 범위 축소
- 의존성 감소
- 핵심 기능(STT, LLM, 임베딩)에 집중

---

## 마이그레이션 단계별 전략

### 전략 A: 점진적 마이그레이션 (권장)

#### Phase 1: 서버 인프라 교체
- **목표**: HTTP/WebSocket 서버를 Rust로 교체
- **기간**: 2-4주
- **작업**:
  1. `axum` 기반 HTTP 서버 구축
  2. WebSocket 실시간 업데이트 재구현
  3. 파일 업로드/다운로드 엔드포인트
  4. Python 기존 로직을 subprocess로 호출 (임시)
- **결과**: Rust 서버 + Python Worker 하이브리드

#### Phase 2: 간단한 기능부터 Rust로 전환
- **목표**: 파일 관리, 설정, JSON 처리 등 Rust로 재작성
- **기간**: 1-2주
- **작업**:
  1. `config.py` → Rust config 모듈
  2. 파일 레지스트리 관리
  3. 히스토리 관리
  4. PDF 처리 기능 제거 (코드 정리)

#### Phase 3: ML 모델 추론 전환
- **목표**: LLM, STT, 임베딩을 llama.cpp/whisper.cpp로 교체
- **기간**: 3-6주
- **작업**:
  1. **LLM 추론**: `llama-cpp-rs`로 교체 (가장 쉬움, 1-2주)
  2. **임베딩**: `llama-cpp-rs` 임베딩 API 사용 (1주)
  3. **STT**: `whisper.cpp` 통합 (subprocess 또는 `whisper-rs`, 1-3주)
  4. 모델 변환: PyTorch 모델 → GGUF/GGML 포맷

#### Phase 4: Python 완전 제거
- **목표**: 모든 Python 의존성 제거
- **기간**: 2-4주
- **작업**:
  1. 남은 Python 코드 재작성
  2. 통합 테스트
  3. 성능 벤치마크
  4. PyInstaller 제거, 단일 Rust 바이너리로 배포

### 전략 B: 하이브리드 유지 (현실적 대안)

Python을 **완전히 제거하지 않고** 다음 구조로 유지:

```
Rust Main Server (HTTP/WebSocket)
    ↓
    ├─ Rust: 파일 관리, 설정, JSON
    ├─ llama.cpp (llama-cpp-rs): LLM 추론 + 임베딩
    └─ Python Worker: STT만 유지 (subprocess)
```

**장점**:
- 개발 속도 빠름 (복잡한 ML 부분은 Python 유지)
- 검증된 라이브러리 사용 (openai-whisper, sentence-transformers)

**단점**:
- Python 런타임 여전히 필요
- 배포 크기 감소 효과 제한적

---

## 기술 스택 매핑

| 현재 (Python) | 채택 방안 | 난이도 | 비고 |
|--------------|-----------|--------|------|
| `http.server` | `axum` | ★☆☆☆☆ | 비동기 HTTP 서버 |
| `websockets` | `axum::extract::ws` | ★☆☆☆☆ | axum 내장 WebSocket |
| `pathlib`, `shutil` | `std::fs`, `tokio::fs` | ★☆☆☆☆ | 표준 라이브러리 |
| `json` | `serde_json` | ★☆☆☆☆ | 타입 안전 직렬화 |
| `dotenv` | `dotenv` (Rust) | ★☆☆☆☆ | 동일 기능 |
| `subprocess` | `std::process::Command` | ★☆☆☆☆ | 더 안전한 프로세스 관리 |
| `llama-cpp-python` | `llama-cpp-rs` | ★☆☆☆☆ | **확정**: FFI 바인딩 |
| `openai-whisper` | `whisper.cpp` + `whisper-rs` | ★★☆☆☆ | **확정**: C++ + Rust 바인딩 |
| `sentence-transformers` | `llama-cpp-rs` (임베딩) | ★★☆☆☆ | **확정**: llama.cpp 임베딩 기능 |
| `pypdf` | **삭제** | - | **확정**: 기능 제거 |
| `torch` | ❌ | - | 필요 없음 (llama.cpp/whisper.cpp로 대체) |

---

## 위험 요소 및 완화 전략

### 위험 1: ML 모델 정확도 하락
- **원인**: whisper.cpp, ONNX 변환 시 양자화로 정확도 저하 가능
- **완화**:
  - 먼저 벤치마크 테스트 (Python vs Rust 정확도 비교)
  - 양자화 레벨 조정 (FP16 → INT8 단계적 테스트)
  - 중요 기능은 Python 유지 옵션 보유

### 위험 2: 개발 기간 과소평가
- **원인**: Rust ML 생태계가 Python보다 미성숙
- **완화**:
  - Phase 1-2만 먼저 진행 (서버 인프라)
  - ML 부분은 Python subprocess 유지
  - 점진적 전환 (Phase 3-4는 선택적)

### 위험 3: 크로스 플랫폼 이슈
- **원인**: whisper.cpp, llama.cpp FFI는 플랫폼별 빌드 복잡
- **완화**:
  - CI/CD 파이프라인 구축 (GitHub Actions)
  - 플랫폼별 테스트 환경 (Windows, macOS, Linux)

### 위험 4: 팀 Rust 학습 곡선
- **원인**: Rust는 Python보다 배우기 어려움
- **완화**:
  - 코드 리뷰 및 문서화 강화
  - Rust 베스트 프랙티스 가이드 공유
  - 간단한 모듈부터 시작

---

## 결론 및 권고사항

### 최종 권고: 단계적 하이브리드 접근

1. **즉시 시작 가능 (Phase 1-2)**:
   - Rust로 HTTP/WebSocket 서버 재작성
   - 파일 관리, 설정, JSON 처리 Rust로 전환
   - **예상 효과**: 서버 응답 속도 30-50% 향상, 메모리 사용량 감소

2. **중기 목표 (Phase 3)**:
   - LLM 추론 + 임베딩을 `llama-cpp-rs`로 전환
   - STT는 `whisper.cpp`로 전환
   - **예상 효과**: ML 추론 속도 20-40% 향상, 메모리 사용량 30% 감소

3. **장기 목표 (Phase 4)**:
   - 모든 ML 모델을 C++ 라이브러리로 완전 전환
   - Python 완전 제거
   - **예상 효과**: 단일 Rust 바이너리 배포, Python 런타임 불필요, 배포 크기 50% 감소

### 구체적 다음 단계

1. **개념 증명 (PoC)**: 1-2주
   - `axum`으로 간단한 HTTP 서버 구축
   - 기존 Python을 subprocess로 호출하는 `/process` 엔드포인트
   - 성능 비교 (Python 단독 vs Rust+Python)

2. **PoC 성공 시**: Phase 1 본격 진행
   - Electron과의 통합 테스트
   - 모든 API 엔드포인트 Rust로 재작성

3. **Phase 1 완료 후 평가**:
   - 성능 개선 측정
   - Phase 2-3 진행 여부 결정

### 최소 목표 vs 최대 목표

- **최소 목표** (Phase 1-2): Rust 서버 + Python ML Worker
  - Python 의존성 40% 감소
  - 배포는 여전히 Python 런타임 필요
  - 개발 기간: 1-2개월

- **중간 목표** (Phase 1-3): Rust + llama.cpp/whisper.cpp
  - Python 의존성 90% 제거 (PyTorch 완전 제거)
  - 단일 바이너리 + 모델 파일로 배포
  - 개발 기간: 3-4개월

- **최대 목표** (Phase 1-4): 100% Rust + C++ 라이브러리
  - Python 완전 제거
  - 단일 바이너리 배포
  - 개발 기간: 4-6개월

**권장**: 중간 목표(Phase 1-3)를 목표로 시작, llama.cpp/whisper.cpp 생태계 활용

---

## 참고 자료

### 핵심 의존성 (확정)
- **HTTP 서버**: [axum](https://github.com/tokio-rs/axum)
- **LLM + 임베딩**: [llama-cpp-rs](https://github.com/utilityai/llama-cpp-rs)
- **STT**: [whisper-rs](https://github.com/tazz4843/whisper-rs) (옵션)

### C++ 라이브러리
- [llama.cpp](https://github.com/ggerganov/llama.cpp): LLM 추론 + 임베딩
- [whisper.cpp](https://github.com/ggerganov/whisper.cpp): STT (음성→텍스트)

### 보조 Crates
- JSON: `serde_json`
- 설정: `dotenv`, `config`
- 파일 처리: `tokio::fs`, `multipart`

### 학습 자료
- [Rust Book](https://doc.rust-lang.org/book/)
- [Async Rust](https://rust-lang.github.io/async-book/)
- [Axum Tutorial](https://github.com/tokio-rs/axum/tree/main/examples)
