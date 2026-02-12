import { useState, useEffect } from 'react';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription, DialogFooter } from './ui/dialog';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Badge } from './ui/badge';
import { FileText, X, RefreshCw } from 'lucide-react';
import { useTheme } from '../contexts/ThemeContext';
import * as api from '../api/client';
import type { SimilarDocument } from '../api/types';

interface SimilarDocsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  filePath: string | null;
  filename: string;
}

export function SimilarDocsDialog({ open, onOpenChange, filePath, filename }: SimilarDocsDialogProps) {
  const { theme } = useTheme();
  const [docs, setDocs] = useState<SimilarDocument[]>([]);
  const [loading, setLoading] = useState(false);

  const loadDocs = async (refresh = false) => {
    if (!filePath) return;
    setLoading(true);
    try {
      const data = await api.getSimilarDocs(filePath, filename, refresh);
      setDocs(Array.isArray(data) ? data : []);
    } catch { setDocs([]); }
    finally { setLoading(false); }
  };

  useEffect(() => {
    if (open && filePath) loadDocs();
  }, [open, filePath]);

  const getColor = (score: number) => {
    const pct = score * 100;
    if (pct >= 90) return 'text-green-400 border-green-500';
    if (pct >= 70) return 'text-violet-400 border-violet-500';
    return 'text-slate-400 border-slate-500';
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className={`max-w-3xl max-h-[85vh] ${theme === 'dark' ? 'bg-slate-900 border-slate-700 text-slate-100' : 'bg-white border-slate-200 text-slate-900'}`}>
        <DialogHeader>
          <DialogTitle className="text-xl">유사한 문서 (상위 {docs.length}개)</DialogTitle>
          <DialogDescription className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>기준: {filename}</DialogDescription>
        </DialogHeader>

        <ScrollArea className="h-[500px] pr-4">
          {loading ? (
            <p className={`text-center py-8 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>로딩 중...</p>
          ) : docs.length === 0 ? (
            <p className={`text-center py-8 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>유사한 문서가 없습니다</p>
          ) : (
            <div className="space-y-3">
              {docs.map((doc, i) => (
                <a key={i} href={doc.link || api.getDownloadUrl(doc.file_uuid || doc.file)} target="_blank" rel="noopener noreferrer"
                  className={`block p-4 rounded-xl border transition-all cursor-pointer ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                    }`}>
                  <div className="flex items-start gap-3">
                    <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600 shrink-0">
                      <FileText className="size-4 text-white" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <div className="flex items-start justify-between gap-2">
                        <h3 className={`font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>
                          {(doc.display_name || doc.source_filename || doc.file || '').normalize('NFC')}
                        </h3>
                        <Badge variant="outline" className={getColor(doc.score)}>
                          유사도: {(doc.score * 100).toFixed(0)}%
                        </Badge>
                      </div>
                    </div>
                  </div>
                </a>
              ))}
            </div>
          )}
        </ScrollArea>

        <DialogFooter className="gap-2">
          <Button variant="outline" onClick={() => loadDocs(true)}
            className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700' : ''}>
            <RefreshCw className="size-4 mr-2" /> 갱신
          </Button>
          <Button variant="outline" onClick={() => onOpenChange(false)}
            className={theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700' : ''}>
            <X className="size-4 mr-2" /> 닫기
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
