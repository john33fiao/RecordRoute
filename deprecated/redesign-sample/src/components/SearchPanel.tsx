import { useState } from 'react';
import { Search as SearchIcon, FileText, Sparkles, TrendingUp } from 'lucide-react';
import { Card } from './ui/card';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { ScrollArea } from './ui/scroll-area';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs';
import { useTheme } from '../contexts/ThemeContext';

interface SearchResult {
  id: string;
  filename: string;
  excerpt: string;
  similarity: number;
  date: Date;
}

// Mock data
const mockSearchResults: SearchResult[] = [
  {
    id: '1',
    filename: 'RECORD20.MP3',
    excerpt: '프로젝트 일정에 대한 논의가 있었으며, 2월 말까지 베타 버전 출시를 목표로 합니다...',
    similarity: 0.92,
    date: new Date(2026, 0, 11),
  },
  {
    id: '2',
    filename: '260111 회의록.md',
    excerpt: '디자인 시스템 구축 및 컴포넌트 라이브러리 개발이 우선순위입니다...',
    similarity: 0.87,
    date: new Date(2026, 0, 11),
  },
  {
    id: '3',
    filename: '260105.mp3',
    excerpt: '클라이언트 피드백을 반영하여 UI/UX 개선 작업을 진행합니다...',
    similarity: 0.78,
    date: new Date(2026, 0, 5),
  },
];

const mockRecommendations: SearchResult[] = [
  {
    id: '4',
    filename: '260108 브레인스토밍.md',
    excerpt: '새로운 기능 아이디어 논의 및 기술 스택 검토...',
    similarity: 0.85,
    date: new Date(2026, 0, 8),
  },
  {
    id: '5',
    filename: '260103 킥오프.mp3',
    excerpt: '프로젝트 목표 설정 및 팀 역할 분담...',
    similarity: 0.81,
    date: new Date(2026, 0, 3),
  },
];

