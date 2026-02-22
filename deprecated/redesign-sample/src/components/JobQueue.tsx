import { Clock, Loader2, CheckCircle2, XCircle, FileAudio, FileText } from 'lucide-react';
import { Card } from './ui/card';
import { Badge } from './ui/badge';
import { Progress } from './ui/progress';
import { ScrollArea } from './ui/scroll-area';
import { useTheme } from '../contexts/ThemeContext';

interface Job {
  id: string;
  filename: string;
  type: 'audio' | 'text';
  task: 'STT' | '교정' | '요약' | '임베딩';
  status: 'queued' | 'processing' | 'completed' | 'error';
  progress: number;
  message?: string;
  timestamp: Date;
}

// Mock data - in real app this would come from WebSocket
const mockJobs: Job[] = [
  {
    id: '1',
    filename: 'RECORD20.MP3',
    type: 'audio',
    task: 'STT',
    status: 'processing',
    progress: 65,
    message: '음성 변환 중...',
    timestamp: new Date(2026, 0, 11, 10, 30),
  },
  {
    id: '2',
    filename: '260111 회의록.md',
    type: 'text',
    task: '요약',
    status: 'queued',
    progress: 0,
    timestamp: new Date(2026, 0, 11, 10, 28),
  },
  {
    id: '3',
    filename: '260105.mp3',
    type: 'audio',
    task: 'STT',
    status: 'completed',
    progress: 100,
    message: '완료',
    timestamp: new Date(2026, 0, 11, 10, 20),
  },
];

export function JobQueue() {
  const getStatusIcon = (status: Job['status']) => {
    switch (status) {
      case 'queued':
        return <Clock className="size-4 text-slate-400" />;
      case 'processing':
        return <Loader2 className="size-4 text-violet-400 animate-spin" />;
      case 'completed':
        return <CheckCircle2 className="size-4 text-green-400" />;
      case 'error':
        return <XCircle className="size-4 text-red-400" />;
    }
  };

  const getStatusBadge = (status: Job['status']) => {
    const variants = {
      queued: 'bg-slate-700 text-slate-300',
      processing: 'bg-violet-500/20 text-violet-300 border-violet-500',
      completed: 'bg-green-500/20 text-green-300 border-green-500',
      error: 'bg-red-500/20 text-red-300 border-red-500',
    };
    
    const labels = {
      queued: '대기중',
      processing: '처리중',
      completed: '완료',
      error: '오류',
    };

    return (
      <Badge variant="outline" className={`${variants[status]} border`}>
        {labels[status]}
      </Badge>
    );
  };

  const getTaskBadge = (task: Job['task'], theme: 'light' | 'dark') => {
    return (
      <Badge variant="outline" className={
        theme === 'dark'
          ? 'border-slate-600 text-slate-300 bg-slate-800/50'
          : 'border-slate-400 text-slate-700 bg-slate-100'
      }>
        {task}
      </Badge>
    );
  };

  const formatTime = (date: Date) => {
    return date.toLocaleString('ko-KR', { 
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const queuedJobs = mockJobs.filter(j => j.status === 'queued' || j.status === 'processing');
  const recentJobs = mockJobs.filter(j => j.status === 'completed' || j.status === 'error').slice(0, 5);

  const { theme } = useTheme();

  return (
    <div className="space-y-6">
      {/* Active Jobs */}
      <Card className={`backdrop-blur-sm ${
        theme === 'dark'
          ? 'border-slate-800 bg-slate-900/50'
          : 'border-slate-200 bg-white/50'
      }`}>
        <div className="p-6 space-y-4">
          <div className="flex items-center justify-between">
            <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
              진행중인 작업
            </h2>
            <Badge variant="outline" className="border-violet-500 text-violet-400">
              {queuedJobs.length}개
            </Badge>
          </div>

          {queuedJobs.length === 0 ? (
            <div className="text-center py-12">
              <div className={`inline-block p-4 rounded-full mb-4 ${
                theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'
              }`}>
                <Clock className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} />
              </div>
              <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>
                진행중인 작업이 없습니다
              </p>
            </div>
          ) : (
            <div className="space-y-3">
              {queuedJobs.map(job => {
                const FileIcon = job.type === 'audio' ? FileAudio : FileText;
                return (
                  <div
                    key={job.id}
                    className={`p-4 rounded-xl border space-y-3 ${
                      theme === 'dark'
                        ? 'bg-slate-800/50 border-slate-700'
                        : 'bg-slate-50 border-slate-200'
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                        <FileIcon className="size-4 text-white" />
                      </div>
                      <div className="flex-1 min-w-0 space-y-2">
                        <div className="flex items-start justify-between gap-2">
                          <div className="min-w-0 flex-1">
                            <p className={`font-medium truncate ${
                              theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                            }`}>
                              {job.filename}
                            </p>
                            <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                              {formatTime(job.timestamp)}
                            </p>
                          </div>
                          <div className="flex items-center gap-2 flex-shrink-0">
                            {getTaskBadge(job.task, theme)}
                            {getStatusBadge(job.status)}
                          </div>
                        </div>
                        
                        {job.status === 'processing' && (
                          <div className="space-y-2">
                            <div className="flex items-center justify-between text-sm">
                              <span className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>
                                {job.message}
                              </span>
                              <span className="text-violet-400 font-medium">{job.progress}%</span>
                            </div>
                            <Progress value={job.progress} className="h-2" />
                          </div>
                        )}
                      </div>
                    </div>
                  </div>
                );
              })}
            </div>
          )}
        </div>
      </Card>

      {/* Recent Completed Jobs */}
      <Card className={`backdrop-blur-sm ${
        theme === 'dark'
          ? 'border-slate-800 bg-slate-900/50'
          : 'border-slate-200 bg-white/50'
      }`}>
        <div className="p-6 space-y-4">
          <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
            최근 완료된 작업
          </h2>

          {recentJobs.length === 0 ? (
            <div className="text-center py-12">
              <div className={`inline-block p-4 rounded-full mb-4 ${
                theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'
              }`}>
                <CheckCircle2 className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} />
              </div>
              <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>
                완료된 작업이 없습니다
              </p>
            </div>
          ) : (
            <ScrollArea className="h-[400px] pr-4">
              <div className="space-y-2">
                {recentJobs.map(job => {
                  const FileIcon = job.type === 'audio' ? FileAudio : FileText;
                  return (
                    <div
                      key={job.id}
                      className={`p-4 rounded-xl border transition-colors group ${
                        theme === 'dark'
                          ? 'bg-slate-800/30 border-slate-700/50 hover:border-slate-600'
                          : 'bg-slate-50/50 border-slate-200/50 hover:border-slate-300'
                      }`}
                    >
                      <div className="flex items-center gap-3">
                        {getStatusIcon(job.status)}
                        <div className={`p-2 rounded-lg ${
                          theme === 'dark' ? 'bg-slate-700' : 'bg-slate-200'
                        }`}>
                          <FileIcon className={`size-3 ${
                            theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
                          }`} />
                        </div>
                        <div className="flex-1 min-w-0">
                          <p className={`text-sm font-medium truncate ${
                            theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                          }`}>
                            {job.filename}
                          </p>
                          <p className={`text-xs ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>
                            {formatTime(job.timestamp)}
                          </p>
                        </div>
                        <div className="flex items-center gap-2">
                          {getTaskBadge(job.task, theme)}
                          {getStatusBadge(job.status)}
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            </ScrollArea>
          )}
        </div>
      </Card>
    </div>
  );
}