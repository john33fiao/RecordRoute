import { useRef, useState } from 'react';
import {
  CheckCircle2,
  FileAudio,
  Loader2,
  TriangleAlert,
  Upload,
  X,
} from 'lucide-react';
import { Card } from './ui/card';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { Alert, AlertDescription, AlertTitle } from './ui/alert';
import { Progress } from './ui/progress';
import { useTheme } from '../contexts/ThemeContext';

const AUDIO_FILE_EXTENSIONS = [
  '.wav',
  '.mp3',
  '.flac',
  '.ogg',
  '.oga',
  '.m4a',
  '.aac',
  '.opus',
  '.webm',
  '.wma',
  '.aiff',
  '.aif',
  '.m4b',
  '.mpeg',
  '.mpga',
  '.qta',
] as const;

const FILE_ACCEPT = `audio/*,${AUDIO_FILE_EXTENSIONS.join(',')}`;
const MAX_UPLOAD_BYTES = 512 * 1024 * 1024;
const MAX_UPLOAD_LABEL = '512MB';

type UploadMessageTone = 'info' | 'success' | 'error';

interface UploadMessage {
  tone: UploadMessageTone;
  title: string;
  description: string;
}

interface UploadJobResponse {
  job_id: string;
  message: string;
  source_file_name: string;
  reused: boolean;
  deduplicated: boolean;
}

interface UploadProgressState {
  completed: number;
  total: number;
}

function isAudioFile(file: File) {
  if (typeof file.type === 'string' && file.type.startsWith('audio/')) {
    return true;
  }

  const lowerName = file.name.toLowerCase();
  return AUDIO_FILE_EXTENSIONS.some((extension) => lowerName.endsWith(extension));
}

