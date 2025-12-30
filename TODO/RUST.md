# RecordRoute Rust 전환 로드맵

## 전환 목표
Python 코드베이스를 점진적으로 Rust로 전환하여 성능 향상, 메모리 안전성 확보, 학습 목적 달성

## 전환 전략: 하이브리드 접근 (Incremental Migration)

ML/AI 라이브러리 의존성이 높은 부분은 Python 유지하고, 성능이 중요한 핵심 인프라를 Rust로 전환

### 아키텍처 개요
```
┌─────────────────────────────────────────┐
│         Rust Core (High Performance)     │
│  ┌─────────────────────────────────────┐ │
│  │  HTTP/WebSocket Server (actix-web)  │ │
│  │  - 요청 라우팅                       │ │
│  │  - 파일 업로드/다운로드              │ │
│  │  - 실시간 진행상황 스트리밍          │ │
│  └─────────────────────────────────────┘ │
│  ┌─────────────────────────────────────┐ │
│  │  Vector Search Engine (ndarray)     │ │
│  │  - 코사인 유사도 계산                │ │
│  │  - 인덱스 관리                       │ │
│  │  - 검색 캐싱                         │ │
│  └─────────────────────────────────────┘ │
│  ┌─────────────────────────────────────┐ │
│  │  File I/O & Storage                 │ │
│  │  - 고성능 파일 처리                  │ │
│  │  - 데이터베이스 관리                 │ │
│  │  - 로깅 시스템                       │ │
│  └─────────────────────────────────────┘ │
└──────────────┬──────────────────────────┘
               │ PyO3 FFI
               ▼
┌─────────────────────────────────────────┐
│      Python ML/AI Layer (Keep)          │
│  ┌─────────────────────────────────────┐ │
│  │  STT Engine (OpenAI Whisper)        │ │
│  │  - 음성 인식                         │ │
│  │  - 전처리/후처리                     │ │
│  └─────────────────────────────────────┘ │
│  ┌─────────────────────────────────────┐ │
│  │  LLM Integration (Ollama)           │ │
│  │  - 텍스트 요약                       │ │
│  │  - 임베딩 생성                       │ │
│  └─────────────────────────────────────┘ │
└─────────────────────────────────────────┘
```

---

## Phase 1: 기반 인프라 구축 (2-3주)

### 1.1 프로젝트 초기 설정
- [ ] Cargo 프로젝트 생성 (`recordroute-rs/`)
- [ ] 디렉토리 구조 설계
  ```
  recordroute-rs/
  ├── Cargo.toml
  ├── src/
  │   ├── main.rs
  │   ├── lib.rs
  │   ├── server/          # HTTP/WebSocket 서버
  │   ├── vector/          # 벡터 검색
  │   ├── storage/         # 파일 I/O
  │   ├── config/          # 설정 관리
  │   └── python_bridge/   # PyO3 브릿지
  └── tests/
  ```
- [ ] 핵심 의존성 추가
  ```toml
  [dependencies]
  actix-web = "4"
  actix-files = "0.6"
  actix-ws = "0.2"
  tokio = { version = "1", features = ["full"] }
  serde = { version = "1", features = ["derive"] }
  serde_json = "1"
  pyo3 = { version = "0.20", features = ["auto-initialize"] }
  ndarray = "0.15"
  uuid = { version = "1", features = ["v4", "serde"] }
  dotenv = "0.15"
  tracing = "0.1"
  tracing-subscriber = "0.3"
  ```

### 1.2 설정 시스템 (config.py → config.rs)
- [ ] 환경변수 로딩 (`dotenv`)
- [ ] 경로 정규화 함수들
  ```rust
  pub fn get_db_base_path() -> PathBuf
  pub fn normalize_db_record_path(path: &str, base_dir: &Path) -> String
  pub fn resolve_db_path(alias: &str) -> Result<PathBuf>
  ```
- [ ] 모델 설정 관리
- [ ] 단위 테스트 작성

