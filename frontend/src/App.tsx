import { useEffect, useState } from 'react';
import {
  Clock,
  History,
  Loader2,
  Pause,
  Play,
  Search,
  Settings,
  Sparkles,
  Square,
  Tags,
  Upload,
} from 'lucide-react';
import { UploadSection } from './components/UploadSection';
import { JobQueue } from './components/JobQueue';
import { HistoryPanel } from './components/HistoryPanel';
import { SearchPanel } from './components/SearchPanel';
import { DictionaryPanel } from './components/DictionaryPanel';
import { SettingsDialog } from './components/SettingsDialog';
import { Button } from './components/ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './components/ui/tabs';
import { ThemeProvider, useTheme } from './contexts/ThemeContext';

type StatusTone = 'success' | 'error' | 'info';

interface QueueEntryResponse {
  job_id: string;
  task_type: string;
  category: string;
  queued_at: string;
}

interface QueueBatchResponse {
  category: string;
  running: QueueEntryResponse | null;
  entries: QueueEntryResponse[];
}

interface QueueStatusResponse {
  paused: boolean;
  burst_limit: number;
  active_batch: QueueBatchResponse | null;
  pending_batches: QueueBatchResponse[];
}

interface BatchQueueSubmissionResponse {
  total_jobs: number;
  ffmpeg_queued: number;
  stt_queued: number;
  summary_queued: number;
  embedding_queued: number;
}

interface QueueCancelPendingResponse {
  total_cancelled: number;
  ffmpeg_cancelled: number;
  stt_cancelled: number;
  summary_cancelled: number;
  embedding_cancelled: number;
}

interface QueueActionMessage {
  tone: StatusTone;
  text: string;
}

interface QueueControlLoadingState {
  batchProcess: boolean;
  cancelPending: boolean;
  pauseToggle: boolean;
}

function tryParseJson(text: string) {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init);
  const text = await response.text();
  const payload = text ? tryParseJson(text) : null;

  if (!response.ok) {
    const message =
      payload && typeof payload === 'object' && 'message' in payload
        ? String(payload.message)
        : `${response.status} ${response.statusText}`;
    throw new Error(message);
  }

  return payload as T;
}

function buildErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

function countQueuedEntries(queueStatus: QueueStatusResponse | null) {
  if (!queueStatus) {
    return 0;
  }

  const activeQueued = queueStatus.active_batch?.entries.length ?? 0;
  const pendingQueued = queueStatus.pending_batches.reduce(
    (count, batch) => count + batch.entries.length,
    0,
  );

  return activeQueued + pendingQueued;
}

function queueMessageClass(theme: 'light' | 'dark', tone: StatusTone) {
  const palette = {
    success:
      theme === 'dark'
        ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-200'
        : 'border-emerald-300 bg-emerald-50 text-emerald-800',
    error:
      theme === 'dark'
        ? 'border-red-500/30 bg-red-500/10 text-red-200'
        : 'border-red-300 bg-red-50 text-red-800',
    info:
      theme === 'dark'
        ? 'border-slate-700 bg-slate-800/80 text-slate-300'
        : 'border-slate-300 bg-slate-50 text-slate-700',
  } satisfies Record<StatusTone, string>;

  return palette[tone];
}

function buildQueuePauseLabel(
  queueStatusLoaded: boolean,
  queueStatus: QueueStatusResponse | null,
  isLoading: boolean,
) {
  if (isLoading) {
    return queueStatus?.paused ? '재시작 중...' : '일시정지 중...';
  }

  if (!queueStatusLoaded || !queueStatus) {
    return '재시작/일시정지';
  }

  return queueStatus.paused ? '재시작' : '일시정지';
}

