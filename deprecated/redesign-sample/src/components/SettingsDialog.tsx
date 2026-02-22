import { useState, useEffect } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from './ui/dialog';
import { Button } from './ui/button';
import { Label } from './ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './ui/select';
import { Switch } from './ui/switch';
import { Power } from 'lucide-react';
import { useTheme } from '../contexts/ThemeContext';

interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

export function SettingsDialog({ open, onOpenChange }: SettingsDialogProps) {
  const { theme, setTheme } = useTheme();
  const [sttModel, setSttModel] = useState('large-v3-turbo');
  const [language, setLanguage] = useState('ko');
  const [summaryModel, setSummaryModel] = useState('gemma3.4b');
  const [embeddingModel, setEmbeddingModel] = useState('bge-m3-latest');
  const [darkMode, setDarkMode] = useState(theme === 'dark');

  useEffect(() => {
    setDarkMode(theme === 'dark');
  }, [theme]);

  const handleSave = () => {
    // Apply theme changes
    setTheme(darkMode ? 'dark' : 'light');
    // Save settings logic
    console.log({ sttModel, language, summaryModel, embeddingModel, darkMode });
    onOpenChange(false);
  };

  const handleClose = () => {
    onOpenChange(false);
  };

  const handleShutdown = () => {
    // Shutdown backend server
    if (confirm('백엔드 서버를 종료하시겠습니까?')) {
      console.log('Shutting down backend server...');
      // API call to shutdown server would go here
      // fetch('/api/shutdown', { method: 'POST' })
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={`max-w-md ${
        theme === 'dark'
          ? 'bg-slate-900 border-slate-700 text-slate-100'
          : 'bg-white border-slate-200 text-slate-900'
      }`}>
        <DialogHeader>
          <DialogTitle className={`text-xl ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
            설정
          </DialogTitle>
        </DialogHeader>

        <div className="space-y-6 py-4">
          {/* STT Model */}
          <div className="space-y-2">
            <Label htmlFor="stt-model" className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>
              Whisper 모델 (STT):
            </Label>
            <Select value={sttModel} onValueChange={setSttModel}>
              <SelectTrigger 
                id="stt-model"
                className={theme === 'dark'
                  ? 'bg-slate-800 border-slate-700 text-slate-200'
                  : 'bg-slate-50 border-slate-300 text-slate-900'
                }
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark'
                ? 'bg-slate-800 border-slate-700 text-slate-200'
                : 'bg-white border-slate-300 text-slate-900'
              }>
                <SelectItem value="large-v3-turbo">large-v3-turbo (기본값)</SelectItem>
                <SelectItem value="large-v3">large-v3</SelectItem>
                <SelectItem value="medium">medium</SelectItem>
                <SelectItem value="small">small</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Language */}
          <div className="space-y-2">
            <Label htmlFor="language" className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>
              Whisper 언어:
            </Label>
            <Select value={language} onValueChange={setLanguage}>
              <SelectTrigger 
                id="language"
                className={theme === 'dark'
                  ? 'bg-slate-800 border-slate-700 text-slate-200'
                  : 'bg-slate-50 border-slate-300 text-slate-900'
                }
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark'
                ? 'bg-slate-800 border-slate-700 text-slate-200'
                : 'bg-white border-slate-300 text-slate-900'
              }>
                <SelectItem value="ko">한국어</SelectItem>
                <SelectItem value="en">English</SelectItem>
                <SelectItem value="ja">日本語</SelectItem>
                <SelectItem value="zh">中文</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Summary Model */}
          <div className="space-y-2">
            <Label htmlFor="summary-model" className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>
              요약 모델:
            </Label>
            <Select value={summaryModel} onValueChange={setSummaryModel}>
              <SelectTrigger 
                id="summary-model"
                className={theme === 'dark'
                  ? 'bg-slate-800 border-slate-700 text-slate-200'
                  : 'bg-slate-50 border-slate-300 text-slate-900'
                }
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark'
                ? 'bg-slate-800 border-slate-700 text-slate-200'
                : 'bg-white border-slate-300 text-slate-900'
              }>
                <SelectItem value="gemma3.4b">gemma3.4b (기본값)</SelectItem>
                <SelectItem value="gpt-4">GPT-4</SelectItem>
                <SelectItem value="claude-3">Claude 3</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Embedding Model */}
          <div className="space-y-2">
            <Label htmlFor="embedding-model" className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>
              임베딩 모델:
            </Label>
            <Select value={embeddingModel} onValueChange={setEmbeddingModel}>
              <SelectTrigger 
                id="embedding-model"
                className={theme === 'dark'
                  ? 'bg-slate-800 border-slate-700 text-slate-200'
                  : 'bg-slate-50 border-slate-300 text-slate-900'
                }
              >
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark'
                ? 'bg-slate-800 border-slate-700 text-slate-200'
                : 'bg-white border-slate-300 text-slate-900'
              }>
                <SelectItem value="bge-m3-latest">bge-m3-latest (기본값)</SelectItem>
                <SelectItem value="text-embedding-3-large">text-embedding-3-large</SelectItem>
                <SelectItem value="all-MiniLM-L6-v2">all-MiniLM-L6-v2</SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* Dark Mode Toggle */}
          <div className="flex items-center justify-between py-2">
            <Label htmlFor="dark-mode" className={theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}>
              다크 모드
            </Label>
            <Switch
              id="dark-mode"
              checked={darkMode}
              onCheckedChange={setDarkMode}
            />
          </div>
        </div>

        <DialogFooter className="flex-row justify-between items-center">
          <Button
            variant="outline"
            onClick={handleShutdown}
            className="bg-red-900/20 border-red-700 text-red-400 hover:bg-red-900/30 hover:text-red-300"
          >
            <Power className="size-4 mr-2" />
            종료
          </Button>
          <div className="flex gap-2">
            <Button
              variant="outline"
              onClick={handleClose}
              className={theme === 'dark'
                ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700'
                : 'bg-slate-100 border-slate-300 text-slate-700 hover:bg-slate-200'
              }
            >
              취소
            </Button>
            <Button
              onClick={handleSave}
              className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500"
            >
              확인
            </Button>
          </div>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}