export function SearchPanel() {
  const [searchQuery, setSearchQuery] = useState('');
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<SearchResult[]>([]);

  const handleSearch = () => {
    setIsSearching(true);
    // Simulate search delay
    setTimeout(() => {
      setResults(mockSearchResults);
      setIsSearching(false);
    }, 500);
  };

  const formatDate = (date: Date) => {
    return date.toLocaleDateString('ko-KR', {
      year: 'numeric',
      month: '2-digit',
      day: '2-digit',
    });
  };

  const getSimilarityColor = (similarity: number) => {
    if (similarity >= 0.9) return 'text-green-400';
    if (similarity >= 0.8) return 'text-violet-400';
    return 'text-slate-400';
  };

  const { theme } = useTheme();

  return (
    <Card className={`backdrop-blur-sm ${
      theme === 'dark'
        ? 'border-slate-800 bg-slate-900/50'
        : 'border-slate-200 bg-white/50'
    }`}>
      <div className="p-6 space-y-6">
        <div className="space-y-2">
          <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
            문서 검색
          </h2>
          <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
            임베딩 기반 의미 검색으로 관련 문서를 찾아보세요
          </p>
        </div>

        {/* Search Input */}
        <div className="flex gap-2">
          <div className="relative flex-1">
            <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 size-5 text-slate-500" />
            <Input
              type="text"
              placeholder="검색어를 입력하세요..."
              value={searchQuery}
              onChange={(e) => setSearchQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
              className={`pl-10 ${
                theme === 'dark'
                  ? 'bg-slate-800/50 border-slate-700 text-slate-200 placeholder:text-slate-500'
                  : 'bg-slate-50 border-slate-300 text-slate-900 placeholder:text-slate-500'
              }`}
            />
          </div>
          <Button
            onClick={handleSearch}
            disabled={!searchQuery || isSearching}
            className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500 px-8"
          >
            {isSearching ? '검색중...' : '검색'}
          </Button>
        </div>

        <Tabs defaultValue="results" className="space-y-4">
          <TabsList className={theme === 'dark'
            ? 'bg-slate-800/50 border border-slate-700'
            : 'bg-slate-100/50 border border-slate-200'
          }>
            <TabsTrigger value="results">검색 결과</TabsTrigger>
            <TabsTrigger value="recommendations">추천 문서</TabsTrigger>
          </TabsList>

          <TabsContent value="results">
            <ScrollArea className="h-[600px] pr-4">
              {results.length === 0 ? (
                <div className="text-center py-12">
                  <div className={`inline-block p-4 rounded-full mb-4 ${
                    theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'
                  }`}>
                    <SearchIcon className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} />
                  </div>
                  <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>
                    검색어를 입력하여 문서를 찾아보세요
                  </p>
                  <p className={`text-sm mt-2 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-500'}`}>
                    자연어 질의를 통해 관련 문서를 찾을 수 있습니다
                  </p>
                </div>
              ) : (
                <div className="space-y-3">
                  {results.map(result => (
                    <div
                      key={result.id}
                      className={`p-4 rounded-xl border transition-all group cursor-pointer ${
                        theme === 'dark'
                          ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600'
                          : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                      }`}
                    >
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                          <FileText className="size-4 text-white" />
                        </div>
                        <div className="flex-1 min-w-0 space-y-2">
                          <div className="flex items-start justify-between gap-2">
                            <div>
                              <h3 className={`font-medium ${
                                theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                              }`}>
                                {result.filename}
                              </h3>
                              <p className={`text-xs mt-1 ${
                                theme === 'dark' ? 'text-slate-500' : 'text-slate-600'
                              }`}>
                                {formatDate(result.date)}
                              </p>
                            </div>
                            <Badge 
                              variant="outline" 
                              className={`${
                                theme === 'dark' ? 'border-slate-600' : 'border-slate-400'
                              } ${getSimilarityColor(result.similarity)}`}
                            >
                              {(result.similarity * 100).toFixed(0)}% 일치
                            </Badge>
                          </div>
                          <p className={`text-sm line-clamp-2 ${
                            theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
                          }`}>
                            {result.excerpt}
                          </p>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </ScrollArea>
          </TabsContent>

          <TabsContent value="recommendations">
            <ScrollArea className="h-[600px] pr-4">
              <div className="space-y-4">
                {/* Info Card */}
                <div className="p-4 rounded-xl bg-gradient-to-r from-violet-600/20 to-fuchsia-600/20 border border-violet-500/50">
                  <div className="flex items-start gap-3">
                    <Sparkles className="size-5 text-violet-400 mt-0.5" />
                    <div>
                      <h3 className="font-medium text-violet-300">AI 추천 문서</h3>
                      <p className={`text-sm mt-1 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-300'}`}>
                        최근 작업 내용을 기반으로 관련성이 높은 문서를 추천합니다
                      </p>
                    </div>
                  </div>
                </div>

                {/* Recommendations */}
                <div className="space-y-3">
                  {mockRecommendations.map(rec => (
                    <div
                      key={rec.id}
                      className={`p-4 rounded-xl border transition-all group cursor-pointer ${
                        theme === 'dark'
                          ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600'
                          : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                      }`}
                    >
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                          <FileText className="size-4 text-white" />
                        </div>
                        <div className="flex-1 min-w-0 space-y-2">
                          <div className="flex items-start justify-between gap-2">
                            <div>
                              <h3 className={`font-medium ${
                                theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                              }`}>
                                {rec.filename}
                              </h3>
                              <p className={`text-xs mt-1 ${
                                theme === 'dark' ? 'text-slate-500' : 'text-slate-600'
                              }`}>
                                {formatDate(rec.date)}
                              </p>
                            </div>
                            <div className="flex items-center gap-1 text-xs text-violet-400">
                              <TrendingUp className="size-3" />
                              추천
                            </div>
                          </div>
                          <p className={`text-sm line-clamp-2 ${
                            theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
                          }`}>
                            {rec.excerpt}
                          </p>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              </div>
            </ScrollArea>
          </TabsContent>
        </Tabs>
      </div>
    </Card>
  );
}