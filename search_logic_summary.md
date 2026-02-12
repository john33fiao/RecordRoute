Similar Documents Search Logic:

Frontend:
- `SimilarDocsDialog.tsx`: Calls `api.getSimilarDocs` -> `POST /similar`
- `HistoryPanel.tsx`: Needs to call `onShowSimilarDocs` when 'embedding' task is completed.

Backend (`server.py`):
- `POST /similar`: Accepts `file_identifier`.
    - If UUID: Look up in registry.
    - If Path: Normalize path.
    - Reads file content.
    - Calls `search_vectors(content, ...)`
    - Maps hits back to registry/history to get display names and download links.
    - Returns list of similar documents.

Issue:
- `HistoryPanel.tsx` was not handling the `embedding` task type completion, so clicking the button did nothing.

Fix:
- Updated `HistoryPanel.tsx` to call `onShowSimilarDocs` when `taskType === 'embedding'`.
