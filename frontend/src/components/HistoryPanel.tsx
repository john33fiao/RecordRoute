import { useEffect, useMemo, useState } from 'react';
import { Trash2, Trash, Calendar, Search, Play, ChevronDown } from 'lucide-react';
import { Card } from './ui/card';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { Input } from './ui/input';
import { useTheme } from '../contexts/ThemeContext';
import { useApp } from '../contexts/AppContext';
import type { HistoryRecord, TaskType } from '../api/types';
import { QueueTaskStatus } from '../api/types';
import { HistoryListItem } from './HistoryListItem';

interface HistoryPanelProps {
  onViewContent: (fileIdentifier: string, fileType: 'stt' | 'summary', record: HistoryRecord) => void;
  onShowSimilarDocs: (filePath: string, filename: string) => void;
  onShowResetAll: () => void;
}

const LIST_HEIGHT = 600;
const ITEM_HEIGHT = 196;
const OVERSCAN = 4;
const VIRTUALIZATION_THRESHOLD = 80;

export function HistoryPanel({ onViewContent, onShowSimilarDocs, onShowResetAll }: HistoryPanelProps) {
  const { theme } = useTheme();
  const { historyState, loadHistory, deleteRecords, updateFilename, addTask, queue, currentTask } = useApp();
  const history = historyState.data;

  const [searchQuery, setSearchQuery] = useState('');
  const [selectedIds, setSelectedIds] = useState<Set<string>>(new Set());
  const [editingId, setEditingId] = useState<string | null>(null);
  const [editingName, setEditingName] = useState('');
  const [sortBy, setSortBy] = useState<'newest' | 'oldest'>('newest');
  const [scrollTop, setScrollTop] = useState(0);

  const filteredHistory = useMemo(() => {
    return history
      .filter((item) => item.filename.toLowerCase().includes(searchQuery.toLowerCase()))
      .sort((a, b) => (sortBy === 'newest'
        ? new Date(b.timestamp).getTime() - new Date(a.timestamp).getTime()
        : new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()));
  }, [history, searchQuery, sortBy]);

  useEffect(() => {
    setScrollTop(0);
  }, [searchQuery, sortBy]);

  const toggleSelect = (id: string) => {
    setSelectedIds((prev) => {
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

  const saveEditing = async () => {
    if (editingId && editingName.trim()) {
      await updateFilename(editingId, editingName.trim().normalize('NFC'));
    }
    setEditingId(null);
  };

  const getQueueTask = (record: HistoryRecord, taskType: TaskType) => {
    const allTasks = currentTask ? [currentTask, ...queue] : queue;
    return allTasks.find((t) => t.recordId === record.id && t.taskType === taskType);
  };

  const getTaskStatus = (record: HistoryRecord, taskType: TaskType) => {
    if (record.completed_tasks?.[taskType] || record.info?.[`${taskType}_completed`]) {
      return QueueTaskStatus.Completed;
    }
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
    history.forEach((record) => {
      getProcessTaskTypes(record).forEach((taskType) => {
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

  const handleRetryFailed = (record: HistoryRecord, statuses: Record<TaskType, QueueTaskStatus>) => {
    if (statuses.stt === QueueTaskStatus.Error || statuses.stt === QueueTaskStatus.Cancelled) {
      addTask(record.id, record.file_path || record.id, 'stt');
    }
    if (statuses.embedding === QueueTaskStatus.Error || statuses.embedding === QueueTaskStatus.Cancelled) {
      addTask(record.id, record.file_path || record.id, 'embedding');
    }
    if (statuses.summary === QueueTaskStatus.Error || statuses.summary === QueueTaskStatus.Cancelled) {
      addTask(record.id, record.file_path || record.id, 'summary');
    }
  };

  const virtualizationEnabled = filteredHistory.length >= VIRTUALIZATION_THRESHOLD;
  const visibleCount = Math.ceil(LIST_HEIGHT / ITEM_HEIGHT);
  const startIndex = virtualizationEnabled ? Math.max(0, Math.floor(scrollTop / ITEM_HEIGHT) - OVERSCAN) : 0;
  const endIndex = virtualizationEnabled ? Math.min(filteredHistory.length - 1, startIndex + visibleCount + OVERSCAN * 2) : filteredHistory.length - 1;
  const visibleItems = filteredHistory.slice(startIndex, endIndex + 1);

  const totalHeight = filteredHistory.length * ITEM_HEIGHT;
  const topPadding = startIndex * ITEM_HEIGHT;

  return (
    <Card className={`backdrop-blur-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'}`}>
      <div className="p-6 space-y-4">
        <div className="flex items-center justify-between gap-2">
          <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>업로드 기록</h2>
          <div className="flex items-center gap-2">
            <Button variant="outline" size="sm" onClick={handleProcessAll}><Play className="size-3 mr-1" /> 전체 진행</Button>
            <Button variant="outline" size="sm" onClick={onShowResetAll}><Trash className="size-3 mr-1" /> 전체 초기화</Button>
            {selectedIds.size > 0 && <Button variant="outline" size="sm" onClick={handleDeleteSelected}><Trash2 className="size-3 mr-1" /> 삭제 ({selectedIds.size})</Button>}
          </div>
        </div>

        <div className="flex items-center gap-2">
          <div className="relative flex-1"><Search className="absolute left-3 top-2.5 size-4 text-slate-500" /><Input value={searchQuery} onChange={(e) => setSearchQuery(e.target.value)} placeholder="파일명 검색..." className="pl-9" /></div>
          <select value={sortBy} onChange={(e) => setSortBy(e.target.value as 'newest' | 'oldest')}
            className={`text-sm rounded-lg border px-2 py-1 ${theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-white border-slate-300 text-slate-900'}`}>
            <option value="newest">최신순</option><option value="oldest">오래된 순</option>
          </select>
          {virtualizationEnabled && (
            <Badge variant="outline" className="gap-1">
              <ChevronDown className="size-3" /> 가상 스크롤
            </Badge>
          )}
        </div>

        <div className="h-[600px] overflow-y-auto pr-4" onScroll={(e) => setScrollTop(e.currentTarget.scrollTop)}>
          {filteredHistory.length === 0 ? (
            <div className="text-center py-12">
              <div className={`inline-block p-4 rounded-full mb-4 ${theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'}`}><Calendar className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} /></div>
              <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>{searchQuery ? '검색 결과가 없습니다' : historyState.loading ? '로딩 중...' : '업로드 기록이 없습니다'}</p>
              {historyState.error && <p className="text-red-400 text-xs mt-2">{historyState.error}</p>}
            </div>
          ) : (
            <div className="relative" style={virtualizationEnabled ? { height: totalHeight } : undefined}>
              <div className="space-y-3" style={virtualizationEnabled ? { transform: `translateY(${topPadding}px)` } : undefined}>
                {visibleItems.map((item) => {
                  const sttStatus = getTaskStatus(item, 'stt');
                  const embStatus = getTaskStatus(item, 'embedding');
                  const sumStatus = getTaskStatus(item, 'summary');

                  return (
                    <div key={item.id} style={virtualizationEnabled ? { minHeight: ITEM_HEIGHT - 12 } : undefined}>
                      <HistoryListItem
                        item={item}
                        theme={theme}
                        isSelected={selectedIds.has(item.id)}
                        isEditing={editingId === item.id}
                        editingName={editingName}
                        sttStatus={sttStatus}
                        embStatus={embStatus}
                        sumStatus={sumStatus}
                        onToggleSelect={toggleSelect}
                        onStartEditing={(id, filename) => {
                          setEditingId(id);
                          setEditingName(filename);
                        }}
                        onCancelEditing={() => setEditingId(null)}
                        onChangeEditingName={setEditingName}
                        onSaveEditing={saveEditing}
                        onTaskAction={handleTaskButton}
                        onRetryFailed={handleRetryFailed}
                      />
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>

        <div className={`text-xs ${theme === 'dark' ? 'text-slate-400' : 'text-slate-500'}`}>
          전체 {filteredHistory.length}개 항목 · 현재 렌더링 {visibleItems.length}개
        </div>

        <Button variant="outline" size="sm" onClick={loadHistory}>새로고침</Button>
      </div>
    </Card>
  );
}
