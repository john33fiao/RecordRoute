import { Clock, Loader2, CheckCircle2, XCircle, FileAudio, FileText, X, AlertTriangle } from 'lucide-react';
import { Card } from './ui/card';
import { Badge } from './ui/badge';
import { Progress } from './ui/progress';
import { Button } from './ui/button';
import { TaskActionControls } from './TaskActionControls';
import { useTheme } from '../contexts/ThemeContext';
import { useApp } from '../contexts/AppContext';
import type { QueueTask } from '../api/types';
import { QueueTaskStatus } from '../api/types';

const STAGE_ORDER: Array<'upload' | 'transform' | 'correct' | 'summary'> = ['upload', 'transform', 'correct', 'summary'];
const STAGE_LABEL: Record<(typeof STAGE_ORDER)[number], string> = {
  upload: '업로드',
  transform: '변환',
  correct: '교정',
  summary: '요약',
};

export function JobQueue() {
  const { theme } = useTheme();
  const { queue, currentTask, sortMode, setSortMode, removeTask, retryTask, cancelAll, categoryLabels } = useApp();

  const allTasks = currentTask ? [currentTask, ...queue] : queue;
  const activeTasks = allTasks.filter(t => t.status === QueueTaskStatus.Processing || t.status === QueueTaskStatus.Pending || t.status === QueueTaskStatus.Queued);

  const getStatusIcon = (status: QueueTask['status']) => {
    switch (status) {
      case QueueTaskStatus.Pending:
      case QueueTaskStatus.Queued:
        return <Clock className="size-4 text-slate-400" />;
      case QueueTaskStatus.Processing:
        return <Loader2 className="size-4 text-violet-400 animate-spin" />;
      case QueueTaskStatus.Completed:
        return <CheckCircle2 className="size-4 text-green-400" />;
      case QueueTaskStatus.Cancelled:
      case QueueTaskStatus.Error:
        return <XCircle className="size-4 text-red-400" />;
    }
  };

  const getStatusBadge = (status: QueueTask['status']) => {
    const config: Record<string, { className: string; label: string }> = {
      [QueueTaskStatus.Pending]: { className: 'bg-slate-700 text-slate-300', label: '대기중' },
      [QueueTaskStatus.Queued]: { className: 'bg-slate-700 text-slate-300', label: '대기중' },
      [QueueTaskStatus.Processing]: { className: 'bg-violet-500/20 text-violet-300 border-violet-500', label: '처리중' },
      [QueueTaskStatus.Completed]: { className: 'bg-green-500/20 text-green-300 border-green-500', label: '완료' },
      [QueueTaskStatus.Error]: { className: 'bg-red-500/20 text-red-300 border-red-500', label: '오류' },
      [QueueTaskStatus.Cancelled]: { className: 'bg-red-500/20 text-red-300 border-red-500', label: '취소됨' },
    };
    const c = config[status] || config.pending;
    return <Badge variant="outline" className={`${c.className} border`}>{c.label}</Badge>;
  };

  const isAudioTask = (task: QueueTask) => ['.mp3', '.wav', '.m4a', '.flac', '.ogg', '.webm', '.mp4'].some(ext => task.filePath.toLowerCase().endsWith(ext));

  const stepLabels: Record<string, string> = { stt: '변환', correct: '교정', embedding: '변환', summary: '요약', summarize: '요약' };



  const formatEta = (seconds?: number | null) => {
    if (seconds === undefined || seconds === null || Number.isNaN(seconds)) return null;
    if (seconds <= 0) return '곧 완료';
    const min = Math.floor(seconds / 60);
    const sec = seconds % 60;
    return min > 0 ? `약 ${min}분 ${sec}초` : `약 ${sec}초`;
  };

  const getCurrentStage = (task: QueueTask): (typeof STAGE_ORDER)[number] => {
    if (task.stage) return task.stage;
    if (task.status === QueueTaskStatus.Pending || task.status === QueueTaskStatus.Queued) return 'upload';
    if (task.failedStep === 'correct') return 'correct';
    if (task.failedStep === 'summary') return 'summary';
    return 'transform';
  };

  return (
    <div className="space-y-6">
      <Card className={`backdrop-blur-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'}`}>
        <div className="p-6 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>작업 큐</h2>
            <div className="flex items-center gap-2">
              <select value={sortMode} onChange={(e) => setSortMode(e.target.value as 'category' | 'order')}
                className={`text-sm rounded-lg border px-2 py-1 ${theme === 'dark' ? 'bg-slate-800 border-slate-700 text-slate-200' : 'bg-white border-slate-300 text-slate-900'}`}>
                <option value="category">카테고리별</option>
                <option value="order">추가순</option>
              </select>
              {allTasks.length > 0 && (
                <Button variant="outline" size="sm" onClick={cancelAll} className="text-red-400 border-red-500/50 hover:bg-red-500/10">모두 취소</Button>
              )}
              <Badge variant="outline" className="border-violet-500 text-violet-400">{activeTasks.length}개</Badge>
            </div>
          </div>

          {allTasks.length === 0 ? (
            <div className="text-center py-12"><div className={`inline-block p-4 rounded-full mb-4 ${theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'}`}><Clock className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} /></div><p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>진행중인 작업이 없습니다</p></div>
          ) : (
            <div className="space-y-3">
              {allTasks.map(task => {
                const FileIcon = isAudioTask(task) ? FileAudio : FileText;
                const fileName = task.filePath.split('/').pop() || task.filePath;
                return (
                  <div key={task.id} className={`p-4 rounded-xl border space-y-3 ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700' : 'bg-slate-50 border-slate-200'}`}>
                    <div className="flex items-start gap-3">
                      <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600"><FileIcon className="size-4 text-white" /></div>
                      <div className="flex-1 min-w-0 space-y-2">
                        <div className="flex items-start justify-between gap-2">
                          <p className={`font-medium truncate ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>{fileName.normalize('NFC')}</p>
                          <div className="flex items-center gap-2 flex-shrink-0">
                            <Badge variant="outline" className={theme === 'dark' ? 'border-slate-600 text-slate-300 bg-slate-800/50' : 'border-slate-400 text-slate-700 bg-slate-100'}>{categoryLabels[task.taskType]}</Badge>
                            {getStatusBadge(task.status)}
                            {task.status !== QueueTaskStatus.Completed && task.status !== QueueTaskStatus.Processing && (
                              <button onClick={() => removeTask(task.id)} className="p-1 rounded hover:bg-red-500/20 transition-colors"><X className="size-3 text-red-400" /></button>
                            )}
                          </div>
                        </div>

                        <div className="flex items-center gap-2 text-sm">
                          {getStatusIcon(task.status)}
                          <span className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>{task.progress || '대기 중...'}</span>
                          {(task.status === QueueTaskStatus.Processing || task.status === QueueTaskStatus.Queued) && (task.progressPercent !== undefined || task.etaSeconds !== undefined) && (
                            <span className={theme === 'dark' ? 'text-xs text-slate-500' : 'text-xs text-slate-500'}>{task.progressPercent !== undefined ? `${task.progressPercent}%` : ''}{task.etaSeconds !== undefined && formatEta(task.etaSeconds) ? ` · ETA ${formatEta(task.etaSeconds)}` : ''}</span>
                          )}
                        </div>
                        {task.status === QueueTaskStatus.Error && (
                          <div className={`rounded-lg border p-2 text-xs space-y-1 ${theme === 'dark' ? 'border-red-500/40 bg-red-950/30 text-red-200' : 'border-red-300 bg-red-50 text-red-700'}`}>
                            <div className="flex items-center gap-1 font-medium"><AlertTriangle className="size-3.5" />오류 정보</div>
                            <div>{task.error?.message || task.progress || '오류 발생'}</div>
                            <div>
                              실패 단계: {task.error?.failed_step ? (stepLabels[task.error.failed_step] || task.error.failed_step) : (task.failedStep ? (stepLabels[task.failedStep] || task.failedStep) : '-')}
                              {(task.error?.code || task.errorCode) ? ` · 코드: ${task.error?.code || task.errorCode}` : ''}
                              {task.retryable === false || task.error?.retryable === false ? ' · 재시도 불가' : ''}
                            </div>
                          </div>
                        )}

                        <div className="space-y-2">
                          <div className="grid grid-cols-4 gap-2">
                            {STAGE_ORDER.map((stage, idx) => {
                              const current = getCurrentStage(task);
                              const currentIndex = STAGE_ORDER.indexOf(current);
                              const complete = task.status === QueueTaskStatus.Completed || idx < currentIndex;
                              const active = idx === currentIndex && task.status !== QueueTaskStatus.Completed;
                              return (
                                <div key={stage} className="space-y-1">
                                  <div className={`h-1.5 rounded ${complete ? 'bg-violet-500' : active ? 'bg-violet-300/80' : 'bg-slate-600/40'}`} />
                                  <div className={`text-[11px] ${active ? (theme === 'dark' ? 'text-violet-300' : 'text-violet-700') : (theme === 'dark' ? 'text-slate-500' : 'text-slate-500')}`}>{STAGE_LABEL[stage]}</div>
                                </div>
                              );
                            })}
                          </div>
                          {task.status === QueueTaskStatus.Processing && <Progress value={task.progressPercent ?? Math.max(20, (STAGE_ORDER.indexOf(getCurrentStage(task)) + 1) * 25)} className="h-1.5" />}
                        </div>
                        <TaskActionControls
                          compact
                          canCancel={task.status === QueueTaskStatus.Processing || task.status === QueueTaskStatus.Pending || task.status === QueueTaskStatus.Queued}
                          canRetry={task.status === QueueTaskStatus.Error && task.retryable !== false}
                          retryLabel={task.failedStep ? `${stepLabels[task.failedStep] || task.failedStep} 단계만 재시도` : "재시도"}
                          onCancel={() => removeTask(task.id)}
                          onRetry={() => retryTask(task)}
                        />
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </Card>
    </div>
  );
}
