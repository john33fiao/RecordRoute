# MVVM 아키텍처 전환 로드맵

이 문서는 현재 프로젝트의 프론트엔드를 순수 JavaScript(Vanilla JS)에서 MVVM(Model-View-ViewModel) 아키텍처로 전환하기 위한 단계별 계획을 정의합니다. 이를 통해 장기적인 유지보수성, 테스트 용이성, 확장성을 확보하는 것을 목표로 합니다.

## 현재 코드베이스 현황

- **upload.html**: 205줄 (9개의 팝업/오버레이 포함)
- **upload.js**: 2,712줄 (단일 파일에 모든 로직 집중)
- **upload.css**: 970줄

**주요 기능:**
- 파일 업로드 (드래그 앤 드롭, 다중 선택)
- 작업 큐 관리 (STT, 임베딩, 요약 단계별 처리)
- 기록 관리 및 검색
- 텍스트 뷰어/편집기 오버레이
- 모델 설정 팝업
- 다크 모드 지원

**전환 전략:** 2,712줄의 레거시 코드를 한 번에 전환하는 것은 위험이 크므로, 점진적 하이브리드 접근 방식을 채택합니다.

---

### Phase 0: 준비 및 프레임워크 선정

이 단계에서는 전환에 필요한 기술적 기반을 마련합니다.

1.  **프레임워크 선정**
    -   **목표:** 프로젝트의 규모와 복잡도에 적합한 프론트엔드 프레임워크를 선택합니다.
    -   **추천:** **Vue.js** 또는 **Svelte**. 두 프레임워크 모두 점진적인 도입이 용이하고 학습 곡선이 완만하여 현재 구조에서 전환하기에 부담이 적습니다.
    -   **결정:** 팀의 숙련도와 선호도에 따라 최종 결정합니다.

2.  **개발 환경 구축**
    -   **목표:** 최신 개발 도구를 사용하여 프레임워크를 빌드하고 실행할 환경을 구축합니다.
    -   **도구:** **Vite**를 빌드 도구로 사용하여 빠른 개발 서버와 최적화된 빌드 프로세스를 구성합니다.
    -   **실행:** `npm create vite@latest my-vue-app -- --template vue` 와 같은 명령어로 프로젝트를 생성합니다.

3.  **프로젝트 구조화**
    -   **목표:** 기존 코드와 새로운 코드를 분리하여 체계적으로 관리합니다.
    -   **방법:** 기존 `frontend` 폴더는 백업하고, Vite로 생성된 새로운 프로젝트 구조(`src`, `public` 등)를 사용합니다. `index.html`은 `public` 폴더로 이동하거나 새로 생성된 `index.html`에 내용을 통합합니다.

---

### Phase 0.5: 하이브리드 환경 및 유틸리티 분리

대규모 레거시 코드(2,712줄)를 안전하게 전환하기 위한 준비 단계입니다.

1.  **레거시 코드 보존**
    -   **목표:** 기존 코드를 보존하면서 새 코드와 병렬 실행할 수 있는 환경을 구축합니다.
    -   **방법:**
        -   기존 `frontend` 폴더를 `legacy-frontend`로 이름 변경
        -   백엔드 서버에서 `/legacy` 경로로 레거시 UI 제공
        -   새로운 Vue/Svelte 앱은 `/app` 경로로 구성
        -   두 환경을 동시에 실행하며 점진적 전환

2.  **유틸리티 함수 먼저 분리**
    -   **목표:** 순수 함수부터 추출하여 테스트 및 재사용 준비를 합니다.
    -   **방법:**
        -   `src/utils/textUtils.js` 생성: `normalizeKorean()`, `escapeHtml()` 등
        -   `src/utils/fileUtils.js` 생성: `isAudioFile()` 등
        -   이 시점에서는 아직 Vanilla JS로 유지하되, ES6 모듈로 export

