// Upload
export interface UploadResult {
  file_path: string;
  file_type: 'audio' | 'document' | 'other';
  record_id: string;
  duplicate?: boolean;
  original_record_id?: string;
  filename?: string;
}

// History record from GET /history
// History record from GET /history
export interface HistoryRecord {
  id: string;
  filename: string;
  file_type: 'audio' | 'document' | 'other';
  timestamp: string;
  file_hash: string;
  duration?: string;
  file_path: string;
  completed_tasks: Record<string, boolean>;
  download_links: Record<string, string>;
  info?: Record<string, any>;
}

// Process task
export interface ProcessResult {
  stt?: string;
  summary?: string;
  error?: string;
}

// Search result from GET /search
export interface SearchResponse {
  keywordMatches: KeywordMatch[];
  similarDocuments: SimilarDocument[];
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
}

// Models from GET /models
export interface ModelsResponse {
  models: string[];
  default: {
    whisper: string;
    summarize: string;
    embedding: string;
  };
}

// Running task from GET /tasks
export interface RunningTask {
  task_id: string;
  status: string;
  [key: string]: any;
}

// Task queue item (client-side)
export type TaskType = 'stt' | 'embedding' | 'summary';

export interface QueueTask {
  id: string;
  recordId: string;
  filePath: string;
  taskType: TaskType;
  status: 'pending' | 'queued' | 'processing' | 'completed' | 'error';
  progress: string;
  order: number;
  taskId: string;
  abortController?: AbortController;
  lastRetryTime?: number;
  retryCount?: number;
  modelInfo?: string;
}

// Model settings (stored in localStorage)
export interface ModelSettings {
  transcribe: string;
  summarize: string;
  embedding: string;
  language: string;
}
