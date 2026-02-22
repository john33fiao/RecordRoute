# 화자 분리 평가셋 매니페스트

- 목적: 화자 분리(Speaker Diarization) 성능을 재현 가능하게 검증하기 위한 입력셋/정답 라벨 위치를 고정한다.
- 경로 원칙: 프로젝트 내부 저장 경로는 `DB/...` alias 형식을 사용한다.
- 갱신 규칙: 평가셋/라벨이 변경되면 버전 식별자를 갱신하고, 본 문서 및 `TODO/화자분리_로드맵.md`의 DoD 링크를 함께 업데이트한다.

## 평가셋 목록

| 구분 | 저장 위치 | 파일 수 / 총 길이 / 샘플레이트 | 정답 라벨 형식/경로 | 버전 식별자 |
| --- | --- | --- | --- | --- |
| 1:1 (저잡음 인터뷰) | `DB/datasets/diarization/eval-1on1/` | 24개 / 02:18:40 / 16 kHz mono | RTTM + JSON (`DB/datasets/diarization/eval-1on1/labels/`) | `eval-1on1@2026-02-18` + SHA256 manifest `8b5b3a5f...` |
| 3~5인 (회의/토론) | `DB/datasets/diarization/eval-group-3to5/` | 18개 / 03:42:15 / 16 kHz mono | RTTM (`DB/datasets/diarization/eval-group-3to5/labels/`) | `eval-group-3to5@2026-02-18` + SHA256 manifest `31f57f9a...` |
| 잡음환경 (카페/야외/원거리) | 외부 저장소 `s3://recordroute-eval/diarization/noisy-v1/` (로컬 미러: `DB/datasets/diarization/eval-noisy/`) | 20개 / 02:55:10 / 16 kHz mono (원본 48 kHz 리샘플) | RTTM + JSON (`s3://recordroute-eval/diarization/noisy-v1/labels/`, 미러: `DB/datasets/diarization/eval-noisy/labels/`) | `noisy-v1.0.0` (tag) / export date `2026-02-18` |

## 재현 절차 메모

1. 평가 실행 전 `DB/datasets/diarization/*/MANIFEST.sha256` 파일로 음원/라벨 무결성을 검증한다.
2. 외부 저장소 평가셋은 로컬 미러 동기화 후 동일 해시를 확인한다.
3. 결과 리포트에는 본 문서의 버전 식별자 문자열을 그대로 기록한다.

## 개인정보/민감정보 처리 원칙

- 보존기간: 원본 음성 및 정답 라벨은 수집 목적 달성 후 최대 90일 보관, 이후 파기/익명화본만 유지.
- 마스킹: 사람 이름, 전화번호, 계좌번호, 주소 등 직접 식별자는 전사 텍스트/라벨 메타데이터에서 마스킹 처리 후 저장한다.
- 접근통제: 평가셋 원본은 최소 권한 원칙으로 접근 권한을 제한하고, 외부 저장소는 암호화 저장(SSE) 및 접근 로그를 유지한다.
- 반출통제: 외부 공유 시 원본 오디오 반출을 금지하고, 필요 시 비식별화 샘플 또는 통계치만 제공한다.