3.  **서비스 레이어 선제 분리**
    -   **목표:** API 통신 로직을 ViewModel로부터 미리 분리합니다.
    -   **방법:**
        -   `src/services/apiService.js` 생성 후 모든 fetch 로직 이동
        -   `src/services/websocketService.js` 생성: WebSocket 연결 관리 로직 이동
        -   레거시 `upload.js`에서 이 서비스들을 import하여 사용

4.  **작은 컴포넌트부터 전환**
    -   **목표:** 독립적인 UI 요소부터 Vue/Svelte로 전환하여 학습 곡선을 완화합니다.
    -   **추천 순서:**
        1.  `ThemeToggle.vue`: 다크 모드 토글 (가장 간단)
        2.  `ConfirmDialog.vue`: 재사용 가능한 확인 팝업
        3.  `DropZone.vue`: 드래그 앤 드롭 영역
    -   **방법:** 레거시 HTML에서 Vue 컴포넌트를 마운트하는 하이브리드 방식 사용

---

### Phase 1: UI 컴포넌트화

현재의 `upload.html`을 논리적인 단위의 재사용 가능한 컴포넌트로 분해합니다.

1.  **컴포넌트 식별**

    **핵심 기능 컴포넌트:**
    -   `FileUploadForm.vue`: 파일 선택 input, "Upload" 버튼을 포함하는 폼
    -   `DropZone.vue`: 드래그 앤 드롭 영역 (별도 컴포넌트로 분리)
    -   `QueueList.vue`: 작업 큐 목록 (대기/실행 중인 작업)
    -   `QueueItem.vue`: 큐 개별 항목 (진행 상태, 취소 버튼 포함)
    -   `HistoryList.vue`: 완료된 작업 기록 목록
    -   `HistoryItem.vue`: 기록 목록의 개별 항목 (체크박스, 액션 버튼 포함)
    -   `SearchPanel.vue`: 검색 UI 전체 (입력창, 결과 표시)
    -   `ThemeToggle.vue`: 다크 모드 토글 버튼

    **팝업/오버레이 컴포넌트:**
    -   `TextOverlay.vue`: 텍스트 뷰어/편집기 오버레이 (복사, 수정, 저장, 다운로드, 삭제)
    -   `ModelSettingsDialog.vue`: 모델 설정 팝업 (Whisper, 요약, 임베딩 모델 선택)
    -   `ConfirmDialog.vue`: 재사용 가능한 확인 팝업 (SummaryPopup, STTConfirmPopup 등을 이것으로 통합)
    -   `SimilarDocsDialog.vue`: 유사 문서 팝업
    -   `ResetOptionsDialog.vue`: 전체 초기화 옵션 선택 팝업

2.  **기본 컴포넌트 생성**
    -   식별된 컴포넌트 별로 `.vue` 또는 `.svelte` 파일을 생성하고, 기존 `upload.html`의 HTML 마크업을 각 컴포넌트로 분리하여 옮깁니다. CSS도 각 컴포넌트의 스코프 스타일로 이전합니다.

---

### Phase 2: ViewModel 구현 및 데이터 바인딩

컴포넌트의 상태와 로직을 담당할 ViewModel을 구현하고, View와 연결합니다.

1.  **상태(State) 정의**

    **전역 상태 (Pinia Store / Svelte Stores 사용):**
    -   `taskStore.js`: 작업 큐 관리
        -   `taskQueue: []` - 대기 중인 작업 목록
        -   `currentTask: null` - 실행 중인 작업
        -   `taskIdCounter: number` - 작업 ID 생성용
    -   `historyStore.js`: 업로드 기록 관리
        -   `historyItems: []` - 완료된 작업 목록
        -   `selectedRecords: Set` - 선택된 기록 ID
    -   `settingsStore.js`: 애플리케이션 설정
        -   `whisperModel: string` - Whisper 모델 선택
        -   `whisperLanguage: string` - Whisper 언어 설정
        -   `summarizeModel: string` - 요약 모델
        -   `embeddingModel: string` - 임베딩 모델
        -   `darkMode: boolean` - 다크 모드 상태

    **컴포넌트 로컬 상태:**
    -   `FileUploadForm`: `selectedFiles: FileList`
    -   `TextOverlay`: `isEditing: boolean`, `currentContent: string`
    -   `SearchPanel`: `searchQuery: string`, `searchResults: []`

    **상태 구분 기준:**
    -   **전역 상태**: 여러 컴포넌트에서 공유하거나, 페이지 새로고침 후에도 유지해야 할 데이터
    -   **로컬 상태**: 특정 컴포넌트 내부에서만 사용되는 일시적 데이터

