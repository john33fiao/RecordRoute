# Rust/C++ 백엔드 재작성 WBS (Work Breakdown Structure)

## 작업 로그

- 2026-02-22: 착수 시작
  - `docs/architecture.md` 신규 작성 (단일 진입점/엔진 경계/포트 고정/큐 분리 원칙 명시)
  - `docs/rust-cpp-backend-rewrite-plan.md`에 기준 문서 링크 추가
  - 다음 우선 작업: WBS 1.2.x ~ 2.2.x 산출물(배포/자산 정책) 구체화

## WBS 1.0 전략/기반 정합성

### 1.1 아키텍처 기준 수립
- [ ] WBS 1.1.1: Rust 단일 진입점 정책 확정
  - 산출물: `docs/architecture.md`에 Rust가 `:18000`에서 모든 외부 트래픽 수신, 오케스트레이션/상태관리/배압 정책을 담당한다고 명시
  - 완료 조건: 기존 라우팅/클라이언트 유입 경로가 1개로 정리되고 변경 동의자 승인
- [ ] WBS 1.1.2: 엔진 실행 경계 규칙 확정
  - 산출물: `llama.cpp`, `whisper.cpp`는 프로세스 분리 + `127.0.0.1` HTTP 계약 준수 규칙 문서화
  - 완료 조건: Rust가 엔진 내부 호출을 직접 수행하지 않는다는 점이 리뷰에서 합의

### 1.2 구성요소 분리
- [ ] WBS 1.2.1: `llama-text`와 `llama-embed` 역할 분리 아키텍처 확정
  - 산출물: llama-text `/models/text`, llama-embed `/models/embed` 모델 경로와 라우팅 책임 분리 설계
  - 완료 조건: 요약/임베딩 요청이 동일 프로세스에서 처리되지 않음
- [ ] WBS 1.2.2: Swagger 분리 운영 방식 확정
  - 산출물: API(18000)와 Swagger(14000) 분리된 배포 단위 정의
  - 완료 조건: Swagger 장애 시에도 `/healthz`/`/readyz`가 API 가용성에 영향 없음

## WBS 2.0 환경/자산 표준 구축

### 2.1 포트 및 네트워크 규칙
- [ ] WBS 2.1.1: API/문서 기본 포트 고정
  - 산출물: `:18000`(Rust), `:14000`(Swagger) 고정
  - 완료 조건: README/런치 스크립트/배포 문서에 동일 포트 값이 일치
- [ ] WBS 2.1.2: 엔진 포트 고정
  - 산출물: `18101(llama-text)`, `18102(llama-embed)`, `18103(whisper-server)` 고정
  - 완료 조건: healthcheck가 각 엔드포인트에 대해 개별 접속 가능
- [ ] WBS 2.1.3: 내부 네트워크 바인딩 강제
  - 산출물: 각 엔진 실행 옵션에 `--host 127.0.0.1` 적용
  - 완료 조건: 외부 노출 포트 스캔에서 엔진 포트가 직접 공개되지 않음

### 2.2 폴더/자산 정책
- [ ] WBS 2.2.1: 저장소 구조 고정화
  - 산출물: `Cargo.toml`, `/src`, `/vendor`, `/models` 중심 구조 확정
  - 완료 조건: 기준 트리에서 누락/중복 디렉터리 없이 1회 정렬
- [ ] WBS 2.2.2: 벤더·모델 산출물 정책 적용
  - 산출물: `/vendor` 소스만 유지, `/models` 원본 미커밋, `manifest.yml/json`만 버전관리
  - 완료 조건: `.gitignore`에 `/build`, `/target`, 바이너리, 모델 원본 규칙 반영

## WBS 3.0 엔진별 도메인 기능 구현

### 3.1 llama-text
- [ ] WBS 3.1.1: llama-text 모델/진입점 고정
  - 산출물: `/models/text/*.gguf` 사용 라우팅
  - 완료 조건: 채팅/요약 요청만 llama-text로 전달
