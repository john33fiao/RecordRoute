import { useState } from 'react';
import { FileAudio, FileText, Download, Eye, Trash2, Calendar, Settings2 } from 'lucide-react';
import { Card } from './ui/card';
import { Badge } from './ui/badge';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Input } from './ui/input';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from './ui/select';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogHeader,
  DialogTitle,
} from './ui/dialog';
import { useTheme } from '../contexts/ThemeContext';

interface HistoryItem {
  id: string;
  filename: string;
  type: 'audio' | 'text';
  originalFile: string;
  results: {
    stt?: string;
    corrected?: string;
    summary?: string;
  };
  uploadDate: Date;
  fileSize: string;
}

// Mock data
const mockHistory: HistoryItem[] = [
  {
    id: '1',
    filename: 'RECORD20.MP3',
    type: 'audio',
    originalFile: 'RECORD20.MP3',
    results: {
      stt: '회의 내용: 새로운 프로젝트 기획안에 대한 논의...',
      summary: '## 주요 주제\n- 프로젝트 일정\n- 예산 검토...',
    },
    uploadDate: new Date(2026, 0, 11, 10, 30),
    fileSize: '12.4 MB',
  },
  {
    id: '2',
    filename: '260111 회의록.md',
    type: 'text',
    originalFile: '260111 회의록.md',
    results: {
      corrected: '교정된 회의록 내용...',
      summary: '## 주요 결정사항\n- 디자인 최종 승인...',
    },
    uploadDate: new Date(2026, 0, 11, 9, 15),
    fileSize: '45.2 KB',
  },
  {
    id: '3',
    filename: '260105.mp3',
    type: 'audio',
    originalFile: '260105.mp3',
    results: {
      stt: '오늘 회의 안건은 다음과 같습니다...',
    },
    uploadDate: new Date(2026, 0, 5, 14, 20),
    fileSize: '8.7 MB',
  },
];