2.  **데이터 바인딩 적용**
    -   View의 요소들을 ViewModel의 상태에 바인딩합니다.
    -   예:
        -   체크박스 선택 상태 → `selectedRecords` Set에 양방향 바인딩
        -   `HistoryList`는 `historyItems` 배열을 기반으로 목록을 렌더링
        -   `ThemeToggle` → `darkMode` boolean에 바인딩

3.  **명령어(Commands) 구현**
    -   `upload.js`에 있던 이벤트 핸들러 함수들을 ViewModel의 메서드(명령어)로 옮깁니다.
    -   예:
        -   `FileUploadForm.uploadFiles()` - 파일 업로드
        -   `QueueItem.cancelTask()` - 작업 취소
        -   `HistoryItem.processRecord()` - 기록 재처리
        -   `TextOverlay.saveEdit()` - 텍스트 수정 저장

---

### Phase 3: 서비스 레이어 분리 및 API 연동

백엔드와 통신하는 로직을 ViewModel로부터 분리하여 독립적인 서비스 레이어를 만듭니다.

1.  **API 서비스 모듈 생성**
    -   `src/services/apiService.js`: 모든 HTTP 요청 통합
    -   `src/services/websocketService.js`: WebSocket 연결 관리

2.  **API 함수 구현**

    `upload.js`에 있던 모든 `fetch` 호출 로직을 서비스 레이어로 옮깁니다.

    **파일 관리 API:**
    -   `uploadFile(file)` → `POST /upload` - 파일 업로드
    -   `getHistory()` → `GET /history` - 업로드 기록 조회
    -   `deleteRecords(recordIds)` → `POST /delete_records` - 기록 삭제
    -   `downloadFile(fileUuid)` → `GET /download/<file_uuid>` - 결과 파일 다운로드

    **작업 처리 API:**
    -   `processTask(filePath, steps, recordId, taskId)` → `POST /process` - 워크플로우 실행 (steps: ["stt", "embedding", "summary"])
    -   `cancelTask(taskId)` → `POST /cancel` - 실행 중인 작업 취소
    -   `resetSummaryEmbedding(recordId)` → `POST /reset_summary_embedding` - 요약/임베딩 초기화

    **검색 API:**
    -   `searchDocuments(query, startDate, endDate)` → `GET /search?q=<query>&start=<start>&end=<end>` - 키워드 및 벡터 검색
    -   `getSimilarDocs(fileIdentifier, refresh)` → `POST /similar` - 유사 문서 조회

    **설정 API:**
    -   `getOllamaModels()` → `GET /models` - 사용 가능한 Ollama 모델 목록
    -   `updateSTTText(fileIdentifier, content)` → `POST /update_stt_text` - STT 텍스트 수정

    **WebSocket 통신:**
    -   `connectWebSocket(onMessage)` → `ws://localhost:8765` - 실시간 진행 상황 수신
    -   메시지 형식: `{"task_id": "...", "message": "..."}`

3.  **ViewModel에서 서비스 호출**
    -   이제 ViewModel은 `fetch`를 직접 호출하는 대신, API 서비스 모듈을 `import`하여 해당 함수들을 호출합니다.
    -   예:
        ```javascript
        // 기존 방식 (upload.js)
        const response = await fetch('/upload', { method: 'POST', body: formData });

        // 새로운 방식 (ViewModel에서 서비스 사용)
        import { uploadFile } from '@/services/apiService';
        const result = await uploadFile(file);
        ```
    -   **장점:**
        -   API 명세가 변경되어도 서비스 모듈만 수정
        -   ViewModel 단위 테스트 시 서비스를 모킹(Mocking)하여 네트워크 요청 없이 테스트 가능
        -   에러 처리 및 재시도 로직을 중앙 집중화

