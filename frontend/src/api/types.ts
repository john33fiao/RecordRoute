export type FileType = 'audio' | 'document' | 'other';
export type ViewerFileType = 'stt' | 'summary';

export enum QueueTaskStatus {
  Pending = 'pending',
  Queued = 'queued',
  Processing = 'processing',
  Completed = 'completed',
  Error = 'error',
  Cancelled = 'cancelled',
}

export type QueueSortMode = 'category' | 'order';
export type TaskType = 'stt' | 'embedding' | 'summary';

export interface AsyncState<T> {
  data: T;
  loading: boolean;
  error: string | null;
  stale: boolean;
}

export interface UploadResult {
  file_path: string;
  file_type: FileType;
  record_id: string;
  duplicate?: boolean;
  original_record_id?: string;
  filename?: string;
}

export interface HistoryRecord {
  id: string;
  filename: string;
  file_type: FileType;
  timestamp: string;
  file_hash: string;
  duration?: string;
  file_path: string;
  completed_tasks: Partial<Record<TaskType, boolean>>;
  download_links: Partial<Record<ViewerFileType, string>>;
  info?: Record<string, unknown>;
}

export interface ProcessResult {
  accepted?: boolean;
  task_id?: string;
  stt?: string;
  summary?: string;
  correct?: string;
  error?: string;
  error_code?: string;
  retryable?: boolean;
  failed_step?: string;
}


export interface SearchRequest {
  query: string;
  limit?: number;
  start_date?: string;
  end_date?: string;
  sort_by?: 'similarity' | 'date';
  sort_order?: 'asc' | 'desc';
  min_score?: number;
  page?: number;
  page_size?: number;
  include_timing?: boolean;
}

export interface SearchResponse {
  keywordMatches: KeywordMatch[];
  similarDocuments: SimilarDocument[];
  query?: string;
  limit?: number;
  sort?: { by: string; order: 'asc' | 'desc' };
  filters?: { start_date?: string | null; end_date?: string | null; min_score?: number | null };
  scoreBreakdown?: { keywordWeight: number; vectorWeight: number };
  pagination?: { page: number; pageSize: number; returned: number; hasNext: boolean };
  timing?: Record<string, number>;
  performanceTargetMs?: Record<string, number>;
  cache?: { hit: boolean };
}

export interface KeywordMatch {
  file: string;
  file_uuid: string;
  uploaded_at: string;
  source_filename: string;
  display_name: string;
  count: number;
}

export interface SimilarDocument {
  file_uuid?: string;
  file: string;
  display_name?: string;
  score: number;
  uploaded_at?: string;
  source_filename?: string;
  link: string;
  record_id?: string;
  score_breakdown?: {
    vector_similarity: number;
    keyword_overlap: number;
    composite: number;
  };
}

export interface ModelsResponse {
  models: string[];
  default: {
    whisper: string;
    summarize: string;
    embedding: string;
  };
}

export interface RunningTask {
  task_id: string;
  status: QueueTaskStatus | string;
  message?: string;
  [key: string]: unknown;
}

export interface TaskProgress {
  task_id: string;
  message: string;
  error_code?: string;
  retryable?: boolean;
  failed_step?: string;
}

export interface QueueTask {
  id: string;
  recordId: string;
  filePath: string;
  taskType: TaskType;
  status: QueueTaskStatus;
  progress: string;
  order: number;
  taskId: string;
  abortController?: AbortController;
  lastRetryTime?: number;
  retryCount?: number;
  modelInfo?: string;
  errorCode?: string;
  retryable?: boolean;
  failedStep?: string;
}

export interface ModelSettings {
  transcribe: string;
  summarize: string;
  embedding: string;
  language: string;
}
