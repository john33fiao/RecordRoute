import { useState, useCallback, useEffect } from 'react';
import type { ModelSettings } from '../api/types';

const STORAGE_KEY = 'modelSettings';

const defaultSettings: ModelSettings = {
  whisper: 'large-v3-turbo',
  summarize: '',
  embedding: '',
  language: 'ko',
};

export function useModelSettings() {
  const [settings, setSettings] = useState<ModelSettings>(() => {
    try {
      const stored = localStorage.getItem(STORAGE_KEY);
      if (stored) {
        const parsed = JSON.parse(stored);
        const migrated = parsed?.transcribe && !parsed?.whisper
          ? { ...parsed, whisper: parsed.transcribe }
          : parsed;
        return { ...defaultSettings, ...migrated };
      }
    } catch {}
    return defaultSettings;
  });

  useEffect(() => {
    try {
      localStorage.setItem(STORAGE_KEY, JSON.stringify(settings));
    } catch {}
  }, [settings]);

  const updateSettings = useCallback((updates: Partial<ModelSettings>) => {
    setSettings(prev => ({ ...prev, ...updates }));
  }, []);

  return { settings, updateSettings, setSettings };
}
