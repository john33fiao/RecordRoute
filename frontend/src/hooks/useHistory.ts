import { useState, useCallback } from 'react';
import * as api from '../api/client';
import type { AsyncState, HistoryRecord } from '../api/types';

const initialState: AsyncState<HistoryRecord[]> = {
  data: [],
  loading: false,
  error: null,
  stale: false,
};

export function useHistory() {
  const [state, setState] = useState<AsyncState<HistoryRecord[]>>(initialState);

  const loadHistory = useCallback(async () => {
    setState(prev => ({ ...prev, loading: true, error: null, stale: prev.data.length > 0 }));
    try {
      const data = await api.getHistory();
      setState({ data, loading: false, error: null, stale: false });
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to load history';
      setState(prev => ({ ...prev, loading: false, error: message, stale: prev.data.length > 0 }));
    }
  }, []);

  const deleteRecords = useCallback(async (ids: string[]) => {
    try {
      await api.deleteRecords(ids);
      await loadHistory();
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to delete records';
      setState(prev => ({ ...prev, error: message, stale: prev.data.length > 0 }));
    }
  }, [loadHistory]);

  const updateFilename = useCallback(async (recordId: string, filename: string) => {
    try {
      await api.updateFilename(recordId, filename);
      setState(prev => ({
        ...prev,
        data: prev.data.map(h => h.id === recordId ? { ...h, filename } : h),
      }));
    } catch (e) {
      const message = e instanceof Error ? e.message : 'Failed to update filename';
      setState(prev => ({ ...prev, error: message, stale: prev.data.length > 0 }));
    }
  }, []);

  return { state, loadHistory, deleteRecords, updateFilename };
}
