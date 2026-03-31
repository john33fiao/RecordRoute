# Deprecated: API TODO

이 문서는 더 이상 활성 설계 문서가 아니다.
초기 API 확장 TODO를 남겨 둔 기록이며, 현재 구현과 1:1로 맞지 않을 수 있다.

## 현재 기준 문서

API를 확인할 때는 아래 문서를 우선한다.

- `docs/API_Doc.md`
- `docs/openapi.yaml`
- `docs/architecture.md`
- `docs/embeddings.md`

## 왜 deprecated 인가

초기 TODO 시점 이후 다음 항목들이 이미 구현됐다.

- completed jobs 조회
- source path 기준 조회
- STT progress 조회
- summary embedding 제출/조회
- summary similarity search
- llama embedding 준비 상태 노출

또한 summary canonical 파일명은 현재 `summary/result.md`이며, 과거 TODO에 남아 있던 `summary/result.txt` 기준 설명은 현행 구현과 다르다.

## 보존 목적

이 문서는 "초기 API가 어떤 순서로 확장됐는가"를 보는 역사 자료로만 유지한다.
새 TODO를 추가하거나 현재 동작 설명을 적는 용도로는 사용하지 않는다.