- [ ] WBS 3.1.2: summarize 큐 라우팅 반영
  - 산출물: Rust 라우팅 레이어에서 `summarize_queue` 경유
  - 완료 조건: 요약 요청이 stt/embed 큐로 유입되지 않음

### 3.2 llama-embed
- [ ] WBS 3.2.1: 임베딩 엔진 분리
  - 산출물: `/v1/embeddings` 전용 엔드포인트 유지
  - 완료 조건: `/models/embed/*.gguf`와 pooling 정책 지정값(`mean`/`cls`)으로 고정
- [ ] WBS 3.2.2: 재현성 확보
  - 산출물: llama-embed 실행 플래그/버전 pinning
  - 완료 조건: 동일 입력/동일 플래그에서 동일한 메타 출력

### 3.3 whisper-server
- [ ] WBS 3.3.1: STT 모델 및 업로드 계약 확정
  - 산출물: `/models/stt/*` 경로 적용, HTTP 업로드 기반 STT 엔드포인트 계약
  - 완료 조건: 업로드 입력 유효성 검사와 실패 코드가 문서화
- [ ] WBS 3.3.2: stt 라우팅 반영
  - 산출물: Rust에서 `stt_queue`를 통한 전달
  - 완료 조건: whisper-server는 추론 외 기능을 수행하지 않음

## WBS 4.0 큐/동시성 및 처리량 제어

### 4.1 큐 설계
- [ ] WBS 4.1.1: stt 큐 구현
  - 산출물: bounded `stt_queue`
  - 완료 조건: 큐 포화 시 429/메트릭 분기 동작 가능
- [ ] WBS 4.1.2: summarize 큐 구현
  - 산출물: bounded `summarize_queue`
  - 완료 조건: 요약 요청이 STT/임베딩 큐를 점유하지 않음
- [ ] WBS 4.1.3: embed 큐 구현
  - 산출물: bounded `embed_queue`
  - 완료 조건: 임베딩 요청 독립 처리 경로 보장

### 4.2 동시성 제어
- [ ] WBS 4.2.1: 큐별 워커/세마포어 설계
  - 산출물: 큐별 워커 수 + 엔진별 semaphore 정책
  - 완료 조건: `stt_concurrency`, `summarize_concurrency`, `embed_concurrency`가 운영 파라미터로 설정됨
- [ ] WBS 4.2.2: 과부하 분류
  - 산출물: 큐 포화/엔진 포화 이벤트 분리 라벨링
  - 완료 조건: 관측지표에서 둘의 원인 추적이 구분됨

## WBS 5.0 타임아웃/에러 계약

### 5.1 처리량 파라미터
- [ ] WBS 5.1.1: timeout 설정 항목 정리
  - 산출물: `stt_rtf`, `summary_tps`, `embed_tps/chars_per_sec`, `min_timeout_sec`, `max_timeout_sec`, `safety_buffer_sec`
  - 완료 조건: 모든 값이 config 파일 또는 env 기반으로 외부화

### 5.2 계산식 구현
- [ ] WBS 5.2.1: STT timeout 산정
  - 산출물: `clamp(audio_sec * stt_rtf + buffer, min, max)` 반영
  - 완료 조건: 긴 오디오에서 오버/언더 타임아웃 오탐이 제한적
- [ ] WBS 5.2.2: 요약 timeout 산정
  - 산출물: `clamp(predicted_tokens / summary_tps + buffer, min, max)` 반영
  - 완료 조건: 토큰 추정치 기반으로 정합성 있는 타임아웃 산정
- [ ] WBS 5.2.3: 임베딩 timeout 산정
  - 산출물: `clamp(input_size / embed_rate + buffer, min, max)` 반영
  - 완료 조건: 큰 입력에서 타임아웃 증가 추적 로그 남김

### 5.3 레이어 분리
- [ ] WBS 5.3.1: HTTP/Job timeout 분리
  - 산출물: 연결/응답 타임아웃과 작업 총예산 timeout 분리
  - 완료 조건: 오류 응답이 timeout 원인(네트워크 vs 작업)별로 분류

## WBS 6.0 오디오 처리 표준화