---

### Phase 4: 리팩토링 및 최종 정리

전환 작업을 마무리하고 코드를 정리합니다.

1.  **전역 상태 관리 도입**

    현재 코드베이스에는 많은 전역 상태가 존재하므로 **필수적**입니다.

    **Vue.js 사용 시:**
    -   **Pinia** 설치: `npm install pinia`
    -   `src/stores/` 디렉토리에 Phase 2에서 정의한 스토어 파일 생성
    -   예: `taskStore.js`, `historyStore.js`, `settingsStore.js`
    -   `main.js`에서 Pinia 플러그인 등록

    **Svelte 사용 시:**
    -   **Svelte Stores** (내장 기능) 사용
    -   `src/stores/` 디렉토리에 `writable` 또는 `readable` 스토어 생성
    -   예: `taskStore.js`, `historyStore.js`, `settingsStore.js`

    **LocalStorage 연동:**
    -   사용자 설정(`whisperModel`, `whisperLanguage`, `darkMode`)은 LocalStorage에 저장하여 새로고침 후에도 유지
    -   작업 큐(`taskQueue`)와 기록(`historyItems`)은 서버에서 조회하되, 성능 향상을 위해 캐싱 고려

2.  **테스트 전략 수립 및 실행**

    **단위 테스트 (Vitest):**
    -   **대상:**
        -   `src/utils/` 폴더의 모든 유틸리티 함수
        -   `src/services/apiService.js`의 각 API 함수 (모킹 사용)
        -   Pinia 스토어의 액션 및 게터
    -   **도구:** Vitest (`npm install -D vitest`)
    -   **예시:**
        ```javascript
        // textUtils.test.js
        import { describe, it, expect } from 'vitest';
        import { normalizeKorean, escapeHtml } from '@/utils/textUtils';

        describe('textUtils', () => {
          it('normalizeKorean should normalize Korean text', () => {
            expect(normalizeKorean('ㅎㅏㄴㄱㅡㄹ')).toBe('한글');
          });
        });
        ```

    **컴포넌트 테스트 (Vue Test Utils / Svelte Testing Library):**
    -   **대상:** 각 Vue/Svelte 컴포넌트의 렌더링 및 사용자 상호작용
    -   **도구:**
        -   Vue: `@vue/test-utils` (`npm install -D @vue/test-utils`)
        -   Svelte: `@testing-library/svelte`
    -   **예시:**
        ```javascript
        // ThemeToggle.test.js
        import { mount } from '@vue/test-utils';
        import ThemeToggle from '@/components/ThemeToggle.vue';

        it('should toggle dark mode on click', async () => {
          const wrapper = mount(ThemeToggle);
          await wrapper.find('button').trigger('click');
          expect(wrapper.vm.darkMode).toBe(true);
        });
        ```

    **통합 테스트:**
    -   **대상:** 서비스 레이어와 스토어 간의 상호작용
    -   **방법:** 실제 API를 모킹하고, 스토어 액션 호출 후 상태 변화 검증

    **E2E 테스트 (Playwright / Cypress):**
    -   **대상:** 전체 사용자 워크플로우
    -   **도구:** Playwright (`npm install -D @playwright/test`) 또는 Cypress
    -   **시나리오:**
        1.  파일 업로드
        2.  작업 큐에 추가 확인
        3.  작업 실행 및 진행 상태 확인
        4.  완료 후 기록 목록에서 결과 확인
        5.  검색 기능 테스트
        6.  텍스트 수정 및 저장
    -   **예시:**
        ```javascript
        // e2e/upload-workflow.spec.js
        import { test, expect } from '@playwright/test';

        test('complete upload workflow', async ({ page }) => {
          await page.goto('http://localhost:5173');
          await page.setInputFiles('#fileInput', 'test-audio.m4a');
          await page.click('#uploadBtn');
          await expect(page.locator('#queue-list')).toContainText('test-audio.m4a');
        });
        ```

