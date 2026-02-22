import { useState, useRef } from 'react';
import { Upload, FileAudio, FileText, X, Check } from 'lucide-react';
import { Card } from './ui/card';
import { Button } from './ui/button';
import { Checkbox } from './ui/checkbox';
import { Label } from './ui/label';
import { Badge } from './ui/badge';
import { useTheme } from '../contexts/ThemeContext';

export function UploadSection() {
  const { theme } = useTheme();
  const [isDragging, setIsDragging] = useState(false);
  const [selectedFiles, setSelectedFiles] = useState<File[]>([]);
  const [tasks, setTasks] = useState({
    stt: true,
    summary: false,
    embedding: false,
  });
  const fileInputRef = useRef<HTMLInputElement>(null);

  const handleDragOver = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(true);
  };

  const handleDragLeave = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
  };

  const handleDrop = (e: React.DragEvent) => {
    e.preventDefault();
    setIsDragging(false);
    
    const files = Array.from(e.dataTransfer.files);
    setSelectedFiles(prev => [...prev, ...files]);
  };

  const handleFileSelect = (e: React.ChangeEvent<HTMLInputElement>) => {
    if (e.target.files) {
      const files = Array.from(e.target.files);
      setSelectedFiles(prev => [...prev, ...files]);
    }
  };

  const removeFile = (index: number) => {
    setSelectedFiles(prev => prev.filter((_, i) => i !== index));
  };

  const getFileIcon = (filename: string) => {
    const audioExts = ['.mp3', '.wav', '.m4a', '.flac', '.ogg', '.webm'];
    const ext = filename.toLowerCase().slice(filename.lastIndexOf('.'));
    return audioExts.includes(ext) ? FileAudio : FileText;
  };

  const formatFileSize = (bytes: number) => {
    if (bytes < 1024) return bytes + ' B';
    if (bytes < 1024 * 1024) return (bytes / 1024).toFixed(1) + ' KB';
    return (bytes / (1024 * 1024)).toFixed(1) + ' MB';
  };

  const handleUpload = () => {
    // Upload logic would go here
    console.log('Uploading files:', selectedFiles);
    console.log('Selected tasks:', tasks);
    // Clear files after upload
    setSelectedFiles([]);
  };

  return (
    <div className="space-y-6">
      {/* Upload Area */}
      <Card className={`backdrop-blur-sm overflow-hidden ${
        theme === 'dark'
          ? 'border-slate-800 bg-slate-900/50'
          : 'border-slate-200 bg-white/50'
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

          {/* Drop Zone */}
          <div
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onDrop={handleDrop}
            onClick={() => fileInputRef.current?.click()}
            className={`
              relative border-2 border-dashed rounded-xl p-12 text-center cursor-pointer
              transition-all duration-300 group
              ${isDragging 
                ? 'border-violet-500 bg-violet-500/10 scale-[1.02]' 
                : theme === 'dark'
                  ? 'border-slate-700 hover:border-slate-600 hover:bg-slate-800/50'
                  : 'border-slate-300 hover:border-slate-400 hover:bg-slate-100/50'
              }
            `}
          >
            <div className="relative z-10 space-y-4">
              <div className="inline-block">
                <div className={`
                  p-4 rounded-full transition-all duration-300
                  ${isDragging 
                    ? 'bg-gradient-to-r from-violet-500 to-fuchsia-500' 
                    : theme === 'dark'
                      ? 'bg-slate-800 group-hover:bg-slate-700'
                      : 'bg-slate-100 group-hover:bg-slate-200'
                  }
                `}>
                  <Upload className={`size-8 ${
                    isDragging 
                      ? 'text-white animate-pulse' 
                      : theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
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
                    <div
                      key={index}
                      className={`flex items-center gap-3 p-3 rounded-lg border transition-colors group ${
                        theme === 'dark'
                          ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600'
                          : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                      }`}
                    >
                      <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                        <FileIcon className="size-4 text-white" />
                      </div>
                      <div className="flex-1 min-w-0">
                        <p className={`text-sm font-medium truncate ${
                          theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                        }`}>
                          {file.name}
                        </p>
                        <p className={`text-xs ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>
                          {formatFileSize(file.size)}
                        </p>
                      </div>
                      <button
                        onClick={() => removeFile(index)}
                        className="p-1 rounded-lg hover:bg-red-500/20 transition-colors opacity-0 group-hover:opacity-100"
                      >
                        <X className="size-4 text-red-400" />
                      </button>
                    </div>
                  );
                })}
              </div>
            </div>
          )}
        </div>
      </Card>

      {/* Task Selection */}
      <Card className={`backdrop-blur-sm ${
        theme === 'dark'
          ? 'border-slate-800 bg-slate-900/50'
          : 'border-slate-200 bg-white/50'
      }`}>
        <div className="p-6 space-y-4">
          <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-slate-100' : 'text-slate-900'}`}>
            처리 작업 선택
          </h2>
          
          <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
            {/* STT Task */}
            <div className={`
              p-4 rounded-xl border-2 transition-all cursor-pointer
              ${tasks.stt 
                ? 'border-violet-500 bg-violet-500/10' 
                : theme === 'dark'
                  ? 'border-slate-700 hover:border-slate-600 bg-slate-800/50'
                  : 'border-slate-300 hover:border-slate-400 bg-slate-50'
              }
            `} onClick={() => setTasks(prev => ({ ...prev, stt: !prev.stt }))}>
              <div className="flex items-start gap-3">
                <div className={`
                  mt-0.5 size-5 rounded border-2 flex items-center justify-center transition-all
                  ${tasks.stt 
                    ? 'border-violet-500 bg-violet-500' 
                    : theme === 'dark' ? 'border-slate-600' : 'border-slate-400'
                  }
                `}>
                  {tasks.stt && <Check className="size-3 text-white" />}
                </div>
                <div className="flex-1">
                  <h3 className={`font-medium ${theme === 'dark' ? 'text-slate-100' : 'text-slate-900'}`}>
                    음성→텍스트 (STT)
                  </h3>
                  <p className={`text-sm mt-1 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                    OpenAI Whisper로 음성을 텍스트로 변환
                  </p>
                </div>
              </div>
            </div>

            {/* Summary Task */}
            <div className={`
              p-4 rounded-xl border-2 transition-all cursor-pointer
              ${tasks.summary 
                ? 'border-violet-500 bg-violet-500/10' 
                : theme === 'dark'
                  ? 'border-slate-700 hover:border-slate-600 bg-slate-800/50'
                  : 'border-slate-300 hover:border-slate-400 bg-slate-50'
              }
            `} onClick={() => setTasks(prev => ({ ...prev, summary: !prev.summary }))}>
              <div className="flex items-start gap-3">
                <div className={`
                  mt-0.5 size-5 rounded border-2 flex items-center justify-center transition-all
                  ${tasks.summary 
                    ? 'border-violet-500 bg-violet-500' 
                    : theme === 'dark' ? 'border-slate-600' : 'border-slate-400'
                  }
                `}>
                  {tasks.summary && <Check className="size-3 text-white" />}
                </div>
                <div className="flex-1">
                  <h3 className={`font-medium ${theme === 'dark' ? 'text-slate-100' : 'text-slate-900'}`}>
                    회의록 요약
                  </h3>
                  <p className={`text-sm mt-1 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                    구조화된 회의록 형태로 요약
                  </p>
                </div>
              </div>
            </div>

            {/* Embedding Task */}
            <div className={`
              p-4 rounded-xl border-2 transition-all cursor-pointer
              ${tasks.embedding 
                ? 'border-violet-500 bg-violet-500/10' 
                : theme === 'dark'
                  ? 'border-slate-700 hover:border-slate-600 bg-slate-800/50'
                  : 'border-slate-300 hover:border-slate-400 bg-slate-50'
              }
            `} onClick={() => setTasks(prev => ({ ...prev, embedding: !prev.embedding }))}>
              <div className="flex items-start gap-3">
                <div className={`
                  mt-0.5 size-5 rounded border-2 flex items-center justify-center transition-all
                  ${tasks.embedding 
                    ? 'border-violet-500 bg-violet-500' 
                    : theme === 'dark' ? 'border-slate-600' : 'border-slate-400'
                  }
                `}>
                  {tasks.embedding && <Check className="size-3 text-white" />}
                </div>
                <div className="flex-1">
                  <h3 className={`font-medium ${theme === 'dark' ? 'text-slate-100' : 'text-slate-900'}`}>
                    문서 임베딩
                  </h3>
                  <p className={`text-sm mt-1 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                    벡터화하여 검색 가능하도록 처리
                  </p>
                </div>
              </div>
            </div>
          </div>

          {/* Upload Button */}
          <div className="pt-4">
            <Button
              onClick={handleUpload}
              disabled={selectedFiles.length === 0}
              className="w-full bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500 text-white font-medium py-6 disabled:opacity-50 disabled:cursor-not-allowed"
            >
              <Upload className="size-5 mr-2" />
              업로드 및 처리 시작
            </Button>
          </div>
        </div>
      </Card>
    </div>
  );
}