### 6.1 Rust 전처리 고정
- [ ] WBS 6.1.1: Symphonia 기반 정규화
  - 산출물: 16kHz, 16-bit mono WAV 변환 파이프라인
  - 완료 조건: whisper 입력 형식 편차가 일관되게 보정됨
- [ ] WBS 6.1.2: 길이 추출 로직 구현
  - 산출물: Rust 내부 메타데이터/샘플 기반 오디오 길이 계산
  - 완료 조건: STT timeout 계산에 길이 추정값 사용

### 6.2 외부 의존 제거
- [ ] WBS 6.2.1: ffmpeg/ffprobe 의존 배제
  - 산출물: whisper-server 기본 경로에서 `--convert` 의존 제거
  - 완료 조건: 전처리 실패 원인이 엔진 외부 툴에 의해 좌우되지 않음

## WBS 7.0 프로세스 슈퍼비전

### 7.1 시작 시퀀스
- [ ] WBS 7.1.1: 기동 전 검증
  - 산출물: 포트 점유/경로/실행파일 존재 점검 모듈
  - 완료 조건: 조건 불충족 시 기동 실패 및 에러 리포트
- [ ] WBS 7.1.2: 자식 프로세스 기동
  - 산출물: child spawn와 초기 health gate
  - 완료 조건: 모든 엔진 health 통과 전 `readyz=false`

### 7.2 운영 중 감독
- [ ] WBS 7.2.1: 종료 감지 및 재시작
  - 산출물: child 종료 감지 + 제한 backoff 재시작
  - 완료 조건: 반복 재시작 시 degraded 전환 조건 기록
- [ ] WBS 7.2.2: 상태 머신 적용
  - 산출물: degraded/healthy 상태 전이 정책
  - 완료 조건: 오퍼레이터가 상태를 바로 판단 가능

### 7.3 종료/복구
- [ ] WBS 7.3.1: Graceful shutdown 구현
  - 산출물: SIGTERM/SIGINT 처리기 + 자식 종료 신호 전달
  - 완료 조건: 종료 시 잔여 요청 정리 로그 남김
- [ ] WBS 7.3.2: 강제 종료 fallback
  - 산출물: 유예시간 후 미종료 child 강제 종료 정책
  - 완료 조건: 비정상 프로세스 잔존 시 장애 복구 플로우 확보

## WBS 8.0 API/문서 분리 및 계약 정립

### 8.1 API 서버(18000) 인터페이스
- [ ] WBS 8.1.1: 비동기 작업 API
  - 산출물: `POST /jobs`(`202`, `job_id`) 계약
  - 완료 조건: 클라이언트가 polling 가능한 응답을 받음
- [ ] WBS 8.1.2: 작업 조회 API
  - 산출물: `GET /jobs/{id}` 상태값 (`queued|running|completed|failed|timeout|canceled`)
  - 완료 조건: 각 상태별 에러/성공 응답 스키마 정합
- [ ] WBS 8.1.3: 상태성 API
  - 산출물: `GET /healthz`, `GET /readyz`
  - 완료 조건: 운영 모니터링에서 readiness 기준 충족

### 8.2 Swagger 분리
- [ ] WBS 8.2.1: 문서 서버 분리 배포
  - 산출물: `:14000` 전용 Swagger 프로세스
  - 완료 조건: API 서버 장애가 Swagger 가동으로 역전파되지 않음
- [ ] WBS 8.2.2: 스펙 연동
  - 산출물: Swagger가 `http://localhost:18000/openapi.json` 참조
  - 완료 조건: API 스펙 변경 시 즉시 반영 가능한 링크 체인

## WBS 9.0 단계별 마일스톤(실행 순서)

### 9.1 Phase 1: 엔진 검증
- [ ] WBS 9.1.1: 엔진 버전 lock
  - 산출물: `llama.cpp`, `whisper.cpp` 버전 고정
  - 완료 조건: 재빌드/재배포 시 동일 동작성 확보
- [ ] WBS 9.1.2: 단독 구동 검증
  - 산출물: `llama-text`, `llama-embed`, `whisper-server` 개별 실행 체크리스트
  - 완료 조건: 각 기능이 독립적으로 정상 동작

