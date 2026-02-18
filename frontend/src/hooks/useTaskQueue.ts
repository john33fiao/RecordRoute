import { useState, useCallback, useRef, useEffect } from 'react';
import * as api from '../api/client';
import type { QueueTask, TaskType, ModelSettings, QueueSortMode, TaskProgress, TaskStage } from '../api/types';
import { QueueTaskStatus } from '../api/types';

const CATEGORY_ORDER: TaskType[] = ['stt', 'embedding', 'summary'];
const CATEGORY_LABELS: Record<TaskType, string> = {
  stt: 'STT 변환',
  embedding: '색인 생성',
  summary: '요약',
};

let globalOrderCounter = 0;

export function useTaskQueue(modelSettings: ModelSettings, onTaskComplete?: () => void) {
  const [queue, setQueue] = useState<QueueTask[]>([]);
  const [currentTask, setCurrentTask] = useState<QueueTask | null>(null);
  const [sortMode, setSortMode] = useState<QueueSortMode>('category');
  const processingRef = useRef(false);
  const queueRef = useRef(queue);
  queueRef.current = queue;
  const currentTaskRef = useRef(currentTask);
  currentTaskRef.current = currentTask;
  const modelSettingsRef = useRef(modelSettings);
  modelSettingsRef.current = modelSettings;
  const progressDedupRef = useRef<Map<string, string>>(new Map());

  const getStepForType = (type: TaskType): string[] => {
    switch (type) {
      case 'stt': return ['stt', 'correct'];
      case 'embedding': return ['embedding'];
      case 'summary': return ['summarize'];
    }
  };

  const addTask = useCallback((recordId: string, filePath: string, taskType: TaskType) => {
    const allTasks = [...queueRef.current, currentTaskRef.current].filter(Boolean) as QueueTask[];
    const exists = allTasks.some(t => t.recordId === recordId && t.taskType === taskType && t.status !== QueueTaskStatus.Error && t.status !== QueueTaskStatus.Completed && t.status !== QueueTaskStatus.Cancelled);
    if (exists) return;

    const task: QueueTask = {
      id: `task_${Date.now()}_${Math.random().toString(36).slice(2)}`,
      recordId,
      filePath,
      taskType,
      status: QueueTaskStatus.Pending,
      progress: '',
      order: globalOrderCounter++,
      taskId: `task_${Date.now()}_${Math.random().toString(36).slice(2)}`,
      retryCount: 0,
    };

    setQueue(prev => [...prev, task]);
  }, []);

  const applyProgressUpdate = useCallback((taskId: string, message: string, meta?: { stage?: TaskStage; error_code?: string; retryable?: boolean; failed_step?: string; progress_percent?: number; eta_seconds?: number | null; error?: { message: string; code?: string | null; retryable?: boolean | null; failed_step?: string | null } | null }) => {
    const previousMessage = progressDedupRef.current.get(taskId);
    if (previousMessage === message) return;
    progressDedupRef.current.set(taskId, message);

    setQueue(prev => prev.map(t => t.taskId === taskId ? { ...t, progress: message, stage: meta?.stage, errorCode: meta?.error_code, retryable: meta?.retryable, failedStep: meta?.failed_step, progressPercent: meta?.progress_percent, etaSeconds: meta?.eta_seconds, error: meta?.error } : t));
    setCurrentTask(prev => prev && prev.taskId === taskId ? { ...prev, progress: message, stage: meta?.stage, errorCode: meta?.error_code, retryable: meta?.retryable, failedStep: meta?.failed_step, progressPercent: meta?.progress_percent, etaSeconds: meta?.eta_seconds, error: meta?.error } : prev);
  }, []);

  const removeTask = useCallback((taskId: string) => {
    setQueue(prev => {
      const task = prev.find(t => t.id === taskId);
      task?.abortController?.abort();
      return prev.filter(t => t.id !== taskId);
    });

    setCurrentTask(prev => {
      if (prev?.id !== taskId) return prev;
      prev.abortController?.abort();
      return { ...prev, status: QueueTaskStatus.Cancelled, progress: '취소됨' };
    });
  }, []);

  const retryTask = useCallback((task: QueueTask) => {
    const retried: QueueTask = {
      ...task,
      id: `task_${Date.now()}_${Math.random().toString(36).slice(2)}`,
      taskId: `task_${Date.now()}_${Math.random().toString(36).slice(2)}`,
      status: QueueTaskStatus.Pending,
      progress: '',
      retryCount: (task.retryCount || 0) + 1,
      retryOfTaskId: task.retryOfTaskId || task.taskId,
      abortController: undefined,
      lastRetryTime: Date.now(),
    };

    setCurrentTask(prev => prev?.id === task.id ? null : prev);
    setQueue(prev => [...prev.filter(q => q.id !== task.id), retried]);
  }, []);

  const cancelAll = useCallback(() => {
    queue.forEach(t => t.abortController?.abort());
    currentTask?.abortController?.abort();
    setQueue([]);
    setCurrentTask(null);
    processingRef.current = false;
  }, [queue, currentTask]);

  const processNext = useCallback(async () => {
    if (processingRef.current) return;
    const nextQueue = queueRef.current;
    if (nextQueue.length === 0) return;

    const sorted = [...nextQueue].sort((a, b) => {
      if (sortMode === 'category') {
        const catA = CATEGORY_ORDER.indexOf(a.taskType);
        const catB = CATEGORY_ORDER.indexOf(b.taskType);
        if (catA !== catB) return catA - catB;
      }
      return a.order - b.order;
    });

    const task = sorted[0];
    if (!task) return;

    processingRef.current = true;
    const abortController = new AbortController();
    const processingTask = { ...task, status: QueueTaskStatus.Processing as const, abortController };

    setCurrentTask(processingTask);
    setQueue(prev => prev.filter(t => t.id !== task.id));

    try {
      if (task.taskType === 'embedding' || task.taskType === 'summary') {
        const sttCheck = await api.checkExistingStt(task.filePath);
        if (!sttCheck.has_stt) {
          const retried = { ...task, retryCount: (task.retryCount || 0) + 1, lastRetryTime: Date.now() };
          if ((retried.retryCount || 0) <= 20) {
            setQueue(prev => [...prev, retried]);
          }
          setCurrentTask(null);
          processingRef.current = false;
          setTimeout(() => processNext(), 2000);
          return;
        }
      }

      const steps = getStepForType(task.taskType);
      const ms = modelSettingsRef.current;
      const result = await api.processTask(
        task.filePath,
        steps,
        task.recordId,
        processingTask.taskId,
        {
          whisper: ms.whisper,
          summarize: ms.summarize,
          embedding: ms.embedding,
          language: ms.language,
          provider: ms.provider,
          llm_provider: ms.llm_provider,
        },
        abortController.signal,
        'new_task',
        task.retryOfTaskId,
      );

      if (result.error) {
        setCurrentTask(prev => prev ? { ...prev, status: QueueTaskStatus.Error, progress: result.error || "오류 발생", stage: result.failed_step === 'correct' ? 'correct' : result.failed_step === 'summary' ? 'summary' : 'transform', errorCode: result.error_code, retryable: result.retryable, failedStep: result.failed_step, error: { message: result.error || '오류 발생', code: result.error_code, retryable: result.retryable, failed_step: result.failed_step } } : null);
      } else {
        setCurrentTask(prev => prev ? { ...prev, status: QueueTaskStatus.Completed } : null);
      }
      if (!result.error) onTaskComplete?.();
    } catch (e: any) {
      if (e.name === 'AbortError') {
        setCurrentTask(prev => prev ? { ...prev, status: QueueTaskStatus.Cancelled, progress: '취소됨' } : null);
      } else {
        setCurrentTask(prev => prev ? { ...prev, status: QueueTaskStatus.Error, progress: e.message || '오류 발생' } : null);
      }
    } finally {
      processingRef.current = false;
      setTimeout(() => {
        setCurrentTask(prev => (prev?.status === QueueTaskStatus.Processing ? null : prev));
        processNext();
      }, 700);
    }
  }, [sortMode, onTaskComplete]);

  useEffect(() => {
    if (!currentTask?.taskId || currentTask.status !== QueueTaskStatus.Processing) return;

    let cancelled = false;
    const poll = async () => {
      try {
        const progress: TaskProgress = await api.getProgress(currentTask.taskId);
        if (cancelled || !progress.message) return;
        applyProgressUpdate(progress.task_id, progress.message, { stage: progress.stage, error_code: progress.error_code, retryable: progress.retryable, failed_step: progress.failed_step, progress_percent: progress.progress_percent, eta_seconds: progress.eta_seconds, error: progress.error });
      } catch {
        // WebSocket is primary; polling explicitly prevents stale UI when websocket events are dropped.
      }
    };

    const timer = setInterval(poll, 1500);
    poll();

    return () => {
      cancelled = true;
      clearInterval(timer);
    };
  }, [currentTask?.taskId, currentTask?.status, applyProgressUpdate]);

  useEffect(() => {
    if (!processingRef.current && queue.length > 0 && !currentTask) {
      processNext();
    }
  }, [queue, currentTask, processNext]);

  const sortedQueue = [...queue].sort((a, b) => {
    if (sortMode === 'category') {
      const catA = CATEGORY_ORDER.indexOf(a.taskType);
      const catB = CATEGORY_ORDER.indexOf(b.taskType);
      if (catA !== catB) return catA - catB;
    }
    return a.order - b.order;
  });

  return {
    queue: sortedQueue,
    currentTask,
    sortMode,
    setSortMode,
    addTask,
    removeTask,
    retryTask,
    cancelAll,
    updateTaskProgress: applyProgressUpdate,
    categoryLabels: CATEGORY_LABELS,
  };
}
