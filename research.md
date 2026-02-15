# RecordRoute 심층 분석 리서치

## 1) 시스템 개요

RecordRoute는 **업로드된 음성/문서 파일을 STT → 교정 → 요약 → 임베딩(검색 인덱싱)** 으로 처리하는 통합 파이프라인입니다.

- 백엔드: Python `ThreadingHTTPServer` 기반 HTTP API + 별도 WebSocket 서버
- 프론트엔드: React(Vite) 기반 큐/이력/검색 UI
- 핵심 워크플로우: `sttEngine/http_api/workflow.py::run_workflow`
- 작업 상태 전달: 서버 `update_task_progress()` → WS 브로드캐스트 + 프론트 폴링(`/progress/{task_id}`)

---

## 2) 디렉터리 및 모듈 역할 (핵심 경로)

### 서버 진입점/런타임
- `sttEngine/server.py`
  - 로깅 초기화 후 `sttEngine.http_api.app.main()` 호출
- `sttEngine/http_api/app.py`
  - `ThreadingHTTPServer`(기본 `127.0.0.1:8080`) 실행
  - 별도 데몬 스레드에서 WebSocket 서버(`:8765`) 시작

### HTTP 라우팅
- `sttEngine/http_api/handler.py`
  - `GET /`: UI 서빙
  - `POST /upload`: 업로드 + 해시 중복 체크 + history 기록
  - `POST /process`: 실제 처리 실행(동기)
  - `POST /cancel`: 작업 취소 API
  - `GET /progress/{task_id}`, `GET /tasks`, `GET /history`, `GET /search` 등

### 처리 워크플로우
- `sttEngine/server/routes/process.py`
  - `/process` 요청을 파싱 후 `run_process_task()` 호출
- `sttEngine/server/tasks/queue.py`
  - `run_process_task()`에서 `run_workflow()` 직접 실행
- `sttEngine/http_api/workflow.py`
  - 입력 파일 타입(audio/text/pdf) 분기
  - `steps` 기반으로 `stt`, `embedding`, `correct`, `summary` 단계 수행
  - 단계별 progress 업데이트 및 결과 링크 반환

### 상태/취소/진행률
- `sttEngine/http_api/state.py`
  - `running_processes`, `task_progress` 메모리 맵 관리
  - `cancel_task()`, `is_task_cancelled()`, `update_task_progress()` 제공
- `sttEngine/http_api/ws.py`
  - `broadcast_progress()`로 WS 클라이언트에 진행률 push

### 프론트 큐 스케줄러
- `frontend/src/hooks/useTaskQueue.ts`
  - 클라이언트 측 큐 정렬/실행(카테고리 우선 또는 추가순)
  - task를 1개씩 `api.processTask()`로 처리
  - 진행률: WS + `/progress` 폴링 병행

---

## 3) 실제 작업 스케줄링 흐름 (E2E)

1. 사용자가 파일 업로드 (`POST /upload`)
2. UI가 파일별 작업(stt/embedding/summary)을 **클라이언트 큐**에 적재 (`useTaskQueue.addTask`)
3. `useTaskQueue.processNext`가 다음 task를 선택해 `/process` 호출
4. 서버 `/process`는 요청 스레드에서 **동기적으로** `run_workflow` 수행
5. `run_workflow`는 단계별로 `update_task_progress(task_id, msg, stage)` 호출
6. progress는
   - WebSocket push
   - 폴링(`/progress/{task_id}`) 둘 다로 UI 반영
7. 완료 시 결과 URL 반환, UI가 history refresh

중요: 현재 구조는 “서버 내부 중앙 큐 + worker”가 아니라 **클라이언트 큐가 순서를 만들고 서버는 요청마다 즉시 실행**하는 방식입니다.

---

## 4) 취소(cancellation) 관련 심층 분석

요구사항에서 지적한 “취소되어야 할 작업이 실행되는” 현상을 기준으로 취소 체인을 추적했습니다.

### 취소 의도된 경로
- UI에서 취소 버튼 클릭
- `AbortController.abort()`로 네트워크 요청 취소
- (이상적으로는) 서버 `/cancel` 호출 + 서버 런타임에서 실행 중 작업 중단

### 실제 구현 경로
- UI 취소 시 `removeTask()`는 `abortController.abort()`만 수행
- **`api.cancelTask()`를 호출하지 않음**
- 서버는 이미 받은 `/process`를 계속 실행 가능
- `run_workflow`의 `is_task_cancelled(task_id)`는 `running_processes` 플래그를 보는데,
  - 현재 이 맵에 task를 넣는 `register_process()` 호출이 처리 경로에 없음
  - 결과적으로 `is_task_cancelled()`가 사실상 항상 False

즉, **취소 신호가 서버 실행 흐름에 도달하지 못하는 구조적 결함**이 있습니다.

