import { useState, useEffect, useCallback, useMemo } from 'react';
import { Dialog, DialogContent, DialogHeader, DialogTitle, DialogDescription } from './ui/dialog';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Badge } from './ui/badge';
import { Copy, Download, Edit, Trash2, X, Save } from 'lucide-react';
import { useTheme } from '../contexts/ThemeContext';
import * as api from '../api/client';
import type { SegmentItem } from '../api/types';

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
  const [speakerFilter, setSpeakerFilter] = useState<string>('all');
  const [groupBySpeaker, setGroupBySpeaker] = useState(false);
  const [segments, setSegments] = useState<SegmentItem[]>([]);

  useEffect(() => {
    if (open && fileIdentifier) {
      setLoading(true);
      setEditing(false);
      setSpeakerFilter('all');
      setGroupBySpeaker(false);
      Promise.all([
        api.downloadFileAsText(fileIdentifier),
        fileType === 'stt' ? api.getSttSegments(fileIdentifier) : Promise.resolve([]),
      ])
        .then(([text, loadedSegments]) => {
          setContent(text);
          setEditContent(text);
          setSegments(loadedSegments);
        })
        .catch(() => {
          setContent('파일을 불러올 수 없습니다.');
          setEditContent('파일을 불러올 수 없습니다.');
          setSegments([]);
        })
        .finally(() => setLoading(false));
    }
  }, [open, fileIdentifier, fileType]);

  const speakerOptions = useMemo(() => {
    const unique = new Set<string>();
    segments.forEach((segment) => {
      if (segment.speaker) unique.add(segment.speaker);
    });
    return Array.from(unique).sort();
  }, [segments]);

  const visibleSegments = useMemo(() => {
    if (speakerFilter === 'all') return segments;
    if (speakerFilter === 'unlabeled') return segments.filter((segment) => !segment.speaker);
    return segments.filter((segment) => segment.speaker === speakerFilter);
  }, [segments, speakerFilter]);

  const groupedSegments = useMemo(() => {
    if (!groupBySpeaker) return [] as Array<{ speaker: string | null; items: SegmentItem[] }>;
    const groups: Array<{ speaker: string | null; items: SegmentItem[] }> = [];
    visibleSegments.forEach((segment) => {
      const last = groups[groups.length - 1];
      if (!last || last.speaker !== segment.speaker) {
        groups.push({ speaker: segment.speaker, items: [segment] });
      } else {
        last.items.push(segment);
      }
    });
    return groups;
  }, [visibleSegments, groupBySpeaker]);

  const handleCopy = useCallback(async () => {
    try {
      await navigator.clipboard.writeText(editing ? editContent : content);
    } catch {
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

  const speakerBadgeClass = (speaker: string | null) => (speaker
    ? 'bg-violet-500/20 border-violet-500/40 text-violet-300'
    : 'bg-slate-500/20 border-slate-500/40 text-slate-300');

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
          {fileType === 'stt' && !editing && (
            <>
              <select
                value={speakerFilter}
                onChange={(e) => setSpeakerFilter(e.target.value)}
                className={`text-xs rounded-md border px-2 py-1 ${theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-white border-slate-200 text-slate-800'}`}
              >
                <option value="all">전체 화자</option>
                {speakerOptions.map((speaker) => <option key={speaker} value={speaker}>{speaker}</option>)}
                <option value="unlabeled">화자 미지정</option>
              </select>
              <Button size="sm" variant="outline" onClick={() => setGroupBySpeaker((prev) => !prev)}
                className={groupBySpeaker ? 'border-violet-500/50 text-violet-300 bg-violet-500/10' : ''}>
                {groupBySpeaker ? '묶음 보기 ON' : '묶음 보기 OFF'}
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
          ) : fileType === 'stt' && segments.length > 0 ? (
            <div className="space-y-3">
              {groupBySpeaker
                ? groupedSegments.map((group, idx) => (
                  <div key={`${group.speaker ?? 'unknown'}-${idx}`} className="space-y-2">
                    <Badge variant="outline" className={speakerBadgeClass(group.speaker)}>{group.speaker ?? '미지정'}</Badge>
                    <div className="space-y-2 pl-3 border-l border-slate-700/50">
                      {group.items.map((segment, segIdx) => (
                        <div key={`${idx}-${segIdx}`} className="text-sm leading-relaxed">
                          {segment.text || ''}
                        </div>
                      ))}
                    </div>
                  </div>
                ))
                : visibleSegments.map((segment, idx) => (
                  <div key={idx} className="flex gap-3 items-start">
                    <Badge variant="outline" className={`${speakerBadgeClass(segment.speaker)} min-w-[100px] justify-center`}>
                      {segment.speaker ?? '미지정'}
                    </Badge>
                    <p className={`font-mono text-sm leading-relaxed ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>{segment.text || ''}</p>
                  </div>
                ))}
            </div>
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
