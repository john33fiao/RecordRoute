# Deprecated: Async API TODO

이 문서는 비동기 stage API 전환 초기에 사용한 TODO 메모다.
현재는 ffmpeg, stt, summary, embedding 모두 job/task 상태를 기준으로 비동기 제출과 polling 조회가 가능하므로 활성 문서가 아니다.

## 현재 상태 요약

이미 코드에 반영된 항목:

- `POST /jobs`
- `POST /jobs/{job_id}/stt`
- `POST /jobs/{job_id}/summary`
- `POST /jobs/{job_id}/summary/embedding`
- task 상태 조회와 progress polling
- 파일 목록/본문/다운로드 조회

현재 기준 문서:

- `docs/openapi.yaml`
- `docs/architecture.md`

## 남겨 두는 이유

초기 비동기 전환 범위를 회고하는 참고 자료로는 가치가 있지만, 현재 API 스펙이나 남은 TODO의 기준으로 사용하면 안 된다.
