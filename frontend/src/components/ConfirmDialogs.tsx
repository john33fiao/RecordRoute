import { useState } from 'react';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { useTheme } from '../contexts/ThemeContext';
import * as api from '../api/client';

interface ResetAllDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  onComplete: () => void;
}

export function ResetAllDialog({ open, onOpenChange, onComplete }: ResetAllDialogProps) {
  const { theme } = useTheme();
  const [selected, setSelected] = useState<Record<string, boolean>>({ stt: false, embedding: false, summary: false });
  const allChecked = selected.stt && selected.embedding && selected.summary;

  const toggleAll = () => {
    const next = !allChecked;
    setSelected({ stt: next, embedding: next, summary: next });
  };

  const handleConfirm = async () => {
    const tasks = Object.entries(selected).filter(([, v]) => v).map(([k]) => k);
    if (tasks.length === 0) return;
    if (!confirm(`선택한 항목(${tasks.join(', ')})을 전체 초기화하시겠습니까?`)) return;
    try {
      await api.resetAllTasks(tasks);
      onComplete();
      onOpenChange(false);
    } catch (e) {
      console.error('Reset failed:', e);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={`max-w-sm ${theme === 'dark' ? 'bg-slate-900 border-slate-700 text-slate-100' : 'bg-white border-slate-200 text-slate-900'}`}>
        <DialogHeader>
          <DialogTitle>전체 초기화</DialogTitle>
          <DialogDescription className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>초기화할 항목을 선택하세요.</DialogDescription>
        </DialogHeader>
        <div className="space-y-3 py-4">
          <label className="flex items-center gap-2 cursor-pointer">
            <input type="checkbox" checked={allChecked} onChange={toggleAll} className="rounded border-slate-600" />
            <span className={`font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>전체</span>
          </label>
          {[['stt', 'STT'], ['embedding', '색인'], ['summary', '요약']].map(([key, label]) => (
            <label key={key} className="flex items-center gap-2 cursor-pointer pl-4">
              <input type="checkbox" checked={selected[key]} onChange={() => setSelected(s => ({ ...s, [key]: !s[key] }))} className="rounded border-slate-600" />
              <span className={theme === 'dark' ? 'text-slate-300' : 'text-slate-700'}>{label}</span>
            </label>
          ))}
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}
            className={theme === 'dark' ? 'border-slate-700 text-slate-300' : ''}>취소</Button>
          <Button onClick={handleConfirm}
            className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500">확인</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

interface SttEditResetDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  recordId: string | null;
  onComplete: () => void;
}

export function SttEditResetDialog({ open, onOpenChange, recordId, onComplete }: SttEditResetDialogProps) {
  const { theme } = useTheme();

  const handleReset = async () => {
    if (!recordId) return;
    try {
      await api.resetSummaryEmbedding(recordId);
      onComplete();
      onOpenChange(false);
    } catch (e) {
      console.error('Reset failed:', e);
    }
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={`max-w-sm ${theme === 'dark' ? 'bg-slate-900 border-slate-700 text-slate-100' : 'bg-white border-slate-200 text-slate-900'}`}>
        <DialogHeader>
          <DialogTitle>초기화 확인</DialogTitle>
          <DialogDescription className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
            텍스트 수정이 완료되었습니다. 저장된 색인 및 요약을 초기화 하시겠습니까?
          </DialogDescription>
        </DialogHeader>
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}
            className={theme === 'dark' ? 'border-slate-700 text-slate-300' : ''}>닫기</Button>
          <Button onClick={handleReset}
            className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500">초기화</Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
