import { useState, useCallback } from 'react';
import { FileAudio, FileText, Download, Trash2, Calendar, Search, Pencil, Check, X, Play, ChevronDown } from 'lucide-react';
import { Card } from './ui/card';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Input } from './ui/input';
import { TaskActionControls } from './TaskActionControls';
import { useTheme } from '../contexts/ThemeContext';
import { useApp } from '../contexts/AppContext';
import * as api from '../api/client';
import type { HistoryRecord, TaskType } from '../api/types';
import { QueueTaskStatus } from '../api/types';

interface HistoryPanelProps {
  onViewContent: (fileIdentifier: string, fileType: 'stt' | 'summary', record: HistoryRecord) => void;
  onShowSimilarDocs: (filePath: string, filename: string) => void;
  onShowResetAll: () => void;
}

export function HistoryPanel({ onViewContent, onShowSimilarDocs, onShowResetAll }: HistoryPanelProps) {
  const { theme } = useTheme();
  const { historyState, loadHistory, deleteRecords, updateFilename, addTask, queue, currentTask } = useApp();
  const history = historyState.data;
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState('');
  const [sortBy, setSortBy] = useState<'newest' | 'oldest'>('newest');

  const filteredHistory = history
    .filter(item => item.filename.toLowerCase().includes(searchQuery.toLowerCase()))
    .sort((a, b) => sortBy === 'newest' ? new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime() : new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime());

  const toggleSelect = (id: string) => setSelectedIds(prev => {
    const next = new Set(prev);
    next.has(id) ? next.delete(id) : next.add(id);
    return next;
  });

  const handleDeleteSelected = async () => {
    if (selectedIds.size === 0) return;
    if (!confirm(`${selectedIds.size}개의 기록을 삭제하시겠습니까?`)) return;
    await deleteRecords(Array.from(selectedIds));
    setSelectedIds(new Set());
  };

  const saveEditing = async () => {
    if (editingId && editingName.trim()) await updateFilename(editingId, editingName.trim().normalize('NFC'));
    setEditingId(null);
  };

  const getQueueTask = (record: HistoryRecord, taskType: TaskType) => {
    const allTasks = currentTask ? [currentTask, ...queue] : queue;
    return allTasks.find(t => t.recordId === record.id && t.taskType === taskType);
  };

  const getTaskStatus = (record: HistoryRecord, taskType: TaskType) => {
    if (record.completed_tasks?.[taskType] || record.info?.[`${taskType}_completed`]) return QueueTaskStatus.Completed;
    return getQueueTask(record, taskType)?.status || QueueTaskStatus.Pending;
  };

  const handleTaskButton = (record: HistoryRecord, taskType: TaskType) => {
    const status = getTaskStatus(record, taskType);
    if (status === QueueTaskStatus.Completed) {
      if (taskType === 'embedding') {
        onShowSimilarDocs(record.file_path || record.id, record.filename);
      } else {
        const fileType = taskType === 'stt' ? 'stt' : taskType === 'summary' ? 'summary' : null;
        if (!fileType) return;
        const downloadLink = record.download_links?.[fileType];
        const fileIdentifier = downloadLink ? downloadLink.replace('/download/', '') : record.id;
        onViewContent(fileIdentifier, fileType, record);
      }
      return;
    }

    if (status === QueueTaskStatus.Pending || status === QueueTaskStatus.Error || status === QueueTaskStatus.Cancelled) {
      addTask(record.id, record.file_path || record.id, taskType);
    }
  };

  const getProcessTaskTypes = (record: HistoryRecord): TaskType[] => {
    if (record.file_type === 'audio') return ['stt', 'embedding', 'summary'];
    return ['embedding', 'summary'];
  };

  const handleProcessAll = () => {
    let tasksAdded = 0;

    history.forEach(record => {
      getProcessTaskTypes(record).forEach(taskType => {
        const status = getTaskStatus(record, taskType);
        if (status === QueueTaskStatus.Pending || status === QueueTaskStatus.Error || status === QueueTaskStatus.Cancelled) {
          addTask(record.id, record.file_path || record.id, taskType);
          tasksAdded += 1;
        }
      });
    });

    if (tasksAdded > 0) {
      alert(`${tasksAdded}개의 작업이 큐에 추가되었습니다.`);
      return;
    }

    alert('진행할 미완료 작업이 없습니다.');
  };

  const getTaskBtnClass = (status: QueueTaskStatus) => {
    if (status === QueueTaskStatus.Completed) return 'bg-green-500/20 text-green-400 border-green-500/50 hover:bg-green-500/30';
    if (status === QueueTaskStatus.Processing) return 'bg-yellow-500/20 text-yellow-400 border-yellow-500/50 cursor-not-allowed';
    if (status === QueueTaskStatus.Queued) return 'bg-blue-500/20 text-blue-400 border-blue-500/50 cursor-not-allowed';
    if (status === QueueTaskStatus.Error || status === QueueTaskStatus.Cancelled) return 'bg-red-500/20 text-red-300 border-red-500/50';
    return theme === 'dark' ? 'border-slate-600 text-slate-400 hover:border-violet-500 hover:bg-violet-500/10' : 'border-slate-400 text-slate-600 hover:border-violet-500 hover:bg-violet-500/10';
  };

  const FileIcon = useCallback(({ fileType }: { fileType: string }) => fileType === 'audio' ? <FileAudio className="size-4 text-white" /> : <FileText className="size-4 text-white" />, []);

  return (
    <Card className={`backdrop-blur-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'}`}>
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between gap-2">
          <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>업로드 기록</h2>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={handleProcessAll}><Play className="size-3 mr-1" /> 전체 진행</Button>
            <Button variant="outline" size="sm" onClick={onShowResetAll}><Play className="size-3 mr-1" /> 전체 초기화</Button>
            {selectedIds.size > 0 && <Button variant="outline" size="sm" onClick={handleDeleteSelected}><Trash2 className="size-3 mr-1" /> 삭제 ({selectedIds.size})</Button>}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="relative flex-1"><Search className="absolute left-3 top-2.5 size-4 text-slate-500" /><Input value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} placeholder="파일명 검색..." className="pl-9" /></div>
          <select value={sortBy} onChange={(e) => setSortBy(e.target.value as 'newest' | 'oldest')}
            className={`text-sm rounded-lg border px-2 py-1 ${theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-white border-slate-300 text-slate-900'}`}>
            <option value="newest">최신순</option><option value="oldest">오래된 순</option>
          </select>
        </div>

        <ScrollArea className="h-[600px] pr-4">
          {filteredHistory.length === 0 ? (
            <div className="text-center py-12">
              <div className={`inline-block p-4 rounded-full mb-4 ${theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'}`}><Calendar className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} /></div>
              <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>{searchQuery ? '검색 결과가 없습니다' : historyState.loading ? '로딩 중...' : '업로드 기록이 없습니다'}</p>
              {historyState.error && <p className="text-red-400 text-xs mt-2">{historyState.error}</p>}
            </div>
          ) : (
            <div className="space-y-3">
              {filteredHistory.map(item => {
                const sttStatus = getTaskStatus(item, 'stt');
                const embStatus = getTaskStatus(item, 'embedding');
                const sumStatus = getTaskStatus(item, 'summary');
                return (
                  <div key={item.id} className={`p-4 rounded-xl border transition-all space-y-3 group ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'}`}>
                    <div className="flex items-start gap-3">
                      <input type="checkbox" checked={selectedIds.has(item.id)} onChange={() => toggleSelect(item.id)} className="mt-1.5 rounded border-slate-600" />
                      <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600"><FileIcon fileType={item.file_type} /></div>
                      <div className="flex-1 min-w-0">
                        {editingId === item.id ? (
                          <div className="flex items-center gap-2">
                            <input type="text" value={editingName} onChange={(e) => setEditingName(e.target.value)} onKeyDown={(e) => { if (e.key === 'Enter') saveEditing(); if (e.key === 'Escape') setEditingId(null); }} className="flex-1 text-sm px-2 py-1 rounded border" />
                            <button onClick={saveEditing} className="text-green-400 hover:text-green-300"><Check className="size-4" /></button>
                            <button onClick={() => setEditingId(null)} className="text-red-400 hover:text-red-300"><X className="size-4" /></button>
                          </div>
                        ) : (
                          <div className="flex items-center gap-2">
                            <p className={`font-medium truncate ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>{item.filename.normalize('NFC')}</p>
                            <button onClick={() => { setEditingId(item.id); setEditingName(item.filename); }} className="opacity-0 group-hover:opacity-100 transition-opacity"><Pencil className={`size-3 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-400'}`} /></button>
                          </div>
                        )}
                      </div>
                    </div>

                    <div className="flex flex-wrap gap-2 pl-12">
                      <button onClick={() => handleTaskButton(item, 'stt')} disabled={sttStatus === QueueTaskStatus.Queued || sttStatus === QueueTaskStatus.Processing} className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskBtnClass(sttStatus)}`}>STT</button>
                      <button onClick={() => handleTaskButton(item, 'embedding')} disabled={embStatus === QueueTaskStatus.Queued || embStatus === QueueTaskStatus.Processing} className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskBtnClass(embStatus)}`}>색인</button>
                      <button onClick={() => handleTaskButton(item, 'summary')} disabled={sumStatus === QueueTaskStatus.Queued || sumStatus === QueueTaskStatus.Processing} className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskBtnClass(sumStatus)}`}>요약</button>
                      <a href={api.getDownloadUrl(item.file_path || item.id)} download className={`text-xs px-3 py-1 rounded-lg border transition-colors ${theme === 'dark' ? 'border-slate-600 text-slate-400 hover:border-green-500 hover:bg-green-500/10' : 'border-slate-400 text-slate-600 hover:border-green-500 hover:bg-green-50'}`}><Download className="size-3 inline mr-1" /> 다운로드</a>
                    </div>

                    <TaskActionControls
                      canRetry={[sttStatus, embStatus, sumStatus].some((s) => s === QueueTaskStatus.Error || s === QueueTaskStatus.Cancelled)}
                      onRetry={() => {
                        if (sttStatus === QueueTaskStatus.Error || sttStatus === QueueTaskStatus.Cancelled) addTask(item.id, item.file_path || item.id, 'stt');
                        if (embStatus === QueueTaskStatus.Error || embStatus === QueueTaskStatus.Cancelled) addTask(item.id, item.file_path || item.id, 'embedding');
                        if (sumStatus === QueueTaskStatus.Error || sumStatus === QueueTaskStatus.Cancelled) addTask(item.id, item.file_path || item.id, 'summary');
                      }}
                    />
                  </div>
                );
              })}
            </div>
          )}
        </ScrollArea>
      </div>
    </Card>
  );
}
