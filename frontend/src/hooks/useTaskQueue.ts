import { useState, useCallback, useRef, useEffect } from 'react';
import * as api from '../api/client';
import type { QueueTask, TaskType, ModelSettings } from '../api/types';

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
  const [sortMode, setSortMode] = useState<'category' | 'order'>('category');
  const processingRef = useRef(false);
  const queueRef = useRef(queue);
  queueRef.current = queue;
  const currentTaskRef = useRef(currentTask);
  currentTaskRef.current = currentTask;
  const modelSettingsRef = useRef(modelSettings);
  modelSettingsRef.current = modelSettings;

  const getStepForType = (type: TaskType): string[] => {
    switch (type) {
      case 'stt': return ['stt', 'correct'];
      case 'embedding': return ['embedding'];
      case 'summary': return ['summarize'];
    }
  };

  const addTask = useCallback((recordId: string, filePath: string, taskType: TaskType) => {
    // Check duplicates
    const allTasks = [...queueRef.current, currentTaskRef.current].filter(Boolean) as QueueTask[];
    const exists = allTasks.some(t => t.recordId === recordId && t.taskType === taskType && t.status !== 'error' && t.status !== 'completed');
    if (exists) return;

    const task: QueueTask = {
      id: `task_${Date.now()}_${Math.random().toString(36).slice(2)}`,
      recordId,
      filePath,
      taskType,
      status: 'pending',
      progress: '',
      order: globalOrderCounter++,
      taskId: `task_${Date.now()}_${Math.random().toString(36).slice(2)}`,
      retryCount: 0,
    };

    setQueue(prev => [...prev, task]);
  }, []);

  const removeTask = useCallback((taskId: string) => {
    setQueue(prev => {
      const task = prev.find(t => t.id === taskId);
      if (task?.abortController) task.abortController.abort();
      return prev.filter(t => t.id !== taskId);
    });
  }, []);

  const cancelAll = useCallback(() => {
    queue.forEach(t => t.abortController?.abort());
    currentTask?.abortController?.abort();
    setQueue([]);
    setCurrentTask(null);
    processingRef.current = false;
  }, [queue, currentTask]);

  const updateTaskProgress = useCallback((taskId: string, message: string) => {
    setQueue(prev => prev.map(t => t.taskId === taskId ? { ...t, progress: message } : t));
    setCurrentTask(prev => prev && prev.taskId === taskId ? { ...prev, progress: message } : prev);
  }, []);

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
    const processingTask = { ...task, status: 'processing' as const, abortController };

    setCurrentTask(processingTask);
    setQueue(prev => prev.filter(t => t.id !== task.id));

    try {
      // Check STT dependency for embedding/summary
      if (task.taskType === 'embedding' || task.taskType === 'summary') {
        const sttCheck = await api.checkExistingStt(task.filePath);
        if (!sttCheck.has_stt) {
          // Re-queue with retry
          const retried = { ...task, retryCount: (task.retryCount || 0) + 1, lastRetryTime: Date.now() };
          if (retried.retryCount <= 20) {
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
      await api.processTask(
        task.filePath,
        steps,
        task.recordId,
        processingTask.taskId,
        {
          transcribe: ms.transcribe,
          summarize: ms.summarize,
          language: ms.language,
        },
        abortController.signal
      );

      setCurrentTask(prev => prev ? { ...prev, status: 'completed' } : null);
      onTaskComplete?.();
    } catch (e: any) {
      if (e.name === 'AbortError') {
        setCurrentTask(prev => prev ? { ...prev, status: 'error', progress: '취소됨' } : null);
      } else {
        setCurrentTask(prev => prev ? { ...prev, status: 'error', progress: e.message || '오류 발생' } : null);
      }
    } finally {
      processingRef.current = false;
      // Auto-process next after a delay
      setTimeout(() => {
        setCurrentTask(null);
        processNext();
      }, 1000);
    }
  }, [sortMode, onTaskComplete]);

  // Auto-process when queue changes
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
    cancelAll,
    updateTaskProgress,
    categoryLabels: CATEGORY_LABELS,
  };
}
