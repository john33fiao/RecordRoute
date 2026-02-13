import type {
  UploadResult,
  HistoryRecord,
  SearchResponse,
  SimilarDocument,
  ModelsResponse,
  RunningTask,
  ModelSettings,
  ProcessResult,
  TaskProgress,
  ViewerFileType,
} from './types';

interface ApiRequestOptions extends RequestInit {
  skipJson?: boolean;
}

async function apiRequest<T>(input: RequestInfo | URL, init: ApiRequestOptions = {}): Promise<T> {
  const { skipJson, ...requestInit } = init;
  const res = await fetch(input, requestInit);
  if (!res.ok) {
    const body = await res.text().catch(() => '');
    throw new Error(body || `API request failed (${res.status})`);
  }
  if (skipJson) return undefined as T;
  return res.json() as Promise<T>;
}

export async function uploadFiles(files: File[], signal?: AbortSignal): Promise<UploadResult[]> {
  const formData = new FormData();
  files.forEach(file => formData.append('files', file));
  return apiRequest<UploadResult[]>('/upload', { method: 'POST', body: formData, signal });
}

export async function processTask(
  filePath: string,
  steps: string[],
  recordId?: string,
  taskId?: string,
  modelSettings?: Partial<ModelSettings>,
  signal?: AbortSignal
): Promise<ProcessResult> {
  return apiRequest<ProcessResult>('/process', {
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
}

export async function cancelTask(taskId: string): Promise<void> {
  await apiRequest('/cancel', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ task_id: taskId }),
    skipJson: true,
  });
}

export const getHistory = () => apiRequest<HistoryRecord[]>('/history');

export const deleteRecords = (recordIds: string[]) => apiRequest('/delete_records', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ record_ids: recordIds }),
});

export async function resetRecord(recordId: string): Promise<void> {
  await apiRequest('/reset', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ record_id: recordId }),
    skipJson: true,
  });
}

export const resetAllTasks = (tasks: string[]) => apiRequest('/reset_all_tasks', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ tasks }),
});

export async function updateFilename(recordId: string, filename: string): Promise<void> {
  await apiRequest('/update_filename', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ record_id: recordId, filename }),
    skipJson: true,
  });
}

export const updateSttText = (fileIdentifier: string, content: string) => apiRequest('/update_stt_text', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ file_identifier: fileIdentifier, content }),
});

export const checkExistingStt = (filePath: string) => apiRequest<{ has_stt: boolean; stt_file?: string }>('/check_existing_stt', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ file_path: filePath }),
});

export const resetSummaryEmbedding = (recordId: string) => apiRequest('/reset_summary_embedding', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({ record_id: recordId }),
});

export async function search(
  query: string,
  startDate?: string,
  endDate?: string,
  options?: {
    sortBy?: string;
    sortOrder?: 'asc' | 'desc';
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
  return apiRequest<SearchResponse>(`/search?${params}`);
}

export const getSimilarDocs = (
  fileIdentifier: string,
  userFilename?: string,
  refresh?: boolean
): Promise<SimilarDocument[]> => apiRequest('/similar', {
  method: 'POST',
  headers: { 'Content-Type': 'application/json' },
  body: JSON.stringify({
    file_identifier: fileIdentifier,
    user_filename: userFilename,
    refresh,
  }),
});

export async function deleteFile(fileIdentifier: string, fileType: ViewerFileType): Promise<void> {
  await apiRequest('/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({ file_identifier: fileIdentifier, file_type: fileType }),
    skipJson: true,
  });
}

export const getDownloadUrl = (fileIdentifier: string): string => `/download/${encodeURIComponent(fileIdentifier)}`;

export const getModels = () => apiRequest<ModelsResponse>('/models');

export async function shutdown(): Promise<void> {
  await apiRequest('/shutdown', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json' },
    body: JSON.stringify({}),
    skipJson: true,
  });
}

export const getRunningTasks = () => apiRequest<RunningTask[]>('/tasks');

export const getProgress = (taskId: string) => apiRequest<TaskProgress>(`/progress/${taskId}`);

export async function downloadFileAsText(fileIdentifier: string): Promise<string> {
  const res = await fetch(getDownloadUrl(fileIdentifier));
  if (!res.ok) throw new Error('Download failed');
  return res.text();
}
