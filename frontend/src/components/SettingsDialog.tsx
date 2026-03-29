import { useEffect, useRef, useState } from 'react';
import { LoaderCircle, RefreshCw, TriangleAlert } from 'lucide-react';
import { Dialog, DialogContent, DialogHeader, DialogTitle } from './ui/dialog';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { cn } from './ui/utils';

type PreparationStatus = 'idle' | 'running' | 'completed' | 'failed';
type StatusTone = 'ready' | 'running' | 'failed' | 'idle';

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

interface SystemStatusResponse {
  ffmpeg_available: boolean;
  whisper_available: boolean;
  llama_available: boolean;
  whisper_model_ready: boolean;
  llama_model_ready: boolean;
  llama_embedding_model_ready: boolean;
  errors: string[];
}

interface ModelPreparationRecord {
  status: PreparationStatus;
  started_at?: string | null;
  finished_at?: string | null;
  heartbeat_at?: string | null;
  last_error?: string | null;
}

interface ModelStatusEntryResponse {
  available: boolean;
  ready: boolean;
  error: string | null;
  embedding_available: boolean;
  embedding_ready: boolean;
  embedding_error: string | null;
  preparation: ModelPreparationRecord;
}

interface ModelStatusResponse {
  whisper: ModelStatusEntryResponse;
  llama: ModelStatusEntryResponse;
}

interface ModelPrepareResponse {
  status: 'ok' | 'accepted';
  message: string;
  ready: boolean;
  already_ready: boolean;
  deduplicated: boolean;
  preparation: ModelPreparationRecord;
}

interface StatusChip {
  label: string;
  tone: StatusTone;
}

interface StatusSnapshot {
  system: SystemStatusResponse;
  models: ModelStatusResponse;
}

interface PreparingState {
  whisper: boolean;
  llama: boolean;
}

const POLL_INTERVAL_MS = 1200;

const cardBaseClass =
  'rounded-xl border border-slate-800 bg-slate-900/50 backdrop-blur-sm';

const buttonBaseClass = 'h-10 rounded-lg px-4 text-sm font-medium transition-all';

const primaryButtonClass =
  'bg-gradient-to-r from-violet-600 to-fuchsia-600 text-white hover:from-violet-500 hover:to-fuchsia-500 disabled:from-slate-800 disabled:to-slate-800 disabled:text-slate-400';

const secondaryButtonClass =
  'border border-slate-700 bg-slate-800 text-slate-300 hover:bg-slate-700 disabled:bg-slate-800/70 disabled:text-slate-500';

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

