import { useState, useEffect } from 'react';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { Label } from './ui/label';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from './ui/select';
import { Switch } from './ui/switch';
import { Power } from 'lucide-react';
import { useTheme } from '../contexts/ThemeContext';
import { useApp } from '../contexts/AppContext';
import * as api from '../api/client';

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { theme, setTheme } = useTheme();
  const { modelSettings, setModelSettings } = useApp();
  const [localSettings, setLocalSettings] = useState(modelSettings);
  const [darkMode, setDarkMode] = useState(theme === 'dark');
  const [availableModels, setAvailableModels] = useState<string[]>([]);
  const [summaryModels, setSummaryModels] = useState<string[]>([]);
  const [embeddingModels, setEmbeddingModels] = useState<string[]>([]);
  const [defaults, setDefaults] = useState<{ whisper: string; summarize: string; embedding: string; provider?: 'ollama' | 'llamacpp' } | null>(null);

  const selectedProvider = (localSettings?.provider || localSettings?.llm_provider || defaults?.provider || 'ollama') as 'ollama' | 'llamacpp';
  const requestedProvider = (localSettings?.provider || localSettings?.llm_provider) as 'ollama' | 'llamacpp' | undefined;

  useEffect(() => {
    if (open) {
      setLocalSettings(modelSettings);
      setDarkMode(theme === 'dark');
      api.getModels(requestedProvider).then(data => {
        const models = data.models || [];
        const modelsByTask = data.models_by_task || {};
        const dedup = (items: string[]) => Array.from(new Set((items || []).filter(Boolean)));
        const fetchedDefaults = data.default;
        setAvailableModels(models);
        setSummaryModels(dedup(modelsByTask.summary || models));
        setEmbeddingModels(dedup(modelsByTask.embedding || models));
        setDefaults(fetchedDefaults);
        setLocalSettings(prev => {
          if (!prev) {
            return prev;
          }
          const summaryDefault = dedup(modelsByTask.summary || models)[0] || models[0] || '';
          const embeddingDefault = dedup(modelsByTask.embedding || models)[0] || models[0] || '';
          return {
            ...prev,
            provider: prev.provider || prev.llm_provider || fetchedDefaults?.provider,
            llm_provider: prev.llm_provider || prev.provider || fetchedDefaults?.provider,
            summarize: prev.summarize || fetchedDefaults?.summarize || summaryDefault,
            embedding: prev.embedding || fetchedDefaults?.embedding || embeddingDefault,
          };
        });
      }).catch(() => { });
    }
  }, [open, modelSettings, theme, requestedProvider]);

  const summarySelectModels = summaryModels.length ? summaryModels : availableModels;
  const embeddingSelectModels = embeddingModels.length ? embeddingModels : availableModels;

  const handleSave = () => {
    setTheme(darkMode ? 'dark' : 'light');
    setModelSettings(localSettings);
    onOpenChange(false);
  };

  const handleShutdown = () => {
    if (confirm('서버를 종료하시겠습니까?')) {
      api.shutdown();
    }
  };

  if (!localSettings) {
    return null;
  }

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={`max-w-md ${theme === 'dark' ? 'bg-slate-900 border-slate-700 text-slate-100' : 'bg-white border-slate-200 text-slate-900'}`}>
        <DialogHeader>
          <DialogTitle className={`text-xl ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>설정</DialogTitle>
          <DialogDescription className="sr-only">RecordRoute 애플리케이션 설정을 변경합니다.</DialogDescription>
        </DialogHeader>

        <div className="space-y-6 py-4">
          <div className="space-y-2">
            <Label className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>Whisper 모델 (STT):</Label>
            <Select value={localSettings.whisper} onValueChange={(v: string) => setLocalSettings(s => ({ ...s, whisper: v }))}>
              <SelectTrigger className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-slate-50 border-slate-300'}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark' ? 'bg-slate-800 border-slate-700' : ''}>
                {['large-v3-turbo', 'large-v3', 'large-v2', 'medium', 'small', 'base', 'tiny'].map(m => (
                  <SelectItem key={m} value={m}>{m}{m === (defaults?.whisper || 'large-v3-turbo') ? ' (기본값)' : ''}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>Whisper 언어:</Label>
            <Select value={localSettings.language || 'auto'} onValueChange={(v: string) => setLocalSettings(s => ({ ...s, language: v === 'auto' ? '' : v }))}>
              <SelectTrigger className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-slate-50 border-slate-300'}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark' ? 'bg-slate-800 border-slate-700' : ''}>
                <SelectItem value="auto">자동 감지</SelectItem>
                <SelectItem value="ko">한국어</SelectItem>
                <SelectItem value="en">English</SelectItem>
                <SelectItem value="ja">日本語</SelectItem>
                <SelectItem value="zh">中文</SelectItem>
              </SelectContent>
            </Select>
          </div>


          <div className="space-y-2">
            <Label className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>LLM Provider:</Label>
            <Select value={selectedProvider} onValueChange={(v: 'ollama' | 'llamacpp') => setLocalSettings(s => ({ ...s, provider: v, llm_provider: v }))}>
              <SelectTrigger className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-slate-50 border-slate-300'}>
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark' ? 'bg-slate-800 border-slate-700' : ''}>
                <SelectItem value="ollama">ollama</SelectItem>
                <SelectItem value="llamacpp">llamacpp</SelectItem>
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>요약 모델:</Label>
            <Select value={localSettings.summarize} onValueChange={(v: string) => setLocalSettings(s => ({ ...s, summarize: v }))}>
              <SelectTrigger className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-slate-50 border-slate-300'}>
                <SelectValue placeholder="모델 선택..." />
              </SelectTrigger>
              <SelectContent className={theme === 'dark' ? 'bg-slate-800 border-slate-700' : ''}>
                {summarySelectModels.map(m => (
                  <SelectItem key={m} value={m}>{m}{m === defaults?.summarize ? ' (기본값)' : ''}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="space-y-2">
            <Label className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>임베딩 모델:</Label>
            <Select value={localSettings.embedding} onValueChange={(v: string) => setLocalSettings(s => ({ ...s, embedding: v }))}>
              <SelectTrigger className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-slate-50 border-slate-300'}>
                <SelectValue placeholder="모델 선택..." />
              </SelectTrigger>
              <SelectContent className={theme === 'dark' ? 'bg-slate-800 border-slate-700' : ''}>
                {embeddingSelectModels.map(m => (
                  <SelectItem key={m} value={m}>{m}{m === defaults?.embedding ? ' (기본값)' : ''}</SelectItem>
                ))}
              </SelectContent>
            </Select>
          </div>

          <div className="flex items-center justify-between py-2">
            <Label className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>다크 모드</Label>
            <Switch checked={darkMode} onCheckedChange={setDarkMode} />
          </div>
        </div>

        <DialogFooter className="flex-row justify-between items-center">
          <Button variant="outline" onClick={handleShutdown}
            className="bg-red-900/20 border-red-700 text-red-400 hover:bg-red-900/30 hover:text-red-300">
            <Power className="size-4 mr-2" /> 종료
          </Button>
          <div className="flex gap-2">
            <Button variant="outline" onClick={() => onOpenChange(false)}
              className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700' : ''}>
              취소
            </Button>
            <Button onClick={handleSave}
              className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500">
              확인
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
