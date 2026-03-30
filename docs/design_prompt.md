# RecordRoute Figma Make 디자인 프롬프트

아래 프롬프트를 Figma Make에 그대로 붙여 넣어 사용합니다.

## Prompt

```md
RecordRoute Web Console의 메인 화면을 재설계해 주세요.

이 제품은 오디오 파일을 업로드하고, ffmpeg -> stt -> summary -> embedding 파이프라인을 관리하며, queue 상태와 검색 결과를 운영자가 한 화면에서 확인하는 내부 운영 콘솔입니다. 현재는 여러 패널이 한 페이지에 동시에 펼쳐져 있지만, 이번 디자인에서는 정보구조를 더 명확하게 재구성하고 시각적 완성도를 높이고 싶습니다.

핵심 목표:
- 기존 기능은 유지하되, 상단 탭 중심 구조로 재정렬합니다.
- TailwindCSS theme로 바로 옮기기 쉬운 토큰 중심 디자인으로 만듭니다.
- light-first의 Modern SaaS 톤으로 구성합니다.
- 과하게 장식적이기보다, 상태와 액션이 빠르게 읽히는 운영 콘솔이어야 합니다.

정보구조 요구사항:
- 기존 상위 섹션 `Upload`, `Jobs`, `Search`, `Queue`, `Dictionary`를 상단 탭 버튼으로 분리해 주세요.
- `Selected Job`은 별도 탭으로 두지 말고, `Jobs` 탭 내부의 상세 영역으로 통합해 주세요.
- `Jobs` 탭에서는 좌측에 Job 목록, 우측 또는 하단에 선택된 Job 상세를 보여주는 master-detail 구조를 사용해 주세요.
- 선택된 Job 상세 안에는 현재 기능을 유지한 채 `Overview`, `STT`, `Summary`, `Embedding`, `Files` 영역이 자연스럽게 정리되어야 합니다.

설정 팝업 요구사항:
- 현재 메인 화면에 있는 `런타임과 모델 준비 상태` 패널은 메인 캔버스에서 제거하고, 우측 상단의 `Settings` 버튼으로 여는 설정 팝업 안으로 이동해 주세요.
- 현재 우측 상단에 있는 서버 준비 상태/서버 메타 영역도 설정 팝업 안으로 함께 이동해 주세요.
- 설정 팝업 안에는 다음 정보와 액션이 모두 포함되어야 합니다:
  - 서버 주소, 서버 준비 상태, 글로벌 상태 캡션
  - 런타임 readiness 카드: FFmpeg, Whisper, Llama, Whisper Model, Llama Model, Embedding Model
  - 모델 준비 상태 행: Whisper, Llama
  - 각 모델의 준비 상태 badge, 오류/보조 설명, prepare 액션 버튼
  - 새로고침 액션
- 설정 팝업은 단순 정보 모달이 아니라, 운영자가 상태를 확인하고 바로 조치할 수 있는 control surface처럼 보여야 합니다.

시각 방향:
- Modern SaaS 스타일이지만 너무 generic하지 않게 해 주세요.
- 밝은 배경 기반, 높은 가독성, 적당한 깊이감, 선명한 상태 컬러 체계를 사용해 주세요.
- 보라색 위주의 흔한 AI 대시보드 느낌은 피하고, 차분한 청록/슬레이트/웜 뉴트럴 계열을 활용해 주세요.
- 다크모드는 이번 범위에서 제외하고, light theme만 설계해 주세요.
- 타이포그래피는 운영 콘솔답게 명료해야 하고, 제목/섹션/상태/메타 정보의 위계가 분명해야 합니다.

레이아웃 요구사항:
- 상단에는 제품 아이덴티티와 짧은 설명이 있는 header가 있어야 합니다.
- header 우측에는 `Settings` 버튼이 있어야 합니다.
- 그 아래에는 탭 바가 있어야 하며, 활성 탭이 명확히 드러나야 합니다.
- `Upload`, `Search`, `Dictionary`는 폼 중심 카드 레이아웃으로 정리해 주세요.
- `Queue`는 칸반형 보드 구조를 유지하되, 열 구분과 active/running 상태가 더 직관적으로 보이게 해 주세요.
- `Jobs`는 가장 중요한 화면으로 다뤄 주세요. 목록과 상세, 작업 상태, 결과 미리보기 간의 관계가 한눈에 들어와야 합니다.
- 모바일에서는 탭 바가 가로 스크롤 가능해야 하며, master-detail 구조는 세로 스택으로 자연스럽게 접혀야 합니다.

컴포넌트/상태 표현 요구사항:
- 탭 버튼 상태: default, hover, active
- 버튼 상태: primary, secondary/ghost, warning/destructive, disabled
- 상태 badge: running, completed, failed, missing, paused, idle
- 리스트/카드 상태: selected, hover
- 설정 팝업 상태: closed, open
- queue card 상태: queued, running
- 빈 상태와 로딩 상태도 디자인 언어 안에서 자연스럽게 표현해 주세요.

TailwindCSS theme 요구사항:
- 결과물이 TailwindCSS의 theme token으로 옮기기 쉬워야 합니다.
- 아래 토큰 계층을 먼저 정의한 뒤 화면에 적용해 주세요:
  - color: `background`, `foreground`, `card`, `card-foreground`, `popover`, `popover-foreground`, `primary`, `primary-foreground`, `secondary`, `secondary-foreground`, `muted`, `muted-foreground`, `accent`, `accent-foreground`, `border`, `input`, `ring`, `success`, `warning`, `destructive`
  - radius: `sm`, `md`, `lg`, `xl`
  - shadow: `sm`, `md`, `lg`
  - spacing rhythm: panel padding, card gap, section gap, tab gap, form gap
  - typography: page title, section title, tab label, body, caption, mono/meta text
- 임의의 one-off 값 남발보다 재사용 가능한 토큰 중심으로 설계해 주세요.
- 나중에 shadcn/ui나 일반 Tailwind 컴포넌트로 옮길 수 있을 정도로 구조가 명확해야 합니다.

산출물 요구사항:
- 메인 콘솔 기본 상태 1안
- `Jobs` 탭이 활성화된 상태 1안
- 설정 팝업이 열린 상태 1안
- Tailwind theme token 표 1개
- 핵심 컴포넌트 상태 샘플: tabs, buttons, badges, cards, modal

제약사항:
- 존재하지 않는 새 기능을 발명하지 말아 주세요.
- 현재 있는 기능과 정보만 더 읽기 좋고 더 구조적으로 재배치해 주세요.
- 시스템/모델/서버 상태는 메인 화면이 아니라 반드시 설정 팝업 안에만 있어야 합니다.
- 운영 콘솔이므로 장식보다 명확성, 스캔 속도, 상태 전달력이 우선입니다.
```

## 참고 메모

- 현재 화면의 SoT는 `frontend/`가 아니라 `rust/web/` 정적 웹 콘솔입니다.
- 현재 상위 섹션은 `System / Models`, `Upload`, `Jobs`, `Selected Job`, `Search`, `Queue`, `Dictionary`입니다.
- 이번 프롬프트의 기본 가정은 다음과 같습니다:
  - `Selected Job`은 `Jobs` 탭 내부 상세 영역으로 통합
  - `Settings`는 우측 상단 트리거로 여는 모달 팝업
  - light-first, Modern SaaS, Tailwind token 중심 산출물