3.  **기존 레거시 코드 제거**
    -   새로운 MVVM 기반 프론트엔드가 모든 테스트를 통과한 후:
        -   `legacy-frontend/` 폴더 삭제
        -   백엔드 서버에서 `/legacy` 경로 라우팅 제거
        -   기존 `upload.html`, `upload.js`, `upload.css` 완전 삭제

4.  **코드 품질 도구 설정**
    -   **ESLint**: 코드 스타일 및 잠재적 버그 탐지
    -   **Prettier**: 코드 포맷팅 자동화
    -   **TypeScript (선택)**: 타입 안정성 강화 (큰 프로젝트일 경우 권장)

5.  **문서화**
    -   각 컴포넌트의 Props, Events 문서화
    -   API 서비스 함수의 매개변수 및 반환값 JSDoc 주석 추가
    -   README 업데이트: 새로운 프로젝트 구조 및 개발 환경 설정 방법

---

## 우선순위 및 실행 순서

현재 코드베이스의 복잡도(2,712줄)를 고려한 추천 순서:

1.  **Phase 0** (프레임워크 선정 및 환경 구축) → 필수
2.  **Phase 0.5** (유틸리티 분리, 서비스 레이어 추출) → **먼저 진행 권장**
3.  **Phase 1** (작은 컴포넌트부터 점진적 전환) → ThemeToggle → ConfirmDialog → DropZone 순서로
4.  **Phase 3** (서비스 레이어 완성) → Phase 1과 병행 가능
5.  **Phase 2** (ViewModel 본격 구현) → 서비스 레이어가 준비된 후
6.  **Phase 4** (상태 관리 도입, 테스트, 정리) → 마지막

**핵심 전략:**
-   **점진적 전환**: 한 번에 모든 것을 바꾸지 않고, 작은 단위부터 검증하며 진행
-   **병렬 작업**: 서비스 레이어와 컴포넌트는 독립적으로 개발 가능
-   **테스트 주도**: 각 단계마다 테스트를 작성하여 회귀 방지

---

## 주의사항 및 팁

### 일반적인 함정 회피

1.  **과도한 추상화 방지**
    -   초기에는 컴포넌트를 너무 세분화하지 말 것
    -   재사용성이 명확한 경우에만 공통 컴포넌트로 추출
    -   예: `ConfirmDialog`는 여러 곳에서 사용되므로 추상화 적절, 하지만 한 곳에서만 쓰는 UI는 그대로 유지

2.  **상태 관리 복잡도 주의**
    -   모든 상태를 전역 스토어에 넣지 말 것
    -   컴포넌트 간 공유가 필요 없는 상태는 로컬로 유지
    -   예: `TextOverlay`의 `isEditing`은 로컬 상태로 충분

3.  **레거시 코드와의 공존**
    -   하이브리드 기간 동안 레거시 코드 수정 최소화
    -   버그 수정은 새 코드에만 적용하고, 레거시는 동결
    -   사용자에게 새 UI로 이전할 것을 권장하는 배너 추가 고려

### 성능 최적화 고려사항

1.  **대용량 파일 목록 처리**
    -   `HistoryList`가 수백 개 항목을 렌더링할 경우 가상 스크롤링(Virtual Scrolling) 고려
    -   Vue: `vue-virtual-scroller`, Svelte: `svelte-virtual-list`

2.  **WebSocket 연결 관리**
    -   컴포넌트 언마운트 시 WebSocket 연결 정리
    -   재연결 로직 구현 (네트워크 끊김 대비)