function tryParseJson(text: string) {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

function statusChipClass(tone: StatusTone) {
  switch (tone) {
    case 'ready':
      return 'border-green-500/30 bg-green-500/15 text-green-300';
    case 'running':
      return 'border-violet-500/30 bg-violet-500/15 text-violet-300';
    case 'failed':
      return 'border-red-500/30 bg-red-500/15 text-red-300';
    case 'idle':
    default:
      return 'border-slate-700 bg-slate-800/70 text-slate-300';
  }
}

function buildRuntimeChip(available: boolean, isLoading: boolean): StatusChip {
  if (isLoading) {
    return { label: 'loading', tone: 'idle' };
  }

  return available
    ? { label: 'ready', tone: 'ready' }
    : { label: 'not ready', tone: 'idle' };
}

function buildPreparationChip(
  preparation: ModelPreparationRecord | undefined,
  error: string | null | undefined,
  forceRunning: boolean,
  isLoading: boolean,
): StatusChip {
  if (isLoading) {
    return { label: 'loading', tone: 'idle' };
  }

  if (error || preparation?.status === 'failed') {
    return { label: 'failed', tone: 'failed' };
  }

  if (forceRunning || preparation?.status === 'running') {
    return { label: 'running', tone: 'running' };
  }

  if (preparation?.status === 'completed') {
    return { label: 'completed', tone: 'ready' };
  }

  return { label: 'not ready', tone: 'idle' };
}

function buildReadinessChip(
  ready: boolean,
  error: string | null | undefined,
  isLoading: boolean,
  readyLabel: string,
  forceRunning = false,
): StatusChip {
  if (isLoading) {
    return { label: 'loading', tone: 'idle' };
  }

  if (error) {
    return { label: 'failed', tone: 'failed' };
  }

  if (forceRunning && !ready) {
    return { label: 'running', tone: 'running' };
  }

  return ready
    ? { label: readyLabel, tone: 'ready' }
    : { label: 'not ready', tone: 'idle' };
}

function buildPrimaryModelChip(
  ready: boolean,
  error: string | null | undefined,
  preparation: ModelPreparationRecord | undefined,
  forceRunning: boolean,
  isLoading: boolean,
  readyLabel = 'ready',
): StatusChip {
  if (isLoading) {
    return { label: 'loading', tone: 'idle' };
  }

  if (error || preparation?.status === 'failed') {
    return { label: 'failed', tone: 'failed' };
  }

  if (ready) {
    return { label: readyLabel, tone: 'ready' };
  }

  if (forceRunning || preparation?.status === 'running') {
    return { label: 'running', tone: 'running' };
  }

  if (preparation?.status === 'completed') {
    return { label: 'completed', tone: 'ready' };
  }

  return { label: 'not ready', tone: 'idle' };
}

function isAbortError(error: unknown) {
  return error instanceof DOMException && error.name === 'AbortError';
}

function isWhisperResolved(snapshot: StatusSnapshot) {
  return (
    snapshot.system.whisper_model_ready ||
    Boolean(snapshot.models.whisper.error) ||
    snapshot.models.whisper.preparation.status === 'failed'
  );
}

function isLlamaResolved(snapshot: StatusSnapshot) {
  return (
    (snapshot.system.llama_model_ready && snapshot.system.llama_embedding_model_ready) ||
    Boolean(snapshot.models.llama.error) ||
    Boolean(snapshot.models.llama.embedding_error) ||
    snapshot.models.llama.preparation.status === 'failed'
  );
}

function getActionLabel({
  isPreparing,
  isReady,
  isAvailable,
  label,
}: {
  isPreparing: boolean;
  isReady: boolean;
  isAvailable: boolean;
  label: string;
}) {
  if (isPreparing) {
    return '준비 중...';
  }

  if (isReady) {
    return `${label} 준비 완료`;
  }

  if (!isAvailable) {
    return `${label} 사용 불가`;
  }

  return `${label} 준비`;
}

function dedupeChips(chips: StatusChip[]) {
  const seen = new Set<string>();

  return chips.filter((chip) => {
    const key = `${chip.label}:${chip.tone}`;
    if (seen.has(key)) {
      return false;
    }

    seen.add(key);
    return true;
  });
}

function resolveServerAddress() {
  if (typeof window !== 'undefined' && window.location.host) {
    return window.location.host;
  }

  return '127.0.0.1:38080';
}

function resolveServerCaption({
  isLoading,
  systemStatus,
  loadError,
}: {
  isLoading: boolean;
  systemStatus: SystemStatusResponse | null;
  loadError: string | null;
}) {
  if (loadError) {
    return '서버 상태를 확인할 수 없습니다.';
  }

  if (isLoading && !systemStatus) {
    return '서버 상태를 확인하는 중입니다.';
  }

  if ((systemStatus?.errors?.length ?? 0) > 0) {
    return '환경 준비가 필요합니다. setup을 다시 실행하세요.';
  }

  return '모든 런타임과 모델이 준비되었습니다.';
}

function SummaryCard({
  title,
  chip,
  isLoading,
}: {
  title: string;
  chip: StatusChip;
  isLoading: boolean;
}) {
  return (
    <div
      className={cn(
        cardBaseClass,
        'flex items-center justify-between gap-4 p-4 transition-colors',
        isLoading && 'animate-pulse',
      )}
    >
      <p className="text-sm font-medium text-slate-300">{title}</p>
      <Badge className={cn('rounded-full border px-3 py-1.5 font-mono text-sm', statusChipClass(chip.tone))}>
        {chip.label}
      </Badge>
    </div>
  );
}

function DetailCard({
  title,
  description,
  chips,
  actionLabel,
  actionDisabled,
  onAction,
  errorMessage,
}: {
  title: string;
  description: string;
  chips: StatusChip[];
  actionLabel: string;
  actionDisabled: boolean;
  onAction: () => void;
  errorMessage?: string | null;
}) {
  return (
    <div className={cn(cardBaseClass, 'p-6 space-y-6')}>
      <div className="flex flex-col gap-4 sm:flex-row sm:items-start sm:justify-between">
        <div className="space-y-4 min-w-0">
          <div className="space-y-2">
            <h3 className="text-xl font-semibold text-white">{title}</h3>
            <p className="max-w-xl text-sm text-slate-400">{description}</p>
          </div>

          <div className="flex flex-wrap gap-2">
            {chips.map((chip) => (
              <Badge
                key={`${title}-${chip.label}`}
                className={cn('rounded-full border px-3 py-1.5 font-mono text-sm', statusChipClass(chip.tone))}
              >
                {chip.label}
              </Badge>
            ))}
          </div>
        </div>

        <Button
          onClick={onAction}
          disabled={actionDisabled}
          className={cn(buttonBaseClass, primaryButtonClass, 'w-full sm:w-auto sm:min-w-[160px]')}
        >
          {actionLabel}
        </Button>
      </div>

      {errorMessage ? (
        <div className="rounded-lg border border-red-500/20 bg-red-500/8 px-4 py-3 text-sm text-red-200">
          {errorMessage}
        </div>
      ) : null}
    </div>
  );
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const [systemStatus, setSystemStatus] = useState<SystemStatusResponse | null>(null);
  const [modelStatus, setModelStatus] = useState<ModelStatusResponse | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [loadError, setLoadError] = useState<string | null>(null);
  const [preparing, setPreparing] = useState<PreparingState>({ whisper: false, llama: false });

  const pollTimerRef = useRef<number | null>(null);
  const requestControllerRef = useRef<AbortController | null>(null);
  const preparingRef = useRef<PreparingState>({ whisper: false, llama: false });

  const clearScheduledPoll = () => {
    if (pollTimerRef.current !== null) {
      window.clearTimeout(pollTimerRef.current);
      pollTimerRef.current = null;
    }
  };

  const abortActiveRequest = () => {
    requestControllerRef.current?.abort();
    requestControllerRef.current = null;
  };

  const syncPreparingState = (next: PreparingState) => {
    preparingRef.current = next;
    setPreparing(next);
  };

  const shouldContinuePolling = (snapshot: StatusSnapshot, nextPreparing: PreparingState) => {
    return (
      snapshot.models.whisper.preparation.status === 'running' ||
      snapshot.models.llama.preparation.status === 'running' ||
      nextPreparing.whisper ||
      nextPreparing.llama
    );
  };

  const loadStatuses = async ({ silent = false }: { silent?: boolean } = {}) => {
    if (!open) {
      return;
    }

    clearScheduledPoll();
    abortActiveRequest();

    const controller = new AbortController();
    requestControllerRef.current = controller;

    if (!silent) {
      setIsLoading(true);
    }

    setLoadError(null);

    try {
      const [system, models] = await Promise.all([
        fetchJson<SystemStatusResponse>('/system/status', { signal: controller.signal }),
        fetchJson<ModelStatusResponse>('/models/status', { signal: controller.signal }),
      ]);

      if (requestControllerRef.current !== controller) {
        return;
      }

      const snapshot = { system, models };
      const nextPreparing: PreparingState = {
        whisper: preparingRef.current.whisper && !isWhisperResolved(snapshot),
        llama: preparingRef.current.llama && !isLlamaResolved(snapshot),
      };

      setSystemStatus(system);
      setModelStatus(models);
      syncPreparingState(nextPreparing);

      if (shouldContinuePolling(snapshot, nextPreparing)) {
        pollTimerRef.current = window.setTimeout(() => {
          void loadStatuses({ silent: true });
        }, POLL_INTERVAL_MS);
      }
    } catch (error) {
      if (!isAbortError(error)) {
        setLoadError(error instanceof Error ? error.message : '상태를 불러오지 못했습니다.');
      }
    } finally {
      if (requestControllerRef.current === controller) {
        requestControllerRef.current = null;
      }

      if (!silent) {
        setIsLoading(false);
      }
    }
  };

  useEffect(() => {
    if (open) {
      void loadStatuses();
    } else {
      clearScheduledPoll();
      abortActiveRequest();
    }

    return () => {
      clearScheduledPoll();
      abortActiveRequest();
    };
  }, [open]);

  const handlePrepare = async (model: 'whisper' | 'llama') => {
    const nextPreparing = {
      ...preparingRef.current,
      [model]: true,
    };
    syncPreparingState(nextPreparing);
    setLoadError(null);

    try {
      const response = await fetchJson<ModelPrepareResponse>(
        model === 'whisper' ? '/models/whisper/prepare' : '/models/llama/prepare',
        { method: 'POST' },
      );

      if (response.ready) {
        syncPreparingState({
          ...preparingRef.current,
          [model]: false,
        });
      }

      await loadStatuses({ silent: true });
    } catch (error) {
      syncPreparingState({
        ...preparingRef.current,
        [model]: false,
      });
      setLoadError(error instanceof Error ? error.message : '모델 준비 요청에 실패했습니다.');
    }
  };

  const whisperRuntimeChip = buildRuntimeChip(systemStatus?.whisper_available ?? false, isLoading && !systemStatus);
  const llamaRuntimeChip = buildRuntimeChip(systemStatus?.llama_available ?? false, isLoading && !systemStatus);
  const ffmpegRuntimeChip = buildRuntimeChip(systemStatus?.ffmpeg_available ?? false, isLoading && !systemStatus);

  const whisperModelChip = buildPrimaryModelChip(
    systemStatus?.whisper_model_ready ?? false,
    modelStatus?.whisper.error,
    modelStatus?.whisper.preparation,
    preparing.whisper && !(systemStatus?.whisper_model_ready ?? false),
    isLoading && !modelStatus,
  );

  const llamaModelChip = buildPrimaryModelChip(
    systemStatus?.llama_model_ready ?? false,
    modelStatus?.llama.error,
    modelStatus?.llama.preparation,
    preparing.llama && !(systemStatus?.llama_model_ready ?? false),
    isLoading && !modelStatus,
  );

  const embeddingModelChip = buildReadinessChip(
    systemStatus?.llama_embedding_model_ready ?? false,
    modelStatus?.llama.embedding_error,
    isLoading && !modelStatus,
    'embedding ready',
    preparing.llama && !(systemStatus?.llama_embedding_model_ready ?? false),
  );

  const whisperDetailChips = [
    buildPreparationChip(
      modelStatus?.whisper.preparation,
      modelStatus?.whisper.error,
      preparing.whisper && !(systemStatus?.whisper_model_ready ?? false),
      isLoading && !modelStatus,
    ),
    buildReadinessChip(
      systemStatus?.whisper_model_ready ?? false,
      modelStatus?.whisper.error,
      isLoading && !modelStatus,
      'model ready',
      preparing.whisper && !(systemStatus?.whisper_model_ready ?? false),
    ),
  ];

  const llamaDetailChips = [
    buildPreparationChip(
      modelStatus?.llama.preparation,
      modelStatus?.llama.error,
      preparing.llama && !(systemStatus?.llama_model_ready ?? false),
      isLoading && !modelStatus,
    ),
    buildReadinessChip(
      systemStatus?.llama_model_ready ?? false,
      modelStatus?.llama.error,
      isLoading && !modelStatus,
      'model ready',
      preparing.llama && !(systemStatus?.llama_model_ready ?? false),
    ),
    buildReadinessChip(
      systemStatus?.llama_embedding_model_ready ?? false,
      modelStatus?.llama.embedding_error,
      isLoading && !modelStatus,
      'embedding ready',
      preparing.llama && !(systemStatus?.llama_embedding_model_ready ?? false),
    ),
  ];

  const whisperDetailChipList = dedupeChips(whisperDetailChips);
  const llamaDetailChipList = dedupeChips(llamaDetailChips);

  const systemErrors = systemStatus?.errors ?? [];
  const serverAddress = resolveServerAddress();
  const serverCaption = resolveServerCaption({
    isLoading,
    systemStatus,
    loadError,
  });
  const whisperActionDisabled =
    isLoading ||
    preparing.whisper ||
    !systemStatus?.whisper_available ||
    (systemStatus?.whisper_model_ready ?? false);
  const llamaActionDisabled =
    isLoading ||
    preparing.llama ||
    !systemStatus?.llama_available ||
    ((systemStatus?.llama_model_ready ?? false) && (systemStatus?.llama_embedding_model_ready ?? false));

  const whisperActionLabel = getActionLabel({
    isPreparing: preparing.whisper,
    isReady: systemStatus?.whisper_model_ready ?? false,
    isAvailable: systemStatus?.whisper_available ?? false,
    label: 'Whisper',
  });

  const llamaActionLabel = getActionLabel({
    isPreparing: preparing.llama,
    isReady: (systemStatus?.llama_model_ready ?? false) && (systemStatus?.llama_embedding_model_ready ?? false),
    isAvailable: systemStatus?.llama_available ?? false,
    label: 'Llama',
  });

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="max-h-[88vh] max-w-[min(1120px,calc(100%-2rem))] overflow-hidden rounded-3xl border-0 bg-slate-950 p-0 text-slate-100 shadow-[0_28px_100px_rgba(2,6,23,0.68)]">
        <div className="max-h-[88vh] overflow-y-auto p-6 space-y-6 sm:p-8">
          <DialogHeader className="gap-6 pr-14 text-left">
            <div className="space-y-4">
              <div className="space-y-3">
                <p className="text-sm font-medium tracking-[0.34em] text-violet-300/90">
                  SYSTEM / MODELS
                </p>
                <DialogTitle asChild>
                  <h2 className="text-xl font-semibold text-white">런타임과 모델 준비 상태</h2>
                </DialogTitle>
              </div>

              <Button
                onClick={() => void loadStatuses()}
                disabled={isLoading}
                className={cn(buttonBaseClass, secondaryButtonClass, 'w-full sm:w-auto')}
              >
                {isLoading ? <LoaderCircle className="size-5 animate-spin" /> : <RefreshCw className="size-5" />}
                새로고침
              </Button>
            </div>

            <div className="rounded-2xl border border-violet-500/20 bg-gradient-to-br from-violet-500/12 via-slate-900/88 to-slate-950 px-5 py-5 shadow-[inset_0_1px_0_rgba(255,255,255,0.05)]">
              <p className="text-xs font-medium uppercase tracking-[0.34em] text-violet-300/90">
                Server
              </p>
              <p className="mt-3 break-all font-mono text-2xl font-semibold text-white sm:text-[2rem]">
                {serverAddress}
              </p>
              <p className="mt-3 max-w-2xl text-sm leading-6 text-slate-300">
                {serverCaption}
              </p>
            </div>

            {loadError ? (
              <div className="rounded-xl border border-red-500/20 bg-red-500/8 px-5 py-4 text-sm text-red-100">
                <div className="flex items-start gap-3">
                  <TriangleAlert className="mt-0.5 size-4 shrink-0" />
                  <p>{loadError}</p>
                </div>
              </div>
            ) : null}

            {systemErrors.length > 0 ? (
              <div className="rounded-xl border border-violet-500/20 bg-violet-500/8 px-5 py-4">
                <div className="flex items-start gap-3">
                  <TriangleAlert className="mt-0.5 size-4 shrink-0 text-violet-200" />
                  <div className="space-y-1 text-sm text-violet-50/90">
                    {systemErrors.map((message) => (
                      <p key={message}>{message}</p>
                    ))}
                  </div>
                </div>
              </div>
            ) : null}
          </DialogHeader>

          <div className="space-y-4">
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-xl font-semibold text-white">모듈 준비 상태</h2>
              <Badge variant="outline" className="border-violet-500 text-violet-400">
                6개 모듈
              </Badge>
            </div>

            <div className="space-y-3">
              <SummaryCard title="FFmpeg" chip={ffmpegRuntimeChip} isLoading={isLoading && !systemStatus} />
              <SummaryCard title="Whisper" chip={whisperRuntimeChip} isLoading={isLoading && !systemStatus} />
              <SummaryCard title="Llama" chip={llamaRuntimeChip} isLoading={isLoading && !systemStatus} />
              <SummaryCard title="Whisper Model" chip={whisperModelChip} isLoading={isLoading && !modelStatus} />
              <SummaryCard title="Llama Model" chip={llamaModelChip} isLoading={isLoading && !modelStatus} />
              <SummaryCard title="Embedding Model" chip={embeddingModelChip} isLoading={isLoading && !modelStatus} />
            </div>
          </div>

          <div className="space-y-4">
            <div className="flex items-center justify-between gap-3">
              <h2 className="text-xl font-semibold text-white">모델 준비</h2>
              <Badge variant="outline" className="border-violet-500 text-violet-400">
                2개 액션
              </Badge>
            </div>

            <div className="space-y-5">
            <DetailCard
              title="Whisper"
              description="STT 전사용 모델과 준비 상태"
              chips={whisperDetailChipList}
              actionLabel={whisperActionLabel}
              actionDisabled={whisperActionDisabled}
              onAction={() => void handlePrepare('whisper')}
              errorMessage={modelStatus?.whisper.error}
            />

            <DetailCard
              title="Llama"
              description="요약 및 embedding 준비 상태"
              chips={llamaDetailChipList}
              actionLabel={llamaActionLabel}
              actionDisabled={llamaActionDisabled}
              onAction={() => void handlePrepare('llama')}
              errorMessage={modelStatus?.llama.error ?? modelStatus?.llama.embedding_error}
            />
            </div>
          </div>
        </div>
      </DialogContent>
    </Dialog>
  );
}