### 1.3 로깅 시스템 (logger.py → logger.rs)
- [ ] `tracing` 기반 구조화 로깅
- [ ] 파일 기반 로그 저장 (`db/log/`)
- [ ] 로그 레벨 설정 (환경변수)
- [ ] 로그 로테이션 구현 (선택사항)

---

## Phase 2: HTTP/WebSocket 서버 (3-4주)

### 2.1 기본 HTTP 서버 (server.py → server.rs)
- [ ] Actix-web 서버 구조 설계
- [ ] 정적 파일 서빙 (`/`, `/frontend/*`)
- [ ] CORS 설정
- [ ] 에러 핸들링 미들웨어

### 2.2 REST API 엔드포인트
- [ ] `POST /upload` - 파일 업로드
  - Multipart form data 처리
  - UUID 기반 파일명 생성
  - 임시 저장소 관리
- [ ] `GET /history` - 작업 기록 조회
  - JSON 파일 파싱 (`db/upload_history.json`)
  - 정렬 및 필터링
- [ ] `GET /download/<uuid>` - 결과 파일 다운로드
  - 경로 검증 및 보안 체크
  - 스트리밍 응답
- [ ] `POST /delete_records` - 기록 삭제
  - 파일 시스템 정리
  - 히스토리 JSON 업데이트
- [ ] `POST /update_stt_text` - 텍스트 수정
  - 파일 원자적 쓰기
- [ ] `GET /models` - Ollama 모델 목록
  - Python 브릿지 호출

### 2.3 WebSocket 실시간 통신
- [ ] WebSocket 핸들러 구현
  - 클라이언트 연결 관리
  - 작업 진행상황 브로드캐스트
- [ ] 작업 상태 공유 (`Arc<Mutex<JobState>>`)
- [ ] 취소 토큰 구현 (`CancellationToken`)

### 2.4 작업 처리 파이프라인
- [ ] `POST /process` - 워크플로우 실행
  - Python 함수 호출 (PyO3)
    - `transcribe_audio_files()`
    - `summarize_text_mapreduce()`
    - `embed_text_ollama()`
  - 비동기 작업 관리 (`tokio::spawn`)
  - 진행률 업데이트 (WebSocket)
  - 에러 복구 및 재시도

---

## Phase 3: 벡터 검색 엔진 (2-3주)

### 3.1 벡터 연산 (vector_search.py → vector.rs)
- [ ] NumPy → `ndarray` 전환
- [ ] 코사인 유사도 계산 최적화
  ```rust
  pub fn cosine_similarity(a: &Array1<f32>, b: &Array1<f32>) -> f32
  ```
- [ ] SIMD 최적화 검토 (선택사항)

### 3.2 인덱스 관리 (embedding_pipeline.py 일부)
- [ ] 인덱스 로드/저장
  - JSON 파싱 (`db/vector_index.json`)
  - NPY 파일 로드 (`ndarray-npy` 크레이트)
- [ ] 벡터 파일 관리
  ```rust
  pub struct VectorIndex {
      entries: HashMap<String, VectorEntry>,
  }

  pub struct VectorEntry {
      vector_path: PathBuf,
      timestamp: DateTime<Utc>,
      deleted: bool,
  }
  ```
- [ ] 동시성 안전 접근 (`RwLock<VectorIndex>`)

### 3.3 검색 기능 (vector_search.py)
- [ ] `search()` 함수 구현
  - 날짜 필터링
  - Top-K 정렬
  - 캐시 통합
- [ ] `find_similar()` 함수 구현
  - 문서 간 유사도 계산
- [ ] 성능 벤치마크 (Python vs Rust)

### 3.4 검색 캐싱 (search_cache.py → cache.rs)
- [ ] 캐시 키 생성 (해시 기반)
- [ ] TTL 기반 만료 관리
- [ ] 캐시 통계 추적
- [ ] 주기적 정리 태스크

---

## Phase 4: 파일 I/O 및 스토리지 (1-2주)

### 4.1 파일 처리 유틸리티
- [ ] 원자적 파일 쓰기
  ```rust
  pub fn write_atomic(path: &Path, content: &str) -> Result<()>
  ```
