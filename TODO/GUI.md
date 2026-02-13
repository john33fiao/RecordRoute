# GUI 개선 TodoList

RecordRoute 프로젝트의 현재 코드베이스를 기반으로 한 UI/UX 개선 작업 목록입니다.

## React 전환 이후 리팩토링 TODO (신규)

> 현황: 기존 `frontend/legacy/upload.*` 기반 UI에서 `frontend/src` React + TypeScript 구조로 이전 완료.

### 아키텍처/상태관리
- [ ] `App.tsx`에 집중된 상태를 도메인 단위 커스텀 훅으로 분리 (`upload`, `queue`, `search`, `viewer`)
- [ ] API 호출 상태(`loading`, `error`, `stale`)를 공통 패턴으로 표준화
- [ ] 폴링/웹소켓 갱신 로직의 책임 경계를 정리하고 중복 갱신 방지
- [ ] Context 사용 범위를 점검해 불필요한 전역 리렌더 최소화

### 컴포넌트 구조
- [ ] 프레젠테이션 컴포넌트와 컨테이너 컴포넌트 역할 재정리
- [ ] `Dialog`/`Overlay`류 컴포넌트 공통 인터페이스 정의 (`open`, `onClose`, `onConfirm`)
- [ ] 리스트성 UI(`HistoryPanel`, `JobQueue`)의 아이템 렌더러 분리 및 재사용성 개선
- [ ] 컴포넌트 Props 타입을 더 엄격히 정의하고 optional 남용 정리

### 타입/모델 정리
- [ ] `src/api/types.ts` 기준으로 서버 응답 스키마 단일화
- [ ] 문자열 리터럴 상태값을 유니온 타입/enum으로 치환
- [ ] 날짜/시간/파일크기 포맷 타입 유틸 정리

### 에러/사용성
- [ ] 네트워크 실패, 타임아웃, 서버 에러에 대한 UI 피드백 정책 통일
- [ ] 장시간 작업(요약/임베딩) 중 취소/재시도 UX를 컴포넌트 전반에 일관 적용
- [ ] 키보드 접근성(포커스 트랩, ESC 닫기, ARIA 라벨) 점검 체크리스트 추가

### 성능/품질
- [ ] 불필요 렌더링 구간 React DevTools로 프로파일링 후 메모이제이션 적용
- [ ] 대용량 히스토리 대응 가상 스크롤 도입 검토
- [ ] 단위 테스트(훅/유틸) + 핵심 사용자 플로우 컴포넌트 테스트 추가
- [ ] ESLint/Prettier/TypeScript 규칙에서 React 패턴 관련 경고를 점진적 에러화

## 파일 업로드 UI 개선

### 드래그&드롭 강화
- [ ] 현재 기본 파일 입력을 드래그&드롭 영역으로 교체
- [ ] 업로드 중 진행률 표시 추가
- [ ] 파일 타입별 아이콘 표시 (오디오/텍스트/PDF)
- [ ] 다중 파일 선택 시 개별 파일 미리보기

