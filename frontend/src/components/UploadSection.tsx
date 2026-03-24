import { useState, useRef, useCallback } from 'react';
import { Upload, FileAudio, FileText, X } from 'lucide-react';
import { Card } from './ui/card';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { useTheme } from '../contexts/ThemeContext';
import { useApp } from '../contexts/AppContext';
import * as api from '../api/client';

const AUDIO_EXTENSIONS = ['.flac', '.m4a', '.mp3', '.mp4', '.mpeg', '.mpga', '.oga', '.ogg', '.qta', '.wav', '.webm'];
const ACCEPTED_EXTENSIONS = [...AUDIO_EXTENSIONS, '.pdf', '.md', '.txt', '.text', '.markdown'];

function isAudioFile(name: string): boolean {
  const lower = name.toLowerCase();
  return AUDIO_EXTENSIONS.some(ext => lower.endsWith(ext));
}

function formatFileSize(bytes: number): string {
  if (bytes < 1024) return bytes + ' B';
  if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
  return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
}

export function UploadSection() {
  const { theme } = useTheme();
  const { addTask, loadHistory } = useApp();
  const [isDragging, setIsDragging] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [uploading, setUploading] = useState(false);
  const [statusMessage, setStatusMessage] = useState<{ text: string; type: 'success' | 'error' | 'info' | 'warning' } | null>(null);
  const fileInputRef = useRef<HTMLInputElement>(null);

  const showStatus = useCallback((text: string, type: 'success' | 'error' | 'info' | 'warning' = 'info', duration = 3000) => {
    setStatusMessage({ text, type });
    if (duration > 0) {
      setTimeout(() => setStatusMessage(null), duration);
    }
  }, []);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    if (!e.currentTarget.contains(e.relatedTarget as Node)) {
      setIsDragging(false);
    }
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    const files = Array.from(e.dataTransfer.files);
    addFiles(files);
  };

  const addFiles = (newFiles: File[]) => {
    const existingKeys = new Set(selectedFiles.map(f => `${f.name}-${f.size}-${f.lastModified}`));
    const accepted: File[] = [];
    const duplicates: string[] = [];
    const rejected: string[] = [];

    newFiles.forEach(file => {
      const ext = '.' + file.name.split('.').pop()?.toLowerCase();
      if (!ACCEPTED_EXTENSIONS.includes(ext) && !file.type.startsWith('audio/')) {
        rejected.push(file.name);
        return;
      }
      const key = `${file.name}-${file.size}-${file.lastModified}`;
      if (existingKeys.has(key)) {
        duplicates.push(file.name);
        return;
      }
      existingKeys.add(key);
      accepted.push(file);
    });

    if (accepted.length > 0) {
      setSelectedFiles(prev => [...prev, ...accepted]);
    }
    if (duplicates.length > 0) {
      showStatus(`이미 추가된 파일 제외: ${duplicates.join(', ')}`, 'info');
    }
    if (rejected.length > 0) {
      showStatus(`지원되지 않는 형식 제외: ${rejected.join(', ')}`, 'warning');
    }
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      addFiles(Array.from(e.target.files));
      e.target.value = '';
    }
  };

  const removeFile = (index: number) => {
    setSelectedFiles(prev => prev.filter((_, i) => i !== index));
  };

  const handleUpload = async () => {
    if (selectedFiles.length === 0) return;
    setUploading(true);
    showStatus(`${selectedFiles.length}개 파일 업로드 중...`, 'info', 0);

    try {
      const results = await api.uploadFiles(selectedFiles);
      const uploaded = results.filter(r => !r.duplicate);
      const duplicates = results.filter(r => r.duplicate);

      if (uploaded.length > 0) {
        showStatus(`${uploaded.length}개 파일 업로드 완료`, 'success');
        // Auto-add tasks
        uploaded.forEach(result => {
          const isAudio = isAudioFile(result.file_path);
          if (isAudio) {
            addTask(result.record_id, result.file_path, 'stt');
          } else {
            addTask(result.record_id, result.file_path, 'embedding');
          }
        });
        await loadHistory();
      }

      if (duplicates.length > 0) {
        showStatus(`${duplicates.length}개 중복 파일 제외`, 'info');
      }

      setSelectedFiles([]);
    } catch (e) {
      showStatus('업로드 실패', 'error');
    } finally {
      setUploading(false);
    }
  };

  const getFileIcon = (filename: string) => isAudioFile(filename) ? FileAudio : FileText;

  return (
    <div className="space-y-6">
      <Card className={`backdrop-blur-sm overflow-hidden ${
        theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'
      }`}>
        <div className="p-6 space-y-6">
          <div className="flex items-center justify-between">
            <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
              파일 업로드
            </h2>
            <Badge variant="outline" className="border-violet-500 text-violet-400">
              {selectedFiles.length}개 선택됨
            </Badge>
          </div>

          {/* Status Message */}
          {statusMessage && (
            <div className={`p-3 rounded-lg text-sm ${
              statusMessage.type === 'success' ? 'bg-green-500/20 text-green-300 border border-green-500/30' :
              statusMessage.type === 'error' ? 'bg-red-500/20 text-red-300 border border-red-500/30' :
              statusMessage.type === 'warning' ? 'bg-yellow-500/20 text-yellow-300 border border-yellow-500/30' :
              'bg-blue-500/20 text-blue-300 border border-blue-500/30'
            }`}>
              {statusMessage.text}
            </div>
          )}

          {/* Drop Zone */}
          <div
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
            onClick={() => fileInputRef.current?.click()}
            className={`relative border-2 border-dashed rounded-xl p-12 text-center cursor-pointer transition-all duration-300 group ${
              isDragging
                ? 'border-violet-500 bg-violet-500/10 scale-[1.02]'
                : theme === 'dark'
                  ? 'border-slate-700 hover:border-slate-600 hover:bg-slate-800/50'
                  : 'border-slate-300 hover:border-slate-400 hover:bg-slate-100/50'
            }`}
          >
            <div className="relative z-10 space-y-4">
              <div className="inline-block">
                <div className={`p-4 rounded-full transition-all duration-300 ${
                  isDragging
                    ? 'bg-gradient-to-r from-violet-500 to-fuchsia-500'
                    : theme === 'dark'
                      ? 'bg-slate-800 group-hover:bg-slate-700'
                      : 'bg-slate-100 group-hover:bg-slate-200'
                }`}>
                  <Upload className={`size-8 ${
                    isDragging ? 'text-white animate-pulse' : theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
                  }`} />
                </div>
              </div>
              <div>
                <p className={`text-lg font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>
                  파일을 드래그하거나 클릭하여 업로드
                </p>
                <p className={`text-sm mt-2 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                  지원 포맷: .flac, .m4a, .mp3, .mp4, .wav, .webm, .ogg, .md, .txt, .pdf
                </p>
              </div>
            </div>
            <input
              ref={fileInputRef}
              type="file"
              multiple
              className="hidden"
              onChange={handleFileSelect}
              accept=".flac,.m4a,.mp3,.mp4,.mpeg,.mpga,.oga,.ogg,.qta,.wav,.webm,.md,.txt,.text,.markdown,.pdf"
            />
          </div>

          {/* Selected Files */}
          {selectedFiles.length > 0 && (
            <div className="space-y-2">
              <h3 className={`text-sm font-medium ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                선택된 파일
              </h3>
              <div className="space-y-2 max-h-60 overflow-y-auto">
                {selectedFiles.map((file, index) => {
                  const FileIcon = getFileIcon(file.name);
                  return (
                    <div key={`${file.name}-${index}`}
                      className={`flex items-center gap-3 p-3 rounded-lg border transition-colors group ${
                        theme === 'dark'
                          ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600'
                          : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                      }`}>
                      <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                        <FileIcon className="size-4 text-white" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className={`text-sm font-medium truncate ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>
                          {file.name}
                        </p>
                        <p className={`text-xs ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>
                          {formatFileSize(file.size)}
                        </p>
                      </div>
                      <button onClick={(e) => { e.stopPropagation(); removeFile(index); }}
                        className="p-1 rounded-lg hover:bg-red-500/20 transition-colors opacity-0 group-hover:opacity-100">
                        <X className="size-4 text-red-400" />
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {/* Upload Button */}
          <Button
            onClick={handleUpload}
            disabled={selectedFiles.length === 0 || uploading}
            className="w-full bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500 text-white font-medium py-6 disabled:opacity-50 disabled:cursor-not-allowed"
          >
            <Upload className="size-5 mr-2" />
            {uploading ? '업로드 중...' : '업로드 및 처리 시작'}
          </Button>
        </div>
      </Card>
    </div>
  );
}