function tryParseJson(text: string) {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

async function uploadFile(file: File) {
  const formData = new FormData();
  formData.append('file', file);

  const response = await fetch('/jobs/upload', {
    method: 'POST',
    body: formData,
  });
  const text = await response.text();
  const payload = text ? tryParseJson(text) : null;

  if (!response.ok) {
    const message =
      payload && typeof payload === 'object' && 'message' in payload
        ? String(payload.message)
        : `${response.status} ${response.statusText}`;
    throw new Error(message);
  }

  return payload as UploadJobResponse;
}

export function UploadSection() {
  const { theme } = useTheme();
  const [isDragging, setIsDragging] = useState(false);
  const [isUploading, setIsUploading] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [uploadMessage, setUploadMessage] = useState<UploadMessage | null>(null);
  const [uploadProgress, setUploadProgress] = useState<UploadProgressState>({
    completed: 0,
    total: 0,
  });
  const fileInputRef = useRef<HTMLInputElement>(null);

  const applySelectedFiles = (files: File[], source: string) => {
    const accepted: File[] = [];
    const rejectedNonAudio: string[] = [];
    const rejectedOversized: string[] = [];

    for (const file of files) {
      if (!isAudioFile(file)) {
        rejectedNonAudio.push(file.name);
        continue;
      }

      if (file.size > MAX_UPLOAD_BYTES) {
        rejectedOversized.push(file.name);
        continue;
      }

      accepted.push(file);
    }

    if (accepted.length > 0) {
      setSelectedFiles((prev) => [...prev, ...accepted]);
    }

    if (accepted.length === 0) {
      const reasons = [];
      if (rejectedNonAudio.length > 0) {
        reasons.push('오디오 파일이 아닙니다');
      }
      if (rejectedOversized.length > 0) {
        reasons.push(`${MAX_UPLOAD_LABEL}를 초과했습니다`);
      }

      setUploadMessage({
        tone: 'error',
        title: `${source}한 파일을 추가하지 못했습니다.`,
        description: reasons.length > 0 ? reasons.join(' ') : '업로드 가능한 오디오 파일이 없습니다.',
      });
      return;
    }

    const messageParts = [`${source}한 파일 ${accepted.length}개를 업로드 목록에 추가했습니다.`];
    if (rejectedNonAudio.length > 0) {
      messageParts.push(`오디오 아님 ${rejectedNonAudio.length}개 제외`);
    }
    if (rejectedOversized.length > 0) {
      messageParts.push(`${MAX_UPLOAD_LABEL} 초과 ${rejectedOversized.length}개 제외`);
    }

    setUploadMessage({
      tone: rejectedNonAudio.length > 0 || rejectedOversized.length > 0 ? 'info' : 'success',
      title: '오디오 파일이 준비되었습니다.',
      description: messageParts.join(' '),
    });
  };

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    if (!isUploading) {
      setIsDragging(true);
    }
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);

    if (isUploading) {
      return;
    }

    applySelectedFiles(Array.from(e.dataTransfer.files), '드래그');
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (!e.target.files || isUploading) {
      return;
    }

    applySelectedFiles(Array.from(e.target.files), '선택');
    e.target.value = '';
  };

  const removeFile = (index: number) => {
    if (isUploading) {
      return;
    }

    setSelectedFiles((prev) => prev.filter((_, i) => i !== index));
  };

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
    return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  };

  const handleUpload = async () => {
    if (selectedFiles.length === 0 || isUploading) {
      setUploadMessage({
        tone: 'error',
        title: '업로드할 파일이 없습니다.',
        description: '오디오 파일을 먼저 선택해 주세요.',
      });
      return;
    }

    setIsUploading(true);
    setUploadProgress({ completed: 0, total: selectedFiles.length });
    setUploadMessage({
      tone: 'info',
      title: '업로드를 시작합니다.',
      description: `${selectedFiles.length}개 파일을 순차적으로 전송하고 있습니다.`,
    });

    let successCount = 0;
    let failedCount = 0;
    let reusedCount = 0;
    let deduplicatedCount = 0;
    const failureMessages: string[] = [];
    let lastSuccessfulUpload: UploadJobResponse | null = null;

    try {
      for (const [index, file] of selectedFiles.entries()) {
        try {
          const response = await uploadFile(file);
          successCount += 1;
          lastSuccessfulUpload = response;
          if (response.reused) {
            reusedCount += 1;
          }
          if (response.deduplicated) {
            deduplicatedCount += 1;
          }
        } catch (error) {
          failedCount += 1;
          const message = error instanceof Error ? error.message : '업로드 중 알 수 없는 오류가 발생했습니다.';
          failureMessages.push(`${file.name}: ${message}`);
        } finally {
          setUploadProgress({
            completed: index + 1,
            total: selectedFiles.length,
          });
        }
      }

      const resultParts = [];
      if (successCount > 0) {
        resultParts.push(`성공 ${successCount}개`);
      }
      if (failedCount > 0) {
        resultParts.push(`실패 ${failedCount}개`);
      }
      if (reusedCount > 0) {
        resultParts.push(`재사용 ${reusedCount}개`);
      }
      if (deduplicatedCount > 0) {
        resultParts.push(`기존 실행 합류 ${deduplicatedCount}개`);
      }

      const failureSummary =
        failureMessages.length > 0
          ? ` 실패 상세: ${failureMessages.slice(0, 2).join(' / ')}${
              failureMessages.length > 2 ? ' ...' : ''
            }`
          : '';
      const successSummary =
        successCount === 1 && failedCount === 0 && lastSuccessfulUpload
          ? ` ${lastSuccessfulUpload.source_file_name} · ${lastSuccessfulUpload.message} · job ${lastSuccessfulUpload.job_id}`
          : '';

      setUploadMessage({
        tone: failedCount > 0 ? 'info' : 'success',
        title: failedCount > 0 ? '업로드가 부분 완료되었습니다.' : '업로드 요청이 접수되었습니다.',
        description: `${resultParts.join(' · ')}.${successSummary}${failureSummary}`.trim(),
      });
      setSelectedFiles([]);
    } finally {
      setIsUploading(false);
      setIsDragging(false);
      setUploadProgress({ completed: 0, total: 0 });
    }
  };

  const uploadMessageClass =
    uploadMessage?.tone === 'error'
      ? theme === 'dark'
        ? 'border-red-500/30 bg-red-500/10 text-red-100'
        : 'border-red-300 bg-red-50 text-red-900'
      : uploadMessage?.tone === 'success'
        ? theme === 'dark'
          ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-100'
          : 'border-emerald-300 bg-emerald-50 text-emerald-900'
        : theme === 'dark'
          ? 'border-violet-500/30 bg-violet-500/10 text-slate-100'
          : 'border-violet-300 bg-violet-50 text-slate-900';

  const uploadMessageIconClass =
    uploadMessage?.tone === 'error'
      ? 'text-red-400'
      : uploadMessage?.tone === 'success'
        ? 'text-emerald-400'
        : 'text-violet-400';

  return (
    <div className="space-y-6">
      <Card
        className={`backdrop-blur-sm overflow-hidden ${
          theme === 'dark'
            ? 'border-slate-800 bg-slate-900/50'
            : 'border-slate-200 bg-white/50'
        }`}
      >
        <div className="p-6 space-y-6">
          <div className="flex flex-wrap items-center justify-between gap-3">
            <div className="space-y-2">
              <h2
                className={`text-xl font-semibold ${
                  theme === 'dark' ? 'text-white' : 'text-slate-900'
                }`}
              >
                파일 업로드
              </h2>
              <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                현재 업로드 패널은 오디오 파일만 받아 새 ffmpeg job을 생성합니다.
              </p>
            </div>
            <div className="flex items-center gap-2">
              <Badge variant="outline" className="border-violet-500 text-violet-400">
                {selectedFiles.length}개 선택됨
              </Badge>
              <Badge
                variant="outline"
                className={
                  theme === 'dark'
                    ? 'border-slate-700 text-slate-300'
                    : 'border-slate-300 text-slate-700'
                }
              >
                최대 {MAX_UPLOAD_LABEL}
              </Badge>
            </div>
          </div>

          <div
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
            onClick={() => !isUploading && fileInputRef.current?.click()}
            className={`
              relative border-2 border-dashed rounded-xl p-12 text-center transition-all duration-300 group
              ${isUploading ? 'cursor-not-allowed opacity-80' : 'cursor-pointer'}
              ${
                isDragging
                  ? 'border-violet-500 bg-violet-500/10 scale-[1.02]'
                  : theme === 'dark'
                    ? 'border-slate-700 hover:border-slate-600 hover:bg-slate-800/50'
                    : 'border-slate-300 hover:border-slate-400 hover:bg-slate-100/50'
              }
            `}
          >
            <div className="relative z-10 space-y-4">
              <div className="inline-block">
                <div
                  className={`
                    p-4 rounded-full transition-all duration-300
                    ${
                      isDragging
                        ? 'bg-gradient-to-r from-violet-500 to-fuchsia-500'
                        : theme === 'dark'
                          ? 'bg-slate-800 group-hover:bg-slate-700'
                          : 'bg-slate-100 group-hover:bg-slate-200'
                    }
                  `}
                >
                  <Upload
                    className={`size-8 ${
                      isDragging
                        ? 'text-white animate-pulse'
                        : theme === 'dark'
                          ? 'text-slate-400'
                          : 'text-slate-600'
                    }`}
                  />
                </div>
              </div>
              <div>
                <p
                  className={`text-lg font-medium ${
                    theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                  }`}
                >
                  오디오 파일을 드래그하거나 클릭하여 업로드
                </p>
                <p className={`text-sm mt-2 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                  지원 포맷: {AUDIO_FILE_EXTENSIONS.join(', ')}
                </p>
              </div>
            </div>
            <input
              ref={fileInputRef}
              type="file"
              multiple
              disabled={isUploading}
              className="hidden"
              onChange={handleFileSelect}
              accept={FILE_ACCEPT}
            />
          </div>

          {uploadMessage && (
            <Alert className={uploadMessageClass}>
              {uploadMessage.tone === 'error' ? (
                <TriangleAlert className={uploadMessageIconClass} />
              ) : uploadMessage.tone === 'success' ? (
                <CheckCircle2 className={uploadMessageIconClass} />
              ) : (
                <Upload className={uploadMessageIconClass} />
              )}
              <AlertTitle>{uploadMessage.title}</AlertTitle>
              <AlertDescription>{uploadMessage.description}</AlertDescription>
            </Alert>
          )}

          {isUploading && uploadProgress.total > 0 && (
            <div
              className={`rounded-xl border p-4 space-y-3 ${
                theme === 'dark'
                  ? 'border-slate-700 bg-slate-800/60'
                  : 'border-slate-200 bg-slate-50'
              }`}
            >
              <div className="flex items-center justify-between gap-3">
                <div className="flex items-center gap-2">
                  <Loader2 className="size-4 animate-spin text-violet-400" />
                  <p className={theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}>
                    업로드 진행 중
                  </p>
                </div>
                <span className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                  {uploadProgress.completed}/{uploadProgress.total}
                </span>
              </div>
              <Progress
                value={(uploadProgress.completed / uploadProgress.total) * 100}
                className={theme === 'dark' ? 'bg-slate-700' : 'bg-slate-200'}
              />
            </div>
          )}

          {selectedFiles.length > 0 && (
            <div className="space-y-2">
              <h3
                className={`text-sm font-medium ${
                  theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
                }`}
              >
                선택된 오디오 파일
              </h3>
              <div className="space-y-2 max-h-60 overflow-y-auto">
                {selectedFiles.map((file, index) => (
                  <div
                    key={`${file.name}-${file.size}-${file.lastModified}-${index}`}
                    className={`flex items-center gap-3 p-3 rounded-lg border transition-colors group ${
                      theme === 'dark'
                        ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600'
                        : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                    }`}
                  >
                    <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                      <FileAudio className="size-4 text-white" />
                    </div>
                    <div className="flex-1 min-w-0">
                      <p
                        className={`text-sm font-medium truncate ${
                          theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                        }`}
                      >
                        {file.name}
                      </p>
                      <p className={`text-xs ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>
                        {formatFileSize(file.size)}
                      </p>
                    </div>
                    <button
                      type="button"
                      disabled={isUploading}
                      onClick={() => removeFile(index)}
                      className="p-1 rounded-lg hover:bg-red-500/20 transition-colors opacity-0 group-hover:opacity-100 disabled:opacity-40 disabled:cursor-not-allowed"
                    >
                      <X className="size-4 text-red-400" />
                    </button>
                  </div>
                ))}
              </div>
            </div>
          )}

          <div className="pt-2">
            <Button
              onClick={handleUpload}
              disabled={selectedFiles.length === 0 || isUploading}
              className="w-full bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500 text-white font-medium py-6 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              {isUploading ? (
                <Loader2 className="size-5 mr-2 animate-spin" />
              ) : (
                <Upload className="size-5 mr-2" />
              )}
              {isUploading ? '업로드 중...' : '오디오 업로드 시작'}
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}
