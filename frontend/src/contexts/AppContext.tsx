import { createContext, useContext, useEffect, useCallback, type ReactNode } from 'react';
import { useTaskQueue } from '../hooks/useTaskQueue';
import { useHistory } from '../hooks/useHistory';
import { useModelSettings } from '../hooks/useModelSettings';
import { useWebSocket } from '../hooks/useWebSocket';
import type { QueueTask, TaskType, ModelSettings, HistoryRecord } from '../api/types';

interface AppContextValue {
  // Task Queue
  queue: QueueTask[];
  currentTask: QueueTask | null;
  sortMode: 'category' | 'order';
  setSortMode: (mode: 'category' | 'order') => void;
  addTask: (recordId: string, filePath: string, taskType: TaskType) => void;
  removeTask: (taskId: string) => void;
  cancelAll: () => void;
  categoryLabels: Record<TaskType, string>;

  // History
  history: HistoryRecord[];
  historyLoading: boolean;
  loadHistory: () => Promise<void>;
  deleteRecords: (ids: string[]) => Promise<void>;
  updateFilename: (recordId: string, filename: string) => Promise<void>;

  // Model Settings
  modelSettings: ModelSettings;
  updateModelSettings: (updates: Partial<ModelSettings>) => void;
  setModelSettings: (settings: ModelSettings) => void;
}

const AppContext = createContext<AppContextValue | null>(null);

export function AppProvider({ children }: { children: ReactNode }) {
  const { settings: modelSettings, updateSettings: updateModelSettings, setSettings: setModelSettings } = useModelSettings();
  const { history, loading: historyLoading, loadHistory, deleteRecords, updateFilename } = useHistory();

  const handleTaskComplete = useCallback(() => {
    loadHistory();
  }, [loadHistory]);

  const {
    queue, currentTask, sortMode, setSortMode,
    addTask, removeTask, cancelAll, updateTaskProgress, categoryLabels,
  } = useTaskQueue(modelSettings, handleTaskComplete);

  // WebSocket for progress updates
  useWebSocket(updateTaskProgress);

  // Load history on mount
  useEffect(() => {
    loadHistory();
  }, [loadHistory]);

  return (
    <AppContext.Provider value={{
      queue, currentTask, sortMode, setSortMode, addTask, removeTask, cancelAll, categoryLabels,
      history, historyLoading, loadHistory, deleteRecords, updateFilename,
      modelSettings, updateModelSettings, setModelSettings,
    }}>
      {children}
    </AppContext.Provider>
  );
}

export function useApp() {
  const ctx = useContext(AppContext);
  if (!ctx) throw new Error('useApp must be used within AppProvider');
  return ctx;
}
