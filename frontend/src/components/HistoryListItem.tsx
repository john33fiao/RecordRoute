import { memo } from 'react';
import { FileAudio, FileText, Download, Pencil, Check, X } from 'lucide-react';
import { TaskActionControls } from './TaskActionControls';
import type { HistoryRecord, TaskType } from '../api/types';
import { QueueTaskStatus } from '../api/types';
import * as api from '../api/client';

interface HistoryListItemProps {
  item: HistoryRecord;
  theme: 'light' | 'dark';
  isSelected: boolean;
  isEditing: boolean;
  editingName: string;
  sttStatus: QueueTaskStatus;
  embStatus: QueueTaskStatus;
  sumStatus: QueueTaskStatus;
  onToggleSelect: (id: string) => void;
  onStartEditing: (id: string, filename: string) => void;
  onCancelEditing: () => void;
  onChangeEditingName: (value: string) => void;
  onSaveEditing: () => void;
  onTaskAction: (record: HistoryRecord, taskType: TaskType) => void;
  onRetryFailed: (record: HistoryRecord, statuses: Record<TaskType, QueueTaskStatus>) => void;
}

function getTaskButtonClass(status: QueueTaskStatus, theme: 'light' | 'dark') {
  if (status === QueueTaskStatus.Completed) return 'bg-green-500/20 text-green-400 border-green-500/50 hover:bg-green-500/30';
  if (status === QueueTaskStatus.Processing) return 'bg-yellow-500/20 text-yellow-400 border-yellow-500/50 cursor-not-allowed';
  if (status === QueueTaskStatus.Queued) return 'bg-blue-500/20 text-blue-400 border-blue-500/50 cursor-not-allowed';
  if (status === QueueTaskStatus.Error || status === QueueTaskStatus.Cancelled) return 'bg-red-500/20 text-red-300 border-red-500/50';
  return theme === 'dark' ? 'border-slate-600 text-slate-400 hover:border-violet-500 hover:bg-violet-500/10' : 'border-slate-400 text-slate-600 hover:border-violet-500 hover:bg-violet-500/10';
}

function FileIcon({ fileType }: { fileType: string }) {
  if (fileType === 'audio') return <FileAudio className="size-4 text-white" />;
  return <FileText className="size-4 text-white" />;
}

function HistoryListItemComponent({
  item,
  theme,
  isSelected,
  isEditing,
  editingName,
  sttStatus,
  embStatus,
  sumStatus,
  onToggleSelect,
  onStartEditing,
  onCancelEditing,
  onChangeEditingName,
  onSaveEditing,
  onTaskAction,
  onRetryFailed,
}: HistoryListItemProps) {
  return (
    <div className={`p-4 rounded-xl border transition-all space-y-3 group ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'}`}>
      <div className="flex items-start gap-3">
        <input type="checkbox" checked={isSelected} onChange={() => onToggleSelect(item.id)} className="mt-1.5 rounded border-slate-600" />
        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600"><FileIcon fileType={item.file_type} /></div>
        <div className="flex-1 min-w-0">
          {isEditing ? (
            <div className="flex items-center gap-2">
              <input
                type="text"
                value={editingName}
                onChange={(e) => onChangeEditingName(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') onSaveEditing();
                  if (e.key === 'Escape') onCancelEditing();
                }}
                className="flex-1 text-sm px-2 py-1 rounded border"
              />
              <button onClick={onSaveEditing} className="text-green-400 hover:text-green-300"><Check className="size-4" /></button>
              <button onClick={onCancelEditing} className="text-red-400 hover:text-red-300"><X className="size-4" /></button>
            </div>
          ) : (
            <div className="space-y-1">
              <div className="flex items-center gap-2">
                <p className={`font-medium truncate ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>{item.filename.normalize('NFC')}</p>
                <button onClick={() => onStartEditing(item.id, item.filename)} className="opacity-0 group-hover:opacity-100 transition-opacity"><Pencil className={`size-3 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-400'}`} /></button>
              </div>
              {item.title_summary.trim() ? (
                <p
                  title={item.title_summary}
                  className={`text-xs truncate ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}
                >
                  {item.title_summary}
                </p>
              ) : null}
            </div>
          )}
        </div>
      </div>

      <div className="flex flex-wrap gap-2 pl-12">
        <button onClick={() => onTaskAction(item, 'stt')} disabled={sttStatus === QueueTaskStatus.Queued || sttStatus === QueueTaskStatus.Processing} className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskButtonClass(sttStatus, theme)}`}>STT</button>
        <button onClick={() => onTaskAction(item, 'embedding')} disabled={embStatus === QueueTaskStatus.Queued || embStatus === QueueTaskStatus.Processing} className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskButtonClass(embStatus, theme)}`}>색인</button>
        <button onClick={() => onTaskAction(item, 'summary')} disabled={sumStatus === QueueTaskStatus.Queued || sumStatus === QueueTaskStatus.Processing} className={`text-xs px-3 py-1 rounded-lg border transition-colors ${getTaskButtonClass(sumStatus, theme)}`}>요약</button>
        <a href={api.getDownloadUrl(item.file_path || item.id)} download className={`text-xs px-3 py-1 rounded-lg border transition-colors ${theme === 'dark' ? 'border-slate-600 text-slate-400 hover:border-green-500 hover:bg-green-500/10' : 'border-slate-400 text-slate-600 hover:border-green-500 hover:bg-green-50'}`}><Download className="size-3 inline mr-1" /> 다운로드</a>
      </div>

      <TaskActionControls
        canRetry={[sttStatus, embStatus, sumStatus].some((s) => s === QueueTaskStatus.Error || s === QueueTaskStatus.Cancelled)}
        onRetry={() => onRetryFailed(item, { stt: sttStatus, embedding: embStatus, summary: sumStatus })}
      />

    </div>
  );
}

export const HistoryListItem = memo(HistoryListItemComponent);
