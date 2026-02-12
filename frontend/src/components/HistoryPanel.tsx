import { useState, useCallback } from 'react';
import { FileAudio, FileText, Download, Trash2, Calendar, Search, Pencil, Check, X, Play, ChevronDown } from 'lucide-react';
import { Card } from './ui/card';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Input } from './ui/input';
import { useTheme } from '../contexts/ThemeContext';
import { useApp } from '../contexts/AppContext';
import * as api from '../api/client';
import type { HistoryRecord, TaskType } from '../api/types';

interface HistoryPanelProps {
  onViewContent: (fileIdentifier: string, fileType: 'stt' | 'summary', record: HistoryRecord) => void;
  onShowSimilarDocs: (filePath: string, filename: string) => void;
  onShowResetAll: () => void;
}

export function HistoryPanel({ onViewContent, onShowSimilarDocs, onShowResetAll }: HistoryPanelProps) {
  const { theme } = useTheme();
  const { history, historyLoading, loadHistory, deleteRecords, updateFilename, addTask, queue, currentTask } = useApp();
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState('');
  const [sortBy, setSortBy] = useState<'newest' | 'oldest'>('newest');

  const filteredHistory = history
    .filter(item => item.filename.toLowerCase().includes(searchQuery.toLowerCase()))
    .sort((a, b) => {
      const dateA = new Date(a.timestamp).getTime();
      const dateB = new Date(b.timestamp).getTime();
      return sortBy === 'newest' ? dateB - dateA : dateA - dateB;
    });

  const toggleSelect = (id: string) => {
    setSelectedIds(prev => {
      const next = new Set(prev);
      next.has(id) ? next.delete(id) : next.add(id);
      return next;
    });
  };

  const handleDeleteSelected = async () => {
    if (selectedIds.size === 0) return;
    if (!confirm(`${selectedIds.size}개의 기록을 삭제하시겠습니까?`)) return;
    await deleteRecords(Array.from(selectedIds));
    setSelectedIds(new Set());
  };

  const startEditing = (record: HistoryRecord) => {
    setEditingId(record.id);
    setEditingName(record.filename);
  };

  const saveEditing = async () => {
    if (editingId && editingName.trim()) {
      await updateFilename(editingId, editingName.trim().normalize('NFC'));
    }
    setEditingId(null);
  };

  const cancelEditing = () => {
    setEditingId(null);
  };

  const getTaskStatus = (record: HistoryRecord, taskType: string): 'completed' | 'pending' | 'queued' | 'processing' => {
    // Check top-level completed_tasks first, fallback to info for legacy compatibility if needed
    const isComplete = record.completed_tasks?.[taskType] || record.info?.[`${taskType}_completed`];

    if (isComplete) return 'completed';

    const allTasks = currentTask ? [currentTask, ...queue] : queue;
    const inQueue = allTasks.find(t => t.recordId === record.id && t.taskType === taskType);
    if (inQueue) {
      return inQueue.status === 'processing' ? 'processing' : 'queued';
    }
    return 'pending';
  };

  const handleTaskButton = (record: HistoryRecord, taskType: TaskType) => {
    const status = getTaskStatus(record, taskType);
    if (status === 'completed') {
      if (taskType === 'embedding') {
        const filePath = record.file_path || record.info?.file_path || record.id;
        onShowSimilarDocs(filePath, record.filename);
      } else {
        const fileType = taskType === 'stt' ? 'stt' : taskType === 'summary' ? 'summary' : null;
        if (fileType) {
          // Extract UUID from download link if available
          const downloadLink = record.download_links?.[fileType];
          const fileIdentifier = downloadLink ? downloadLink.replace('/download/', '') : record.id;
          onViewContent(fileIdentifier, fileType, record);
        }
      }
    } else if (status === 'pending') {
      // Use record.file_path if available, otherwise record.id
      const filePath = record.file_path || record.info?.file_path || record.id;
      addTask(record.id, filePath, taskType);
    }
  };

  const handleProcessAll = () => {
    history.forEach(record => {
      const filePath = record.file_path || record.info?.file_path || record.id;
      if (!getTaskStatus(record, 'stt').match(/completed|queued|processing/)) {
        if (record.file_type === 'audio') addTask(record.id, filePath, 'stt');
      }
      if (!getTaskStatus(record, 'embedding').match(/completed|queued|processing/)) {
        addTask(record.id, filePath, 'embedding');
      }
    });
  };

  const formatDate = (dateStr: string) => {
    try {
      return new Date(dateStr).toLocaleString('ko-KR', { year: 'numeric', month: '2-digit', day: '2-digit', hour: '2-digit', minute: '2-digit' });
    } catch { return dateStr; }
  };

  const getTaskBtnClass = (status: string) => {
    if (status === 'completed') return 'bg-green-500/20 text-green-400 border-green-500/50 hover:bg-green-500/30';
    if (status === 'processing') return 'bg-yellow-500/20 text-yellow-400 border-yellow-500/50 cursor-not-allowed';
    if (status === 'queued') return 'bg-blue-500/20 text-blue-400 border-blue-500/50 cursor-not-allowed';
    return theme === 'dark' ? 'border-slate-600 text-slate-400 hover:border-violet-500 hover:bg-violet-500/10' : 'border-slate-400 text-slate-600 hover:border-violet-500 hover:bg-violet-500/10';
  };

  const FileIcon = useCallback(({ fileType }: { fileType: string }) => {
    return fileType === 'audio' ? <FileAudio className="size-4 text-white" /> : <FileText className="size-4 text-white" />;
  }, []);

  return (
    <Card className={`backdrop-blur-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'}`}>
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between gap-4 flex-wrap">
          <div className="flex items-center gap-3">
            <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>업로드 기록</h2>
            <Badge variant="outline" className="border-violet-500 text-violet-400">{filteredHistory.length}개</Badge>
          </div>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={handleProcessAll}
              className={theme === 'dark' ? 'border-green-500/50 text-green-400 hover:bg-green-500/10' : 'border-green-500 text-green-700 hover:bg-green-50'}>
              <Play className="size-3 mr-1" /> 전체 진행
            </Button>
            <Button variant="outline" size="sm" onClick={onShowResetAll}
              className="text-red-400 border-red-500/50 hover:bg-red-500/10">
              전체 초기화
            </Button>
            <Button variant="outline" size="sm" onClick={handleDeleteSelected}
              disabled={selectedIds.size === 0}
              className="text-red-400 border-red-500/50 hover:bg-red-500/10 disabled:opacity-50">
              <Trash2 className="size-3 mr-1" /> 삭제 ({selectedIds.size})
            </Button>
          </div>
        </div>

        <div className="flex gap-2">
          <Input
            type="text" placeholder="파일명으로 검색..." value={searchQuery}
            onChange={(e) => setSearchQuery(e.target.value)}
            className={theme === 'dark' ? 'bg-slate-800/50 border-slate-700 text-slate-200 placeholder:text-slate-500' : 'bg-slate-50 border-slate-300 text-slate-900 placeholder:text-slate-500'}
          />
          <select value={sortBy} onChange={(e) => setSortBy(e.target.value as any)}
            className={`text-sm rounded-lg border px-2 py-1 ${theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-white border-slate-300 text-slate-900'}`}>
            <option value="newest">최신순</option>
            <option value="oldest">오래된 순</option>
          </select>
        </div>

        <ScrollArea className="h-[600px] pr-4">
          {filteredHistory.length === 0 ? (
            <div className="text-center py-12">
              <div className={`inline-block p-4 rounded-full mb-4 ${theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'}`}>
                <Calendar className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} />
              </div>
              <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>
                {searchQuery ? '검색 결과가 없습니다' : historyLoading ? '로딩 중...' : '업로드 기록이 없습니다'}
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {filteredHistory.map(item => {
                const sttStatus = getTaskStatus(item, 'stt');
                const embStatus = getTaskStatus(item, 'embedding');
                const sumStatus = getTaskStatus(item, 'summary');

                return (
                  <div key={item.id} className={`p-4 rounded-xl border transition-all space-y-3 group ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                    }`}>
                    <div className="flex items-start gap-3">
                      <input type="checkbox" checked={selectedIds.has(item.id)}
                        onChange={() => toggleSelect(item.id)}
                        className="mt-1.5 rounded border-slate-600" />
                      <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                        <FileIcon fileType={item.file_type} />
                      </div>
                      <div className="flex-1 min-w-0">
                        {editingId === item.id ? (
                          <div className="flex items-center gap-2">
                            <input type="text" value={editingName} onChange={(e) => setEditingName(e.target.value)}
                              onKeyDown={(e) => { if (e.key === 'Enter') saveEditing(); if (e.key === 'Escape') cancelEditing(); }}
                              autoFocus
                              className={`flex-1 text-sm px-2 py-1 rounded border ${theme === 'dark' ? 'bg-slate-800 border-slate-600 text-slate-200' : 'bg-white border-slate-300'}`} />
                            <button onClick={saveEditing} className="text-green-400 hover:text-green-300"><Check className="size-4" /></button>
                            <button onClick={cancelEditing} className="text-red-400 hover:text-red-300"><X className="size-4" /></button>
                          </div>
                        ) : (
                          <div className="flex items-center gap-2">
                            <p className={`font-medium truncate ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>
                              {item.filename.normalize('NFC')}
                            </p>
                            <button onClick={() => startEditing(item)} className="opacity-0 group-hover:opacity-100 transition-opacity">
                              <Pencil className={`size-3 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-400'}`} />
                            </button>
                          </div>
                        )}
                        <div className={`flex items-center gap-3 mt-1 text-xs ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>
                          <span>{formatDate(item.timestamp)}</span>
                          <span>•</span>
                          <span>{item.file_type}</span>
                        </div>
                      </div>
                    </div>

                    <div className="flex flex-wrap gap-2 pl-12">
                      <button onClick={() => handleTaskButton(item, 'stt')}
                        disabled={sttStatus === 'queued' || sttStatus === 'processing'}
                        className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskBtnClass(sttStatus)}`}>
                        STT
                      </button>
                      <button onClick={() => handleTaskButton(item, 'embedding')}
                        disabled={embStatus === 'queued' || embStatus === 'processing'}
                        className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskBtnClass(embStatus)}`}>
                        색인
                      </button>
                      <button onClick={() => handleTaskButton(item, 'summary')}
                        disabled={sumStatus === 'queued' || sumStatus === 'processing'}
                        className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskBtnClass(sumStatus)}`}>
                        요약
                      </button>
                      <a href={api.getDownloadUrl(item.file_path || item.id)} download
                        className={`text-xs px-3 py-1 rounded-lg border transition-colors ${theme === 'dark' ? 'border-slate-600 text-slate-400 hover:border-green-500 hover:bg-green-500/10' : 'border-slate-400 text-slate-600 hover:border-green-500 hover:bg-green-50'
                          }`}>
                        <Download className="size-3 inline mr-1" /> 다운로드
                      </a>
                    </div>
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
