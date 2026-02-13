import type {
  UploadResult,
  HistoryRecord,
  SearchResponse,
  SimilarDocument,
  ModelsResponse,
  RunningTask,
  ModelSettings,
} from './types';

// Upload files (multipart/form-data)
export async function uploadFiles(files: File[], signal?: AbortSignal): Promise<UploadResult[]> {
  const formData = new FormData();
  files.forEach(file => formData.append('files', file));
  const res = await fetch('/upload', { method: 'POST', body: formData, signal });
  if (!res.ok) throw new Error('Upload failed');
  return res.json();
}

// Process a task (STT, embedding, summary)
export async function processTask(
  filePath: string,
  steps: string[],
  recordId?: string,
  taskId?: string,
  modelSettings?: Partial<ModelSettings>,
  signal?: AbortSignal
): Promise<any> {
  const res = await fetch('/process', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      file_path: filePath,
      steps,
      record_id: recordId,
      task_id: taskId,
      model_settings: modelSettings,
    }),
    signal,
  });
  return res.json();
}

// Cancel a running task
export async function cancelTask(taskId: string): Promise<void> {
  await fetch('/cancel', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ task_id: taskId }),
  });
}

// Get upload history
export async function getHistory(): Promise<HistoryRecord[]> {
  const res = await fetch('/history');
  if (!res.ok) throw new Error('Failed to fetch history');
  return res.json();
}

// Delete multiple records
export async function deleteRecords(recordIds: string[]): Promise<any> {
  const res = await fetch('/delete_records', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ record_ids: recordIds }),
  });
  return res.json();
}

// Reset a single record
export async function resetRecord(recordId: string): Promise<void> {
  await fetch('/reset', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ record_id: recordId }),
  });
}

// Reset all tasks by type
export async function resetAllTasks(tasks: string[]): Promise<any> {
  const res = await fetch('/reset_all_tasks', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ tasks }),
  });
  return res.json();
}

// Update filename
export async function updateFilename(recordId: string, filename: string): Promise<void> {
  await fetch('/update_filename', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ record_id: recordId, filename }),
  });
}

// Update STT text
export async function updateSttText(fileIdentifier: string, content: string): Promise<any> {
  const res = await fetch('/update_stt_text', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file_identifier: fileIdentifier, content }),
  });
  return res.json();
}

// Check existing STT
export async function checkExistingStt(
  filePath: string
): Promise<{ has_stt: boolean; stt_file?: string }> {
  const res = await fetch('/check_existing_stt', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file_path: filePath }),
  });
  return res.json();
}

// Reset summary and embedding
export async function resetSummaryEmbedding(recordId: string): Promise<any> {
  const res = await fetch('/reset_summary_embedding', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ record_id: recordId }),
  });
  return res.json();
}

// Search (keyword + vector)
export async function search(
  query: string,
  startDate?: string,
  endDate?: string,
  options?: {
    sortBy?: string;
    sortOrder?: "asc" | "desc";
    minScore?: number;
    page?: number;
    pageSize?: number;
  }
): Promise<SearchResponse> {
  const params = new URLSearchParams({ q: query });
  if (startDate) params.set('start', startDate);
  if (endDate) params.set('end', endDate);
  if (options?.sortBy) params.set('sort_by', options.sortBy);
  if (options?.sortOrder) params.set('sort_order', options.sortOrder);
  if (options?.minScore !== undefined) params.set('min_score', String(options.minScore));
  if (options?.page !== undefined) params.set('page', String(options.page));
  if (options?.pageSize !== undefined) params.set('page_size', String(options.pageSize));
  const res = await fetch(`/search?${params}`);
  if (!res.ok) throw new Error('Search failed');
  return res.json();
}

// Get similar documents
export async function getSimilarDocs(
  fileIdentifier: string,
  userFilename?: string,
  refresh?: boolean
): Promise<SimilarDocument[]> {
  const res = await fetch('/similar', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({
      file_identifier: fileIdentifier,
      user_filename: userFilename,
      refresh,
    }),
  });
  return res.json();
}

// Delete a file (stt/summary/embedding)
export async function deleteFile(fileIdentifier: string, fileType: string): Promise<void> {
  await fetch('/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file_identifier: fileIdentifier, file_type: fileType }),
  });
}

// Get download URL
export function getDownloadUrl(fileIdentifier: string): string {
  return `/download/${encodeURIComponent(fileIdentifier)}`;
}

// Get available models
export async function getModels(): Promise<ModelsResponse> {
  const res = await fetch('/models');
  if (!res.ok) throw new Error('Failed to fetch models');
  return res.json();
}

// Shutdown server
export async function shutdown(): Promise<void> {
  await fetch('/shutdown', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({}),
  });
}

// Get running tasks
export async function getRunningTasks(): Promise<RunningTask[]> {
  const res = await fetch('/tasks');
  return res.json();
}

// Get task progress
export async function getProgress(
  taskId: string
): Promise<{ task_id: string; message: string }> {
  const res = await fetch(`/progress/${taskId}`);
  return res.json();
}

// Download file content as text
export async function downloadFileAsText(fileIdentifier: string): Promise<string> {
  const res = await fetch(getDownloadUrl(fileIdentifier));
  if (!res.ok) throw new Error('Download failed');
  return res.text();
}
