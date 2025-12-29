# MVVM 아키텍처 전환 로드맵

이 문서는 현재 프로젝트의 프론트엔드를 순수 JavaScript(Vanilla JS)에서 MVVM(Model-View-ViewModel) 아키텍처로 전환하기 위한 단계별 계획을 정의합니다. 이를 통해 장기적인 유지보수성, 테스트 용이성, 확장성을 확보하는 것을 목표로 합니다.

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

### Phase 1: UI 컴포넌트화

현재의 `upload.html`을 논리적인 단위의 재사용 가능한 컴포넌트로 분해합니다.

1.  **컴포넌트 식별**
    -   `FileUploadForm.vue`: 파일 선택 input, "처리 시작" 버튼을 포함하는 폼.
    -   `TaskOptions.vue`: '음성 변환', '텍스트 교정', '텍스트 요약' 체크박스 그룹.
    -   `ProgressView.vue`: 현재 진행 중인 작업의 상태와 진행률을 표시하는 영역.
    -   `HistoryList.vue`: 완료된 작업 기록을 목록 형태로 보여주는 컴포넌트.
    -   `HistoryItem.vue`: 기록 목록의 개별 항목.

2.  **기본 컴포넌트 생성**
    -   식별된 컴포넌트 별로 `.vue` 또는 `.svelte` 파일을 생성하고, 기존 `upload.html`의 HTML 마크업을 각 컴포넌트로 분리하여 옮깁니다. CSS도 각 컴포넌트의 스코프 스타일로 이전합니다.

---

### Phase 2: ViewModel 구현 및 데이터 바인딩

컴포넌트의 상태와 로직을 담당할 ViewModel을 구현하고, View와 연결합니다.

1.  **상태(State) 정의**
    -   각 컴포넌트 또는 전역 스토어에 필요한 데이터 속성을 정의합니다.
    -   예: `selectedFile`, `taskOptions: { transcribe: true, ... }`, `currentProgress`, `historyItems: []`

2.  **데이터 바인딩 적용**
    -   View의 요소들을 ViewModel의 상태에 바인딩합니다.
    -   예: 체크박스의 `checked` 속성을 `taskOptions` 객체의 각 불리언 값에 바인딩합니다. `HistoryList`는 `historyItems` 배열을 기반으로 목록을 렌더링합니다.

3.  **명령어(Commands) 구현**
    -   `upload.js`에 있던 이벤트 핸들러 함수들을 ViewModel의 메서드(명령어)로 옮깁니다.
    -   예: "처리 시작" 버튼 클릭 시 실행될 `startProcessing` 메서드를 `FileUploadForm`의 ViewModel에 구현합니다.

---

### Phase 3: 서비스 레이어 분리 및 API 연동

백엔드와 통신하는 로직을 ViewModel로부터 분리하여 독립적인 서비스 레이어를 만듭니다.

1.  **API 서비스 모듈 생성**
    -   `src/services/api.js` 또는 `src/services/taskService.js` 같은 파일을 생성합니다.

2.  **API 함수 구현**
    -   `upload.js`에 있던 `fetch` 호출 로직을 이곳으로 옮깁니다.
    -   `processFile(file, options)`: `/process` API 호출
    -   `pollProgress(taskId)`: `/progress/<taskId>` API 호출
    -   `getHistory()`: `/history` API 호출

3.  **ViewModel에서 서비스 호출**
    -   이제 ViewModel은 `fetch`를 직접 호출하는 대신, API 서비스 모듈을 `import`하여 해당 함수들을 호출합니다.
    -   **장점:** API 명세가 변경되어도 서비스 모듈만 수정하면 되며, ViewModel을 테스트할 때 실제 네트워크 요청 없이 서비스를 모킹(Mocking)할 수 있습니다.

---

### Phase 4: 리팩토링 및 최종 정리

전환 작업을 마무리하고 코드를 정리합니다.

1.  **기존 코드 제거**
    -   새로운 MVVM 기반의 프론트엔드가 완전히 동작하는 것을 확인한 후, 기존의 `frontend/upload.html`, `upload.js`, `upload.css` 파일을 프로젝트에서 삭제합니다.

2.  **전역 상태 관리 도입 (선택 사항)**
    -   컴포넌트 간 상태 공유가 복잡해질 경우, Pinia(Vue)나 Svelte Stores 같은 상태 관리 라이브러리를 도입하여 데이터를 중앙에서 관리하는 것을 고려합니다.

3.  **최종 테스트**
    -   파일 업로드부터 결과 확인까지 전체 워크플로우에 대한 End-to-End(E2E) 테스트를 수행하여 모든 기능이 정상적으로 동작하는지 검증합니다.