**참고 레퍼런스**: 
- [Uppy Dashboard](https://uppy.io/docs/dashboard/)
- [Dribbble 파일 업로드 UI](https://dribbble.com/tags/file-upload-ui)
- [파일 업로드 베스트 프랙티스](https://www.uinkits.com/blog-post/best-practices-for-file-upload-components)

### 파일 미리보기 기능
- [ ] 오디오 파일: 파형 표시 및 간단한 재생 컨트롤
- [ ] 텍스트 파일: 첫 몇 줄 미리보기
- [ ] PDF: 첫 페이지 썸네일
- [ ] 파일 크기 및 예상 처리 시간 표시

## 작업 큐 및 진행 상태 UI

### 큐 시각화 개선
- [ ] 현재 카테고리별 정렬을 시각적 파이프라인으로 표현
- [ ] STT → 교정 → 요약 → 임베딩 단계를 프로그레스 바로 시각화
- [ ] 각 단계별 예상 소요 시간 표시
- [ ] 큐 내 작업 순서 드래그&드롭으로 변경 가능

**참고 레퍼런스**:
- [Queue Status 모듈](https://docs.datamatics.com/TruCap+/7.7.0/Monitoring%20Data/Queue%20Status.htm)
- [Telerik Upload 컴포넌트](https://www.telerik.com/design-system/docs/components/upload/)

### 실시간 상태 표시
- [ ] 현재 작업 중인 파일의 실시간 로그 스트리밍
- [ ] 각 단계별 완료율 표시 (STT: 45%, 요약: 대기중...)
- [ ] 오류 발생 시 상세 에러 메시지 및 재시도 버튼
- [ ] 전체 큐 처리 예상 완료 시간

**참고 레퍼런스**:
- [RealtimeSTT](https://github.com/KoljaB/RealtimeSTT)
- [Deepgram 스트리밍](https://deepgram.com/learn/all-about-transcription-for-real-time-audio-streaming)
- [Gladia API](https://www.gladia.io)

## 오버레이 모달 UI 개선

### 텍스트 뷰어 모달
- [ ] 현재 `#textOverlay` 모달의 접근성 개선
- [ ] 텍스트 검색 및 하이라이트 기능
- [ ] 폰트 크기 조절, 다크모드 토글
- [ ] 텍스트 편집 기능 (간단한 수정 후 저장)
- [ ] 문서 간 빠른 전환 탭

**참고 레퍼런스**:
- [PatternFly Modal Overlay](https://pf3.patternfly.org/v3/pattern-library/forms-and-controls/modal-overlay/)
- [Pure CSS Modal](https://www.cssscript.com/minimal-overlay-modal-pure-css/)

### 설정 모달 개선
- [ ] 현재 `#modelSettingsPopup`의 UX 개선
- [ ] 모델별 성능/정확도 정보 표시
- [ ] 실시간 모델 상태 확인 (Ollama 연결 상태)
- [ ] 설정 저장 및 프리셋 관리
- [ ] 고급 설정 접기/펼치기

## 검색 및 탐색 강화

### 검색 인터페이스 개선
- [ ] 현재 단순한 `#searchInput`을 고급 검색으로 확장
- [ ] 검색 필터: 날짜, 파일 타입, 처리 상태
- [ ] 검색 결과 하이라이트 및 스니펫 표시
- [ ] 검색 히스토리 및 자동완성
- [ ] 태그 기반 검색

### 유사 문서 탐색
- [ ] 현재 `#similarDocsPopup` 기능 강화
- [ ] 유사도 점수 시각화
- [ ] 문서 간 연관 관계 그래프
- [ ] 클러스터링 기반 문서 그룹핑

## 대시보드 레이아웃 개선

### 전체 레이아웃 최적화
- [ ] 현재 섹션별 구조를 대시보드 형태로 재구성
- [ ] 사이드바: 파일 트리 및 빠른 액세스
- [ ] 메인 영역: 작업 상태 및 진행률
- [ ] 우측 패널: 속성 및 설정

### 반응형 디자인
- [ ] 모바일/태블릿 최적화
- [ ] 화면 크기별 레이아웃 적응
- [ ] 터치 인터페이스 지원

**참고 레퍼런스**:
- [Tabler](https://github.com/tabler/tabler) - 무료 HTML 대시보드 템플릿
- [Eleken 대시보드 예제](https://www.eleken.co/blog-posts/dashboard-design-examples-that-catch-the-eye)

## 테마 및 스타일링

### 다크모드 구현
- [ ] 현재 `#themeToggle` 기능 완성
- [ ] CSS 변수 기반 테마 시스템
- [ ] 사용자 선택 저장 (localStorage)
- [ ] 시스템 테마 자동 감지

### 컴포넌트 일관성
- [ ] 버튼, 입력창, 모달 등 일관된 디자인 시스템
- [ ] 로딩 스피너 및 스켈레톤 UI
- [ ] 애니메이션 및 트랜지션 추가
- [ ] 접근성 준수 (ARIA 라벨, 키보드 네비게이션)

## 성능 최적화

### 프론트엔드 최적화
- [ ] 현재 `upload.js`의 모듈화 및 분리
- [ ] 가상 스크롤링 (대량 파일 목록)
- [ ] 이미지 레이지 로딩
- [ ] Service Worker 캐싱

### 실시간 업데이트
- [ ] WebSocket 연결로 실시간 상태 업데이트
- [ ] 폴링 최적화 및 배터리 절약
- [ ] 오프라인 모드 지원

## 추가 기능

### 사용자 경험 개선
- [ ] 온보딩 튜토리얼
- [ ] 키보드 단축키 지원
- [ ] 컨텍스트 메뉴 (우클릭)
- [ ] 실행 취소/다시 실행

### 데이터 시각화
- [ ] 처리 통계 대시보드
- [ ] 파일 크기/처리 시간 차트
- [ ] 모델 성능 비교 그래프

## 구현 우선순위

### Phase 1 (즉시 구현)
1. 파일 업로드 드래그&드롭 강화
2. 작업 큐 시각화 개선
3. 오버레이 모달 접근성 개선

### Phase 2 (단기)
1. 검색 인터페이스 고급화
2. 실시간 상태 표시
3. 다크모드 완성

### Phase 3 (중기)
1. 대시보드 레이아웃 재구성
2. 반응형 디자인
3. 성능 최적화

### Phase 4 (장기)
1. 고급 데이터 시각화
2. 사용자 경험 고도화
3. 추가 기능 구현

## 참고 자료

### 디자인 시스템 및 템플릿
- [Figma Community](https://www.figma.com/community) - 대시보드 UI 킷
- [Untitled UI](https://www.untitledui.com/components/file-uploaders) - Figma 컴포넌트
- [Envato Elements](https://elements.envato.com/graphic-templates/dashboard+ui) - 대시보드 템플릿

### 기술 레퍼런스
- [React MUI 파일 업로드](https://www.dhiwise.com/post/how-to-implement-react-mui-file-upload-in-your-applications)
- [Bulk Upload UX Case Study](https://medium.com/design-bootcamp/ux-case-study-bulk-upload-feature-785803089328)
- [모바일 모달 베스트 프랙티스](https://www.appcues.com/blog/mobile-app-modal-windows)

---

*본 문서는 RecordRoute 프로젝트의 현재 코드베이스(`frontend/src/*`)를 기준으로 작성되었으며, 레거시 참조는 `frontend/legacy/*`에 보관되어 있습니다.*
