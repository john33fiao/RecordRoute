import { useState, useEffect, useCallback } from 'react';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from './ui/dialog';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Copy, Download, Edit, Trash2, X, Save } from 'lucide-react';
import { useTheme } from '../contexts/ThemeContext';
import * as api from '../api/client';

interface TextOverlayProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  fileIdentifier: string | null;
  fileType: 'stt' | 'summary';
  filename: string;
  recordId: string;
  onDeleted?: () => void;
  onSttEdited?: (recordId: string) => void;
}

export function TextOverlay({ open, onOpenChange, fileIdentifier, fileType, filename, recordId, onDeleted, onSttEdited }: TextOverlayProps) {
  const { theme } = useTheme();
  const [content, setContent] = useState('');
  const [loading, setLoading] = useState(false);
  const [editing, setEditing] = useState(false);
  const [editContent, setEditContent] = useState('');
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open && fileIdentifier) {
      setLoading(true);
      setEditing(false);
      api.downloadFileAsText(fileIdentifier)
        .then(text => { setContent(text); setEditContent(text); })
        .catch(() => setContent('파일을 불러올 수 없습니다.'))
        .finally(() => setLoading(false));
    }
  }, [open, fileIdentifier]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(editing ? editContent : content);
    } catch {
      // fallback
      const ta = document.createElement('textarea');
      ta.value = editing ? editContent : content;
      document.body.appendChild(ta);
      ta.select();
      document.execCommand('copy');
      document.body.removeChild(ta);
    }
  }, [content, editContent, editing]);

  const handleDownload = useCallback(() => {
    if (!fileIdentifier) return;
    const a = document.createElement('a');
    a.href = api.getDownloadUrl(fileIdentifier);
    a.download = filename;
    a.click();
  }, [fileIdentifier, filename]);

  const handleDelete = useCallback(async () => {
    if (!fileIdentifier) return;
    if (!confirm('이 파일을 삭제하시겠습니까?')) return;
    try {
      await api.deleteFile(fileIdentifier, fileType);
      onDeleted?.();
      onOpenChange(false);
    } catch (e) {
      console.error('Delete failed:', e);
    }
  }, [fileIdentifier, fileType, onDeleted, onOpenChange]);

  const handleSave = useCallback(async () => {
    if (!fileIdentifier) return;
    setSaving(true);
    try {
      await api.updateSttText(fileIdentifier, editContent);
      setContent(editContent);
      setEditing(false);
      onSttEdited?.(recordId);
    } catch (e) {
      console.error('Save failed:', e);
    } finally {
      setSaving(false);
    }
  }, [fileIdentifier, editContent, recordId, onSttEdited]);

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={`max-w-4xl max-h-[85vh] ${theme === 'dark' ? 'bg-slate-900 border-slate-700 text-slate-100' : 'bg-white border-slate-200 text-slate-900'
        }`}>
        <DialogHeader>
          <div className="flex items-start justify-between gap-4">
            <div>
              <DialogTitle className="text-xl">{fileType === 'stt' ? 'STT 결과' : '요약 결과'}</DialogTitle>
              <DialogDescription className={`text-sm mt-1 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>{filename}</DialogDescription>
            </div>
          </div>
        </DialogHeader>

        <div className="flex items-center gap-2 flex-wrap">
          <Button size="sm" variant="outline" onClick={handleCopy}
            className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700' : ''}>
            <Copy className="size-4 mr-2" /> 복사
          </Button>
          {fileType === 'stt' && !editing && (
            <Button size="sm" variant="outline" onClick={() => setEditing(true)}
              className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700' : ''}>
              <Edit className="size-4 mr-2" /> 수정
            </Button>
          )}
          {editing && (
            <>
              <Button size="sm" variant="outline" onClick={handleSave} disabled={saving}
                className="border-green-500/50 text-green-400 hover:bg-green-500/10">
                <Save className="size-4 mr-2" /> {saving ? '저장 중...' : '저장'}
              </Button>
              <Button size="sm" variant="outline" onClick={() => { setEditing(false); setEditContent(content); }}
                className="border-slate-600 text-slate-300">
                취소
              </Button>
            </>
          )}
          <Button size="sm" variant="outline" onClick={handleDownload}
            className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700' : ''}>
            <Download className="size-4 mr-2" /> 다운로드
          </Button>
          <Button size="sm" variant="outline" onClick={handleDelete}
            className="bg-red-900/20 border-red-700 text-red-400 hover:bg-red-900/30">
            <Trash2 className="size-4 mr-2" /> 삭제
          </Button>
        </div>

        <ScrollArea className={`h-[500px] rounded-lg border p-4 ${theme === 'dark' ? 'border-slate-700 bg-slate-950/50' : 'border-slate-200 bg-slate-50'
          }`}>
          {loading ? (
            <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>로딩 중...</p>
          ) : editing ? (
            <textarea
              value={editContent}
              onChange={(e) => setEditContent(e.target.value)}
              className={`w-full h-full min-h-[450px] bg-transparent resize-none font-mono text-sm outline-none ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                }`}
            />
          ) : (
            <pre className={`whitespace-pre-wrap font-mono text-sm leading-relaxed ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
              }`}>{content}</pre>
          )}
        </ScrollArea>

        <div className="flex justify-end pt-2">
          <Button variant="outline" onClick={() => onOpenChange(false)}
            className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700' : ''}>
            <X className="size-4 mr-2" /> 닫기
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
