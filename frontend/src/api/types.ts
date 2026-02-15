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
export type TaskStage = 'upload' | 'transform' | 'correct' | 'summary';
export type RetryMode = 'new_task' | 'resume_existing';

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
  retry_mode?: RetryMode;
  retry_of_task_id?: string;
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
  file_type?: string[];
  status?: 'completed' | 'pending';
  status_task?: 'stt' | 'summary' | 'embedding';
}

export interface SearchResponse {
  keywordMatches: KeywordMatch[];
  similarDocuments: SimilarDocument[];
  query?: string;
  limit?: number;
  sort?: { by: string; order: 'asc' | 'desc' };
  filters?: { start_date?: string | null; end_date?: string | null; min_score?: number | null; file_type?: string[]; status?: string | null; status_task?: string | null };
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
  score?: number;
  snippet?: string;
}

export interface SimilarDocument {
  file_uuid?: string;
  file: string;
  display_name?: string;
  score: number;
  snippet?: string;
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

export interface SimilarityGraphNode {
  id: string;
  label: string;
  file?: string;
  record_id?: string | null;
  uploaded_at?: string | null;
}

export interface SimilarityEdge {
  source: string;
  target: string;
  weight: number;
}

export interface SimilarityGraphResponse {
  nodes: SimilarityGraphNode[];
  edges: SimilarityEdge[];
  meta?: Record<string, unknown>;
}

export interface SimilarityGraphRequest {
  min_similarity?: number;
  max_neighbors?: number;
  max_nodes?: number;
  sampling?: 'recent' | 'random' | 'hybrid';
  doc_id?: string;
  refresh?: boolean;
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

export interface StandardErrorPayload {
  message: string;
  code?: string | null;
  retryable?: boolean | null;
  failed_step?: string | null;
}

export interface TaskProgress {
  task_id: string;
  message: string;
  stage?: TaskStage;
  progress_percent?: number;
  eta_seconds?: number | null;
  error_code?: string;
  retryable?: boolean;
  failed_step?: string;
  error?: StandardErrorPayload | null;
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
  retryOfTaskId?: string;
  modelInfo?: string;
  errorCode?: string;
  retryable?: boolean;
  failedStep?: string;
  stage?: TaskStage;
  progressPercent?: number;
  etaSeconds?: number | null;
  error?: StandardErrorPayload | null;
}

export interface ModelSettings {
  transcribe: string;
  summarize: string;
  embedding: string;
  language: string;
}