export function HistoryPanel() {
  const { theme } = useTheme();
  const [searchQuery, setSearchQuery] = useState('');
  const [selectedItem, setSelectedItem] = useState<HistoryItem | null>(null);
  const [viewContent, setViewContent] = useState<{ title: string; content: string } | null>(null);
  const [sortBy, setSortBy] = useState('category');

  const formatDate = (date: Date) => {
    return date.toLocaleString('ko-KR', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
      hour: '2-digit',
      minute: '2-digit',
    });
  };

  const filteredHistory = mockHistory.filter(item =>
    item.filename.toLowerCase().includes(searchQuery.toLowerCase())
  );

  // Sort history based on selected option
  const sortedHistory = [...filteredHistory].sort((a, b) => {
    if (sortBy === 'category') {
      // Sort by type first, then by date (newest first)
      if (a.type !== b.type) {
        return a.type.localeCompare(b.type);
      }
      return b.uploadDate.getTime() - a.uploadDate.getTime();
    } else if (sortBy === 'oldest') {
      // Sort by date (oldest first)
      return a.uploadDate.getTime() - b.uploadDate.getTime();
    }
    return 0;
  });

  const handleView = (item: HistoryItem, type: 'stt' | 'corrected' | 'summary') => {
    const titles = {
      stt: 'STT 결과',
      corrected: '교정된 텍스트',
      summary: '요약 결과',
    };
    
    setViewContent({
      title: `${item.filename} - ${titles[type]}`,
      content: item.results[type] || '',
    });
  };

  const handleDelete = (id: string) => {
    // Delete logic would go here
    console.log('Deleting item:', id);
  };

  return (
    <>
      <Card className={`backdrop-blur-sm ${
        theme === 'dark'
          ? 'border-slate-800 bg-slate-900/50'
          : 'border-slate-200 bg-white/50'
      }`}>
        <div className="p-6 space-y-4">
          <div className="flex items-center justify-between gap-4">
            <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
              업로드 기록
            </h2>
            <Badge variant="outline" className="border-violet-500 text-violet-400">
              {filteredHistory.length}개
            </Badge>
          </div>

          {/* Search */}
          <div className="relative">
            <Input
              type="text"
              placeholder="파일명으로 검색..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              className={theme === 'dark'
                ? 'bg-slate-800/50 border-slate-700 text-slate-200 placeholder:text-slate-500'
                : 'bg-slate-50 border-slate-300 text-slate-900 placeholder:text-slate-500'
              }
            />
          </div>

          {/* Sort */}
          <div className="flex items-center gap-2">
            <label className={`text-sm whitespace-nowrap ${
              theme === 'dark' ? 'text-slate-300' : 'text-slate-700'
            }`}>
              정렬:
            </label>
            <Select onValueChange={setSortBy} value={sortBy}>
              <SelectTrigger className={theme === 'dark'
                ? 'bg-slate-800/50 border-slate-700 text-slate-200'
                : 'bg-slate-50 border-slate-300 text-slate-900'
              }>
                <SelectValue />
              </SelectTrigger>
              <SelectContent className={theme === 'dark'
                ? 'bg-slate-800 border-slate-700'
                : 'bg-white border-slate-300'
              }>
                <SelectItem value="category" className={theme === 'dark'
                  ? 'text-slate-200 focus:bg-slate-700 focus:text-slate-100'
                  : 'text-slate-900 focus:bg-slate-100 focus:text-slate-900'
                }>
                  기본값 (카테고리별)
                </SelectItem>
                <SelectItem value="oldest" className={theme === 'dark'
                  ? 'text-slate-200 focus:bg-slate-700 focus:text-slate-100'
                  : 'text-slate-900 focus:bg-slate-100 focus:text-slate-900'
                }>
                  추가순 (오래된 순)
                </SelectItem>
              </SelectContent>
            </Select>
          </div>

          {/* History List */}
          <ScrollArea className="h-[600px] pr-4">
            {sortedHistory.length === 0 ? (
              <div className="text-center py-12">
                <div className={`inline-block p-4 rounded-full mb-4 ${
                  theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'
                }`}>
                  <Calendar className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} />
                </div>
                <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>
                  {searchQuery ? '검색 결과가 없습니다' : '업로드 기록이 없습니다'}
                </p>
              </div>
            ) : (
              <div className="space-y-3">
                {sortedHistory.map(item => {
                  const FileIcon = item.type === 'audio' ? FileAudio : FileText;
                  const resultCount = Object.keys(item.results).length;

                  return (
                    <div
                      key={item.id}
                      className={`p-4 rounded-xl border transition-all space-y-3 group ${
                        theme === 'dark'
                          ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600'
                          : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                      }`}
                    >
                      {/* Header */}
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                          <FileIcon className="size-4 text-white" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <p className={`font-medium truncate ${
                            theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                          }`}>
                            {item.filename}
                          </p>
                          <div className={`flex items-center gap-3 mt-1 text-xs ${
                            theme === 'dark' ? 'text-slate-500' : 'text-slate-600'
                          }`}>
                            <span>{formatDate(item.uploadDate)}</span>
                            <span>•</span>
                            <span>{item.fileSize}</span>
                            <span>•</span>
                            <span>{resultCount}개 결과</span>
                          </div>
                        </div>
                        <Button
                          variant="ghost"
                          size="sm"
                          onClick={() => handleDelete(item.id)}
                          className="opacity-0 group-hover:opacity-100 transition-opacity text-red-400 hover:text-red-300 hover:bg-red-500/20"
                        >
                          <Trash2 className="size-4" />
                        </Button>
                      </div>

                      {/* Results */}
                      <div className="flex flex-wrap gap-2">
                        {item.results.stt && (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleView(item, 'stt')}
                            className={theme === 'dark'
                              ? 'border-slate-600 hover:border-violet-500 hover:bg-violet-500/10 text-slate-300'
                              : 'border-slate-400 hover:border-violet-500 hover:bg-violet-500/10 text-slate-700'
                            }
                          >
                            <Eye className="size-3 mr-1" />
                            STT 결과
                          </Button>
                        )}
                        {item.results.corrected && (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleView(item, 'corrected')}
                            className={theme === 'dark'
                              ? 'border-slate-600 hover:border-violet-500 hover:bg-violet-500/10 text-slate-300'
                              : 'border-slate-400 hover:border-violet-500 hover:bg-violet-500/10 text-slate-700'
                            }
                          >
                            <Eye className="size-3 mr-1" />
                            교정 결과
                          </Button>
                        )}
                        {item.results.summary && (
                          <Button
                            variant="outline"
                            size="sm"
                            onClick={() => handleView(item, 'summary')}
                            className={theme === 'dark'
                              ? 'border-slate-600 hover:border-violet-500 hover:bg-violet-500/10 text-slate-300'
                              : 'border-slate-400 hover:border-violet-500 hover:bg-violet-500/10 text-slate-700'
                            }
                          >
                            <Eye className="size-3 mr-1" />
                            요약 결과
                          </Button>
                        )}
                        <Button
                          variant="outline"
                          size="sm"
                          className={theme === 'dark'
                            ? 'border-slate-600 hover:border-green-500 hover:bg-green-500/10 text-slate-300'
                            : 'border-slate-400 hover:border-green-500 hover:bg-green-500/10 text-slate-700'
                          }
                        >
                          <Download className="size-3 mr-1" />
                          다운로드
                        </Button>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </ScrollArea>
        </div>
      </Card>

      {/* View Content Dialog */}
      <Dialog open={viewContent !== null} onOpenChange={() => setViewContent(null)}>
        <DialogContent className={`max-w-3xl max-h-[80vh] ${
          theme === 'dark'
            ? 'bg-slate-900 border-slate-800 text-slate-100'
            : 'bg-white border-slate-200 text-slate-900'
        }`}>
          <DialogHeader>
            <DialogTitle className="text-xl">{viewContent?.title}</DialogTitle>
            <DialogDescription className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>
              결과를 확인하고 필요한 경우 다운로드하세요
            </DialogDescription>
          </DialogHeader>
          <ScrollArea className="h-[60vh] pr-4">
            <div className={`prose max-w-none ${
              theme === 'dark' ? 'prose-invert prose-slate' : 'prose-slate'
            }`}>
              <pre className={`whitespace-pre-wrap text-sm p-4 rounded-lg border ${
                theme === 'dark'
                  ? 'bg-slate-800/50 border-slate-700'
                  : 'bg-slate-50 border-slate-200'
              }`}>
                {viewContent?.content}
              </pre>
            </div>
          </ScrollArea>
          <div className="flex justify-end gap-2">
            <Button 
              variant="outline" 
              onClick={() => setViewContent(null)} 
              className={theme === 'dark' ? 'border-slate-700' : 'border-slate-300'}
            >
              닫기
            </Button>
            <Button className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500">
              <Download className="size-4 mr-2" />
              다운로드
            </Button>
          </div>
        </DialogContent>
      </Dialog>
    </>
  );
}