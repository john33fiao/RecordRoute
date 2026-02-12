import { useState, useCallback } from 'react';
import * as api from '../api/client';
import type { HistoryRecord } from '../api/types';

export function useHistory() {
  const [history, setHistory] = useState<HistoryRecord[]>([]);
  const [loading, setLoading] = useState(false);

  const loadHistory = useCallback(async () => {
    try {
      setLoading(true);
      const data = await api.getHistory();
      setHistory(data);
    } catch (e) {
      console.error('Failed to load history:', e);
    } finally {
      setLoading(false);
    }
  }, []);

  const deleteRecords = useCallback(async (ids: string[]) => {
    try {
      await api.deleteRecords(ids);
      await loadHistory();
    } catch (e) {
      console.error('Failed to delete records:', e);
    }
  }, [loadHistory]);

  const updateFilename = useCallback(async (recordId: string, filename: string) => {
    try {
      await api.updateFilename(recordId, filename);
      setHistory(prev => prev.map(h => h.id === recordId ? { ...h, filename } : h));
    } catch (e) {
      console.error('Failed to update filename:', e);
    }
  }, []);

  return { history, loading, loadHistory, deleteRecords, updateFilename };
}