### 9.2 Phase 2: Rust 코어 + 큐
- [ ] WBS 9.2.1: 프레임워크 기반 구성
  - 산출물: `axum`, `tokio`, `reqwest`, `utoipa` 기반 스캐폴딩
  - 완료 조건: 최소 실행 서버+라우팅 정상 기동
- [ ] WBS 9.2.2: 엔진별 큐/sem 적용
  - 산출물: 3개 큐 + 엔진별 동시성 제한
  - 완료 조건: 병목 시 단계별 제어 동작 확인
- [ ] WBS 9.2.3: 작업 API 구현
  - 산출물: `POST /jobs`, `GET /jobs/{id}`
  - 완료 조건: queue 기반 비동기 처리로 상태 전이 가능

### 9.3 Phase 3: 전처리/타임아웃/오류계약
- [ ] WBS 9.3.1: 전처리 파이프라인 통합
  - 산출물: symphonia 전처리 + 길이 추출
  - 완료 조건: STT 입력 품질 편차 감쇄
- [ ] WBS 9.3.2: timeout 파라미터화 및 계산식 반영
  - 산출물: config 기반 timeout 산정
  - 완료 조건: 장애 대응 시 타임아웃 로그와 상태 분리
- [ ] WBS 9.3.3: 에러 코드 정리
  - 산출물: HTTP timeout vs job timeout 상태 코드 매핑
  - 완료 조건: 운영자가 원인 분석 가능한 표준 응답

### 9.4 Phase 4: 슈퍼비전/복구
- [ ] WBS 9.4.1: lifecycle 감시 모듈 적용
  - 산출물: child 감시 루프 구현
  - 완료 조건: 장애 인입 시 재기동/알림이 추적됨
- [ ] WBS 9.4.2: 재시작 상한/폐기 조건
  - 산출물: backoff + 상한 정책
  - 완료 조건: 무한 재시작 루프 방지
- [ ] WBS 9.4.3: 종료 프로토콜 적용
  - 산출물: 종료 절차 + fallback 종료
  - 완료 조건: 운영 중단 시 잔류 자원 정리

### 9.5 Phase 5: 문서 분리·관측성
- [ ] WBS 9.5.1: Swagger 분리 운영 완료
  - 산출물: `:14000` 배포 구성 파일
  - 완료 조건: 배포/운영 분리 체크 통과
- [ ] WBS 9.5.2: 관측 지표 노출
  - 산출물: 큐 포화/엔진 포화/timeout/재시작 지표
  - 완료 조건: 알림 임계값과 알람 룰 적용
- [ ] WBS 9.5.3: 운영 시나리오 문서화
  - 산출물: 부하/장애/복구 시나리오 문서
  - 완료 조건: 대응 절차 RTO/RPO와 연동

## WBS 10.0 즉시 착수 패키지
- [ ] WBS 10.0.1: 모델 디렉터리 및 manifest 작업
  - 산출물: `/models/{text,embed,stt}` 생성 + manifest 관리
- [ ] WBS 10.0.2: 엔진 포트별 실행 검증
  - 산출물: `llama-text(18101)`, `llama-embed(18102)`, `whisper-server(18103)` 가동 기록
- [ ] WBS 10.0.3: 임베딩 정책 고정
  - 산출물: pooling/런치 플래그 고정값 반영
- [ ] WBS 10.0.4: 큐+세마포어 구현
  - 산출물: `stt/summarize/embed` 큐 및 동시성 제한
- [ ] WBS 10.0.5: 전처리+길이 추출 반영
  - 산출물: symphonia 파이프라인 적용
- [ ] WBS 10.0.6: timeout 파라미터 파일 작성
  - 산출물: 운영용 timeout 설정값 파일
- [ ] WBS 10.0.7: supervisor 핵심 동작 구현
  - 산출물: 헬스체크/재시작/종료 핸들러
- [ ] WBS 10.0.8: Swagger 분리 배포 구성
  - 산출물: `14000` 분리 실행/배포 구성
