import type {
  UploadResult,
  HistoryRecord,
  SearchResponse,
  SearchRequest,
  SimilarDocument,
  ModelsResponse,
  RunningTask,
  ModelSettings,
  ProcessResult,
  RetryMode,
  TaskProgress,
  ViewerFileType,
  SimilarityGraphRequest,
  SimilarityGraphResponse,
  DestructiveApiAuth,
  SegmentItem,
} from './types';
import { resolveApiBaseUrl } from '../runtime/endpoints';

interface ApiRequestOptions extends RequestInit {
  skipJson?: boolean;
}

const API_BASE_URL = resolveApiBaseUrl();

function withApiBase(path: string): string {
  if (!path.startsWith('/')) return path;
  return API_BASE_URL ? `${API_BASE_URL}${path}` : path;
}


function buildDestructiveApiAuthOptions(auth?: DestructiveApiAuth): { headers?: Record<string, string>; body?: Record<string, string> } {
  if (!auth) return {};

  const headers: Record<string, string> = {};
  const body: Record<string, string> = {};

  if (auth.adminToken) {
    headers['X-RecordRoute-Admin-Token'] = auth.adminToken;
    body.admin_token = auth.adminToken;
  }
  if (auth.sessionId) {
    headers['X-RecordRoute-Session-Id'] = auth.sessionId;
    body.session_id = auth.sessionId;
  }
  if (auth.sessionToken) {
    headers['X-RecordRoute-Session-Token'] = auth.sessionToken;
    body.session_token = auth.sessionToken;
  }

  return {
    headers: Object.keys(headers).length ? headers : undefined,
    body: Object.keys(body).length ? body : undefined,
  };
}

async function apiRequest<T>(input: RequestInfo | URL, init: ApiRequestOptions = {}): Promise<T> {
  const { skipJson, ...requestInit } = init;
  const requestInput = typeof input === 'string' ? withApiBase(input) : input;
  const res = await fetch(requestInput, requestInit);
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
  signal?: AbortSignal,
  retryMode: RetryMode = 'new_task',
  retryOfTaskId?: string,
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
      retry_mode: retryMode,
      retry_of_task_id: retryOfTaskId,
    }),
    signal,
  });
}



export async function getSttSegments(fileIdentifier: string): Promise<SegmentItem[]> {
  return apiRequest<SegmentItem[]>(`/segments/${encodeURIComponent(fileIdentifier)}`);
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

export const deleteRecords = (recordIds: string[], auth?: DestructiveApiAuth) => {
  const authOptions = buildDestructiveApiAuthOptions(auth);
  return apiRequest('/delete_records', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...(authOptions.headers ?? {}) },
    body: JSON.stringify({ record_ids: recordIds, ...(authOptions.body ?? {}) }),
  });
};

export async function resetRecord(recordId: string, auth?: DestructiveApiAuth): Promise<void> {
  const authOptions = buildDestructiveApiAuthOptions(auth);
  await apiRequest('/reset', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...(authOptions.headers ?? {}) },
    body: JSON.stringify({ record_id: recordId, ...(authOptions.body ?? {}) }),
    skipJson: true,
  });
}

export const resetAllTasks = (tasks: string[], auth?: DestructiveApiAuth) => {
  const authOptions = buildDestructiveApiAuthOptions(auth);
  return apiRequest('/reset_all_tasks', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...(authOptions.headers ?? {}) },
    body: JSON.stringify({ tasks, ...(authOptions.body ?? {}) }),
  });
};

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

export const resetSummaryEmbedding = (recordId: string, auth?: DestructiveApiAuth) => {
  const authOptions = buildDestructiveApiAuthOptions(auth);
  return apiRequest('/reset_summary_embedding', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...(authOptions.headers ?? {}) },
    body: JSON.stringify({ record_id: recordId, ...(authOptions.body ?? {}) }),
  });
};

export async function search(request: SearchRequest): Promise<SearchResponse> {
  const params = new URLSearchParams({ query: request.query });
  if (request.limit !== undefined) params.set('limit', String(request.limit));
  if (request.start_date) params.set('start_date', request.start_date);
  if (request.end_date) params.set('end_date', request.end_date);
  if (request.sort_by) params.set('sort_by', request.sort_by);
  if (request.sort_order) params.set('sort_order', request.sort_order);
  if (request.min_score !== undefined) params.set('min_score', String(request.min_score));
  if (request.page !== undefined) params.set('page', String(request.page));
  if (request.page_size !== undefined) params.set('page_size', String(request.page_size));
  if (request.include_timing !== undefined) params.set('include_timing', String(request.include_timing));
  if (request.file_type?.length) params.set('file_type', request.file_type.join(','));
  if (request.status) params.set('status', request.status);
  if (request.status_task) params.set('status_task', request.status_task);
  return apiRequest<SearchResponse>(`/search?${params}`);
}


export async function getSimilarityGraph(request: SimilarityGraphRequest = {}): Promise<SimilarityGraphResponse> {
  const params = new URLSearchParams();
  if (request.min_similarity !== undefined) params.set('min_similarity', String(request.min_similarity));
  if (request.max_neighbors !== undefined) params.set('max_neighbors', String(request.max_neighbors));
  if (request.max_nodes !== undefined) params.set('max_nodes', String(request.max_nodes));
  if (request.sampling) params.set('sampling', request.sampling);
  if (request.neighbor_strategy) params.set('neighbor_strategy', request.neighbor_strategy);
  if (request.doc_id) params.set('doc_id', request.doc_id);
  if (request.doc_types?.length) params.set('doc_types', request.doc_types.join(','));
  if (request.start_date) params.set('start_date', request.start_date);
  if (request.end_date) params.set('end_date', request.end_date);
  if (request.keyword) params.set('keyword', request.keyword);
  if (request.refresh !== undefined) params.set('refresh', String(request.refresh));
  const query = params.toString();
  return apiRequest<SimilarityGraphResponse>(`/api/similarity-graph${query ? `?${query}` : ''}`);
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

export async function deleteFile(fileIdentifier: string, fileType: ViewerFileType, auth?: DestructiveApiAuth): Promise<void> {
  const authOptions = buildDestructiveApiAuthOptions(auth);
  await apiRequest('/delete', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...(authOptions.headers ?? {}) },
    body: JSON.stringify({ file_identifier: fileIdentifier, file_type: fileType, ...(authOptions.body ?? {}) }),
    skipJson: true,
  });
}

export const getDownloadUrl = (fileIdentifier: string): string => withApiBase(`/download/${encodeURIComponent(fileIdentifier)}`);

export const getModels = (provider?: 'ollama' | 'llamacpp') => {
  const query = provider ? `?provider=${encodeURIComponent(provider)}` : '';
  return apiRequest<ModelsResponse>(`/models${query}`);
};

export async function shutdown(auth?: DestructiveApiAuth): Promise<void> {
  const authOptions = buildDestructiveApiAuthOptions(auth);
  await apiRequest('/shutdown', {
    method: 'POST',
    headers: { 'Content-Type': 'application/json', ...(authOptions.headers ?? {}) },
    body: JSON.stringify({ ...(authOptions.body ?? {}) }),
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