- [ ] 안전한 파일 삭제
- [ ] 디렉토리 순회 최적화

### 4.2 히스토리 관리
- [ ] `upload_history.json` 파서
- [ ] 히스토리 레코드 CRUD
- [ ] JSON 직렬화/역직렬화 (`serde`)

### 4.3 임시 파일 관리
- [ ] 업로드 임시 저장소
- [ ] 자동 정리 스케줄러
- [ ] 디스크 공간 모니터링 (선택사항)

---

## Phase 5: Python 통합 (PyO3 Bridge) (2-3주)

### 5.1 Whisper STT 브릿지
- [ ] Python 함수 래퍼
  ```rust
  pub fn transcribe_audio(
      file_path: &Path,
      model: &str,
  ) -> PyResult<String>
  ```
- [ ] 에러 핸들링 (Python 예외 → Rust Result)
- [ ] 진행률 콜백 구현

### 5.2 Ollama LLM 브릿지
- [ ] 요약 함수 래퍼
  ```rust
  pub fn summarize_text(
      text: &str,
      model: &str,
      chunk_size: usize,
  ) -> PyResult<String>
  ```
- [ ] 임베딩 생성 래퍼
  ```rust
  pub fn generate_embedding(
      text: &str,
      model: &str,
  ) -> PyResult<Vec<f32>>
  ```

### 5.3 Python 런타임 관리
- [ ] GIL 최적화 (`allow_threads`)
- [ ] Python 인터프리터 초기화
- [ ] 메모리 관리 및 누수 방지

---

## Phase 6: 테스트 및 최적화 (진행 중)

### 6.1 단위 테스트
- [ ] 각 모듈별 테스트 커버리지 > 80%
- [ ] 통합 테스트 (서버 API)
- [ ] Python 브릿지 테스트

### 6.2 성능 최적화
- [ ] 프로파일링 (`cargo flamegraph`)
- [ ] 병목 지점 식별 및 개선
- [ ] 메모리 사용량 최적화

### 6.3 벤치마크
- [ ] Python vs Rust 성능 비교
  - HTTP 요청 처리 속도
  - 벡터 검색 속도
  - 파일 I/O 속도
- [ ] 동시성 부하 테스트

---

## Phase 7: 배포 및 통합 (1-2주)

### 7.1 빌드 시스템
- [ ] 크로스 플랫폼 빌드 설정
  - Linux (x86_64, aarch64)
  - macOS (Intel, Apple Silicon)
  - Windows
- [ ] Cargo 릴리스 프로파일 최적화
  ```toml
  [profile.release]
  opt-level = 3
  lto = true
  codegen-units = 1
  strip = true
  ```

### 7.2 Python 번들링
- [ ] PyO3 런타임 임베딩
- [ ] Python 가상환경 패키징
- [ ] 의존성 번들링 (`maturin` 검토)

### 7.3 Electron 통합
- [ ] Rust 백엔드 바이너리 통합
- [ ] `electron/main.js` 수정 (Rust 실행파일 호출)
- [ ] 빌드 스크립트 업데이트

---

## 유지 관리 항목 (Python → Keep)

다음 부분은 Python으로 유지하고 PyO3로 호출:

- `workflow/transcribe.py` - Whisper STT (OpenAI 라이브러리 의존)
- `workflow/summarize.py` - Ollama 요약 (HTTP 클라이언트는 Rust로 가능하지만 복잡도 고려)
- `embedding_pipeline.py` - 임베딩 생성 (sentence-transformers 의존)
- `ollama_utils.py` - Ollama 서버 관리
- `llamacpp_utils.py` - llama.cpp 통합
- `keyword_frequency.py` - 키워드 분석 (간단하지만 우선순위 낮음)
- `one_line_summary.py` - 한 줄 요약 (우선순위 낮음)

---

## 학습 체크포인트