---

## 5) 발견한 버그 목록 (스케줄링/취소 중심)

아래는 취소 흐름을 기준으로 확인된 버그들입니다.

### 버그 A — 프론트 취소가 서버 취소 API를 호출하지 않음
- 위치: `frontend/src/hooks/useTaskQueue.ts` (`removeTask`, `cancelAll`)
- 증상:
  - UI는 취소됨으로 표시하지만 서버는 해당 작업을 계속 수행
  - 특히 대형 STT/요약에서 체감됨
- 원인:
  - `abortController.abort()`만 호출
  - `/cancel` API(`api.cancelTask`) 미호출
- 결과:
  - “취소했는데도 완료 결과가 나옴” 현상 발생

### 버그 B — 서버 취소 상태 저장소(`running_processes`)가 실제 실행 경로와 분리됨
- 위치: `sttEngine/http_api/state.py`, `sttEngine/server/tasks/queue.py`, `sttEngine/http_api/workflow.py`
- 증상:
  - `/cancel` 요청이 와도 해당 task를 찾지 못해 `success=false` 가능
  - `is_task_cancelled`가 true가 되지 않음
- 원인:
  - `register_process()`가 `/process` 실행 경로에서 호출되지 않음
  - task lifecycle이 state 레지스트리와 연결되지 않음
- 결과:
  - 워크플로우 내 취소 체크 코드가 무력화

### 버그 C — 취소 구현이 "프로세스 종료"를 전제로 하나 실제 실행은 동일 프로세스 동기 함수
- 위치: `state.cancel_task()`
- 증상:
  - `process.terminate()/wait()/kill()` 전제
- 원인:
  - 실제 `/process`는 `run_workflow()`를 현재 Python 프로세스의 요청 스레드에서 동기 실행
  - 종료 가능한 외부 subprocess 핸들을 기본으로 갖지 않음
- 결과:
  - 설계와 런타임 모델 불일치
  - 취소 처리의 실효성 저하

### 버그 D — 장시간 단계 내부에서 취소 반응 불가/지연
- 위치: `workflow.py`의 취소 체크 위치
- 증상:
  - STT/요약 같은 장시간 함수 호출 중 취소해도 즉시 반영 안 됨
- 원인:
  - `is_task_cancelled()` 체크가 단계 진입 시점 위주
  - `transcribe_audio_files`, `summarize_text_mapreduce`, `correct_text_file` 내부 cooperative cancel 경로 부재
- 결과:
  - "취소했는데 오래 실행" 혹은 사실상 완료까지 감

### 버그 E — 작업 단계 이름 불일치로 summary 작업이 실제 실행되지 않는 케이스
- 위치:
  - 프론트: `useTaskQueue.getStepForType('summary')` → `['summarize']`
  - 백엔드: `run_workflow`는 `if "summary" in steps:` 확인
- 증상:
  - summary 큐 작업이 성공/실패가 아니라 **아무 단계도 수행하지 않는 no-op** 가능
- 결과:
  - 스케줄링 관점에서 “작업은 실행됐다고 보이는데 기대 단계 미실행” 불일치
  - 취소 분석 시에도 상태 혼선을 유발

---

## 6) 왜 "가끔" 취소 실패처럼 보이는가

현상은 “항상 실패”에 가까우나 사용자 관점에선 가끔처럼 보일 수 있습니다.

- 요청 전/초기 구간에서 abort되면 서버 진입 전 취소된 것처럼 보임
- 이미 서버에 진입한 후엔 계속 수행되어 취소 실패로 보임
- 단계별/파일별 실행 시간 차이가 커서 체감 빈도가 들쭉날쭉

---

## 7) 시스템 이해를 위한 추가 핵심 포인트

- 진행률은 `task_progress` 메모리 맵 기반이며 프로세스 재시작 시 휘발성
- WebSocket은 push 채널, polling은 보조 채널로 중복 안전성 확보
- 파일 타입별 분기:
  - audio: STT 가능
  - text/pdf: STT를 변환 단계로 취급(복사/텍스트 추출)
- embedding/summary 단계에서 audio이고 STT 산출물이 없으면 STT 자동 선행 실행
- history/registry 갱신은 단계 완료마다 부분적으로 반영

---

## 8) 결론

현재 작업 스케줄링은 “클라이언트 큐 + 서버 동기 실행” 구조이며, 취소 체인이 프론트/UI와 서버 런타임 state 사이에서 끊겨 있습니다. 이로 인해 **취소 요청 후에도 작업이 계속 실행되는 버그가 재현 가능**합니다.

특히 A~D는 취소 기능의 핵심 결함이고, E는 스케줄링 단계 일관성 결함입니다. 이 다섯 가지가 결합되어 사용자 체감상 "취소 불안정"과 "작업 동작 불일치"를 만듭니다.