function AppContent() {
  const [activeTab, setActiveTab] = useState('upload');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [queueStatus, setQueueStatus] = useState<QueueStatusResponse | null>(null);
  const [queueStatusLoaded, setQueueStatusLoaded] = useState(false);
  const [queueMessage, setQueueMessage] = useState<QueueActionMessage | null>(null);
  const [queueLoading, setQueueLoading] = useState<QueueControlLoadingState>({
    batchProcess: false,
    cancelPending: false,
    pauseToggle: false,
  });
  const { theme } = useTheme();

  const refreshQueueStatus = async ({
    signal,
    onError,
  }: {
    signal?: AbortSignal;
    onError?: string;
  } = {}) => {
    try {
      const snapshot = await fetchJson<QueueStatusResponse>('/queue', { signal });
      setQueueStatus(snapshot);
      setQueueStatusLoaded(true);
      return snapshot;
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        return null;
      }

      if (!queueStatus) {
        setQueueStatusLoaded(false);
      }
      if (onError) {
        setQueueMessage({
          tone: 'error',
          text: `${onError} ${buildErrorMessage(error, '큐 상태를 불러오지 못했습니다.')}`,
        });
      }
      return null;
    }
  };

  useEffect(() => {
    const controller = new AbortController();
    void refreshQueueStatus({
      signal: controller.signal,
      onError: '큐 상태를 불러오지 못했습니다.',
    });

    return () => controller.abort();
  }, []);

  const pendingQueuedCount = countQueuedEntries(queueStatus);
  const queuePauseLabel = buildQueuePauseLabel(
    queueStatusLoaded,
    queueStatus,
    queueLoading.pauseToggle,
  );

  const handleBatchProcess = async () => {
    setQueueLoading((prev) => ({ ...prev, batchProcess: true }));
    try {
      const payload = await fetchJson<BatchQueueSubmissionResponse>('/jobs/batch-process', {
        method: 'POST',
      });
      setQueueMessage({
        tone: 'success',
        text: [
          '일괄처리 큐 등록 완료',
          `ffmpeg ${payload.ffmpeg_queued}건`,
          `stt ${payload.stt_queued}건`,
          `llm ${payload.summary_queued}건`,
          `embed ${payload.embedding_queued}건`,
        ].join(' · '),
      });
      await refreshQueueStatus();
    } catch (error) {
      setQueueMessage({
        tone: 'error',
        text: buildErrorMessage(error, '일괄처리 시작에 실패했습니다.'),
      });
    } finally {
      setQueueLoading((prev) => ({ ...prev, batchProcess: false }));
    }
  };

  const handleCancelPending = async () => {
    if (pendingQueuedCount === 0) {
      setQueueMessage({
        tone: 'info',
        text: '취소할 대기 작업이 없습니다.',
      });
      return;
    }

    if (!window.confirm(`현재 대기 중인 작업 ${pendingQueuedCount}건을 취소하시겠습니까?`)) {
      return;
    }

    setQueueLoading((prev) => ({ ...prev, cancelPending: true }));
    try {
      const payload = await fetchJson<QueueCancelPendingResponse>('/queue/cancel-pending', {
        method: 'POST',
      });
      setQueueMessage({
        tone: 'info',
        text: [
          `대기 작업 ${payload.total_cancelled}건 취소`,
          `ffmpeg ${payload.ffmpeg_cancelled}건`,
          `stt ${payload.stt_cancelled}건`,
          `summary ${payload.summary_cancelled}건`,
          `embedding ${payload.embedding_cancelled}건`,
        ].join(' · '),
      });
      await refreshQueueStatus();
    } catch (error) {
      setQueueMessage({
        tone: 'error',
        text: buildErrorMessage(error, '전체 종료에 실패했습니다.'),
      });
    } finally {
      setQueueLoading((prev) => ({ ...prev, cancelPending: false }));
    }
  };

  const handleQueuePauseToggle = async () => {
    if (!queueStatusLoaded || !queueStatus) {
      setQueueMessage({
        tone: 'info',
        text: '큐 상태를 확인한 뒤 다시 시도해 주세요.',
      });
      return;
    }

    const nextPaused = !queueStatus.paused;
    setQueueLoading((prev) => ({ ...prev, pauseToggle: true }));
    try {
      const snapshot = await fetchJson<QueueStatusResponse>('/queue/pause', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ paused: nextPaused }),
      });
      setQueueStatus(snapshot);
      setQueueStatusLoaded(true);
      setQueueMessage({
        tone: 'info',
        text: nextPaused
          ? '큐를 일시정지했습니다. 현재 진행 중인 작업만 끝나고 다음 작업은 대기합니다.'
          : '큐를 재시작했습니다. 다음 작업부터 순서대로 이어집니다.',
      });
    } catch (error) {
      setQueueMessage({
        tone: 'error',
        text: buildErrorMessage(error, '재시작/일시정지 전환에 실패했습니다.'),
      });
    } finally {
      setQueueLoading((prev) => ({ ...prev, pauseToggle: false }));
    }
  };

  return (
    <div className={`min-h-screen ${
      theme === 'dark'
        ? 'bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950 text-slate-100'
        : 'bg-gradient-to-br from-slate-50 via-white to-slate-100 text-slate-900'
    }`}>
      <header className={`border-b sticky top-0 z-50 backdrop-blur-xl ${
        theme === 'dark'
          ? 'border-slate-800 bg-slate-900/50'
          : 'border-slate-200 bg-white/50'
      }`}>
        <div className="container mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="relative">
              <div className="absolute inset-0 bg-gradient-to-r from-violet-500 to-fuchsia-500 rounded-lg blur-md opacity-75"></div>
              <div className="relative bg-gradient-to-r from-violet-600 to-fuchsia-600 p-2 rounded-lg">
                <Sparkles className="size-6 text-white" />
              </div>
            </div>
            <div>
              <h1 className="text-2xl font-bold bg-gradient-to-r from-violet-400 to-fuchsia-400 bg-clip-text text-transparent">
                RecordRoute
              </h1>
              <p className={`text-xs ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                음성을 회의록으로 변환
              </p>
            </div>
          </div>

          <button
            className={`p-2 rounded-lg transition-colors ${
              theme === 'dark' ? 'hover:bg-slate-800' : 'hover:bg-slate-100'
            }`}
            onClick={() => setSettingsOpen(true)}
          >
            <Settings className={`size-5 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`} />
          </button>
        </div>
      </header>

      <div className="container mx-auto px-6 py-8">
        <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-6">
          <div className="space-y-3">
            <div className="flex flex-col gap-3 xl:flex-row xl:items-center xl:justify-between">
              <TabsList className={`w-full justify-start overflow-x-auto p-1 border xl:w-fit ${
                theme === 'dark'
                  ? 'bg-slate-900/50 border-slate-800'
                  : 'bg-slate-100/50 border-slate-200'
              }`}>
                <TabsTrigger value="upload" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
                  <Upload className="size-4" />
                  업로드
                </TabsTrigger>
                <TabsTrigger value="queue" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
                  <Clock className="size-4" />
                  작업 큐
                </TabsTrigger>
                <TabsTrigger value="history" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
                  <History className="size-4" />
                  기록
                </TabsTrigger>
                <TabsTrigger value="search" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
                  <Search className="size-4" />
                  검색
                </TabsTrigger>
                <TabsTrigger value="dictionary" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
                  <Tags className="size-4" />
                  키워드
                </TabsTrigger>
              </TabsList>

              <div className="flex flex-wrap items-center justify-end gap-2">
                <Button
                  type="button"
                  onClick={handleBatchProcess}
                  disabled={queueLoading.batchProcess || queueLoading.cancelPending || queueLoading.pauseToggle}
                  className="bg-gradient-to-r from-violet-600 to-fuchsia-600 text-white hover:from-violet-500 hover:to-fuchsia-500"
                >
                  {queueLoading.batchProcess ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Play className="size-4" />
                  )}
                  {queueLoading.batchProcess ? '전체 시작 중...' : '전체 시작'}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleCancelPending}
                  disabled={
                    !queueStatusLoaded ||
                    pendingQueuedCount === 0 ||
                    queueLoading.batchProcess ||
                    queueLoading.cancelPending ||
                    queueLoading.pauseToggle
                  }
                  className={
                    theme === 'dark'
                      ? 'border-red-500/40 bg-red-500/10 text-red-200 hover:bg-red-500/20 hover:text-red-100'
                      : 'border-red-300 bg-red-50 text-red-700 hover:bg-red-100 hover:text-red-900'
                  }
                >
                  {queueLoading.cancelPending ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : (
                    <Square className="size-4" />
                  )}
                  {queueLoading.cancelPending ? '전체 종료 중...' : '전체 종료'}
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={handleQueuePauseToggle}
                  disabled={
                    !queueStatusLoaded ||
                    queueLoading.batchProcess ||
                    queueLoading.cancelPending ||
                    queueLoading.pauseToggle
                  }
                  className={
                    queueStatus?.paused
                      ? theme === 'dark'
                        ? 'border-emerald-500/40 bg-emerald-500/10 text-emerald-200 hover:bg-emerald-500/20 hover:text-emerald-100'
                        : 'border-emerald-300 bg-emerald-50 text-emerald-700 hover:bg-emerald-100 hover:text-emerald-900'
                      : theme === 'dark'
                        ? 'border-slate-700 bg-slate-900/60 text-slate-200 hover:bg-slate-800 hover:text-white'
                        : 'border-slate-300 bg-white/70 text-slate-700 hover:bg-slate-100 hover:text-slate-900'
                  }
                >
                  {queueLoading.pauseToggle ? (
                    <Loader2 className="size-4 animate-spin" />
                  ) : queueStatus?.paused ? (
                    <Play className="size-4" />
                  ) : (
                    <Pause className="size-4" />
                  )}
                  {queuePauseLabel}
                </Button>
              </div>
            </div>

            {queueMessage ? (
              <div className={`rounded-2xl border px-4 py-3 text-sm ${queueMessageClass(theme, queueMessage.tone)}`}>
                {queueMessage.text}
              </div>
            ) : null}
          </div>

          <TabsContent value="upload" className="space-y-6">
            <UploadSection />
          </TabsContent>

          <TabsContent value="queue" className="space-y-6">
            <JobQueue />
          </TabsContent>

          <TabsContent value="history" className="space-y-6">
            <HistoryPanel />
          </TabsContent>

          <TabsContent value="search" className="space-y-6">
            <SearchPanel />
          </TabsContent>

          <TabsContent value="dictionary" className="space-y-6">
            <DictionaryPanel />
          </TabsContent>
        </Tabs>
      </div>

      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
    </div>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <AppContent />
    </ThemeProvider>
  );
}