### 러스트 핵심 개념 습득
- [ ] 소유권 & 차용 (Ownership & Borrowing)
- [ ] 라이프타임 (Lifetimes)
- [ ] 트레잇 & 제네릭 (Traits & Generics)
- [ ] 에러 핸들링 (`Result`, `Option`)
- [ ] 비동기 프로그래밍 (`async`/`await`, Tokio)
- [ ] 스마트 포인터 (`Arc`, `Mutex`, `RwLock`)
- [ ] 패턴 매칭
- [ ] 매크로 (선택사항)

### 생태계 숙달
- [ ] Actix-web (웹 프레임워크)
- [ ] Serde (직렬화)
- [ ] Tokio (비동기 런타임)
- [ ] PyO3 (Python 통합)
- [ ] ndarray (과학 계산)
- [ ] Tracing (로깅)

---

## 예상 타임라인

| Phase | 기간 | 누적 기간 |
|-------|------|-----------|
| Phase 1: 기반 인프라 | 2-3주 | 3주 |
| Phase 2: HTTP/WebSocket 서버 | 3-4주 | 7주 |
| Phase 3: 벡터 검색 엔진 | 2-3주 | 10주 |
| Phase 4: 파일 I/O | 1-2주 | 12주 |
| Phase 5: Python 통합 | 2-3주 | 15주 |
| Phase 6: 테스트 & 최적화 | 지속적 | - |
| Phase 7: 배포 | 1-2주 | 17주 |

**총 예상 기간**: 약 4-5개월 (파트타임 기준)

---

## 성공 지표

### 성능 목표
- HTTP 요청 처리: Python 대비 2-5배 빠름
- 벡터 검색: Python 대비 5-10배 빠름
- 메모리 사용량: 30-50% 감소
- 바이너리 크기: 단일 실행파일 < 50MB

### 품질 목표
- 테스트 커버리지 > 80%
- Clippy 경고 0개
- 메모리 누수 없음 (Valgrind 검증)
- 동시성 안전성 검증

---

## 리스크 및 대응 방안

### 리스크 1: PyO3 통합 복잡도
- **대응**: 초기에 간단한 함수부터 시작, 점진적 확장
- **플랜 B**: Python 서버를 별도 프로세스로 실행하고 IPC 사용

### 리스크 2: ML 라이브러리 호환성
- **대응**: Python 레이어 유지, 필요 시 Rust ML 라이브러리 검토 (`tract`, `burn`)
- **플랜 B**: gRPC/HTTP로 Python 서비스와 통신

### 리스크 3: 개발 시간 초과
- **대응**: MVP 우선 (Phase 1-3만 완료해도 학습 목표 달성)
- **조정**: 일부 Phase 생략 가능 (Phase 4, 7)

---

## 참고 자료

### 공식 문서
- [Rust Book](https://doc.rust-lang.org/book/)
- [Actix-web 가이드](https://actix.rs/docs/)
- [PyO3 가이드](https://pyo3.rs/)
- [Tokio 튜토리얼](https://tokio.rs/tokio/tutorial)

### 유사 프로젝트
- [tantivy](https://github.com/quickwit-oss/tantivy) - Rust 풀텍스트 검색
- [qdrant](https://github.com/qdrant/qdrant) - Rust 벡터 검색 엔진
- [ruff](https://github.com/astral-sh/ruff) - Python 린터 (Rust 구현)

### 블로그 포스트
- ["Rewriting Python in Rust"](https://www.lpalmieri.com/posts/2019-12-01-taking-ml-to-production-with-rust-a-25x-speedup/)
- ["Building a Python Extension in Rust"](https://blog.yossarian.net/2020/08/02/Writing-and-publishing-a-python-module-in-rust)

---

## 다음 스텝

1. **Phase 1.1 시작**: Cargo 프로젝트 생성 및 디렉토리 구조 설계
2. **간단한 예제 작성**: "Hello World" HTTP 서버로 Actix-web 학습
3. **작은 모듈부터**: `config.rs`, `logger.rs` 완성
4. **점진적 확장**: 작동하는 부분부터 천천히 확장

시작하시겠습니까? 🦀