3.  **이미지 및 정적 자산**
    -   Vite의 자동 최적화 기능 활용
    -   큰 파일은 지연 로딩(Lazy Loading)

### 협업 및 배포

1.  **Git 브랜치 전략**
    -   `main` 브랜치: 레거시 코드 유지
    -   `feature/mvvm-migration` 브랜치: 새 코드 개발
    -   Phase별로 서브 브랜치 생성 (예: `feature/mvvm-phase1`)

2.  **점진적 배포**
    -   Feature Flag를 사용하여 사용자 일부에게만 새 UI 노출
    -   피드백 수집 후 전체 롤아웃

3.  **백엔드 호환성**
    -   API 엔드포인트는 변경하지 말 것
    -   레거시 UI와 새 UI가 동일한 백엔드 API 사용
    -   필요 시 백엔드에 새 엔드포인트 추가는 가능하나, 기존 것은 유지

### 추천 학습 자료

-   **Vue.js**: [Vue.js 공식 문서](https://vuejs.org/), [Vue Mastery](https://www.vuemastery.com/)
-   **Svelte**: [Svelte 공식 튜토리얼](https://svelte.dev/tutorial)
-   **Pinia**: [Pinia 공식 문서](https://pinia.vuejs.org/)
-   **Vitest**: [Vitest 공식 문서](https://vitest.dev/)
-   **Playwright**: [Playwright 공식 문서](https://playwright.dev/)

---

## 체크리스트

각 Phase가 완료되었는지 확인하기 위한 체크리스트:

### Phase 0
- [ ] 프레임워크 선정 완료 (Vue.js 또는 Svelte)
- [ ] Vite 프로젝트 생성 완료
- [ ] 기본 프로젝트 구조 설정 완료

### Phase 0.5
- [ ] 레거시 코드를 `legacy-frontend`로 이동
- [ ] 유틸리티 함수 분리 완료 (`textUtils.js`, `fileUtils.js`)
- [ ] 서비스 레이어 분리 완료 (`apiService.js`, `websocketService.js`)
- [ ] 작은 컴포넌트 1개 이상 전환 성공 (예: `ThemeToggle`)

### Phase 1
- [ ] 모든 컴포넌트 식별 완료 (핵심 + 팝업 컴포넌트)
- [ ] 각 컴포넌트 파일 생성 완료
- [ ] HTML 마크업 및 CSS를 각 컴포넌트로 이전 완료
- [ ] 컴포넌트 간 Props 및 Events 정의 완료

### Phase 2
- [ ] 전역 스토어 정의 완료 (`taskStore`, `historyStore`, `settingsStore`)
- [ ] 컴포넌트 로컬 상태 정의 완료
- [ ] 데이터 바인딩 적용 완료
- [ ] 모든 이벤트 핸들러를 ViewModel 메서드로 전환 완료

### Phase 3
- [ ] 모든 API 함수를 `apiService.js`로 이전 완료
- [ ] WebSocket 연결 관리 서비스 완성
- [ ] ViewModel에서 서비스 호출로 전환 완료
- [ ] 에러 처리 및 재시도 로직 중앙 집중화 완료

### Phase 4
- [ ] Pinia/Svelte Stores 도입 완료
- [ ] LocalStorage 연동 완료
- [ ] 단위 테스트 작성 완료 (주요 유틸리티 및 서비스)
- [ ] 컴포넌트 테스트 작성 완료
- [ ] E2E 테스트 시나리오 작성 및 실행 완료
- [ ] 모든 테스트 통과
- [ ] 레거시 코드 완전 제거
- [ ] ESLint/Prettier 설정 완료
- [ ] 문서화 완료

### 최종 검증
- [ ] 모든 기능이 레거시 버전과 동일하게 동작
- [ ] 브라우저 호환성 테스트 완료 (Chrome, Firefox, Safari, Edge)
- [ ] 성능 테스트 완료 (대용량 파일 목록 처리)
- [ ] 사용자 피드백 수집 및 반영
- [ ] 프로덕션 배포 준비 완료
