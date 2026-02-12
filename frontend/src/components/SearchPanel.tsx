import { useState } from 'react';
import { Search as SearchIcon, FileText, TrendingUp } from 'lucide-react';
import { Card } from './ui/card';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { ScrollArea } from './ui/scroll-area';
import { useTheme } from '../contexts/ThemeContext';
import * as api from '../api/client';
import type { KeywordMatch, SimilarDocument } from '../api/types';

export function SearchPanel() {
  const { theme } = useTheme();
  const [query, setQuery] = useState('');
  const [searching, setSearching] = useState(false);
  const [keywordResults, setKeywordResults] = useState<KeywordMatch[]>([]);
  const [similarResults, setSimilarResults] = useState<SimilarDocument[]>([]);
  const [hasSearched, setHasSearched] = useState(false);

  const handleSearch = async () => {
    if (!query.trim()) return;
    setSearching(true);
    try {
      const data = await api.search(query.trim());
      setKeywordResults(data.keywordMatches || []);
      setSimilarResults(data.similarDocuments || []);
      setHasSearched(true);
    } catch (e) {
      console.error('Search failed:', e);
    } finally {
      setSearching(false);
    }
  };

  const getSimilarityColor = (score: number) => {
    if (score >= 0.9) return 'text-green-400';
    if (score >= 0.8) return 'text-violet-400';
    return 'text-slate-400';
  };

  const formatDate = (dateStr: string) => {
    try { return new Date(dateStr).toLocaleDateString('ko-KR', { year: 'numeric', month: '2-digit', day: '2-digit' }); }
    catch { return dateStr; }
  };

  return (
    <Card className={`backdrop-blur-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'}`}>
      <div className="p-6 space-y-6">
        <div className="space-y-2">
          <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>문서 검색</h2>
          <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
            임베딩 기반 의미 검색으로 관련 문서를 찾아보세요
          </p>
        </div>

        <div className="flex gap-2">
          <div className="relative flex-1">
            <SearchIcon className="absolute left-3 top-1/2 -translate-y-1/2 size-5 text-slate-500" />
            <Input type="text" placeholder="검색어를 입력하세요..." value={query}
              onChange={(e) => setQuery(e.target.value)}
              onKeyDown={(e) => e.key === 'Enter' && handleSearch()}
              className={`pl-10 ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700 text-slate-200 placeholder:text-slate-500' : 'bg-slate-50 border-slate-300 text-slate-900'}`}
            />
          </div>
          <Button onClick={handleSearch} disabled={!query.trim() || searching}
            className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500 px-8">
            {searching ? '검색중...' : '검색'}
          </Button>
        </div>

        <ScrollArea className="h-[600px] pr-4">
          {!hasSearched ? (
            <div className="text-center py-12">
              <div className={`inline-block p-4 rounded-full mb-4 ${theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'}`}>
                <SearchIcon className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} />
              </div>
              <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>검색어를 입력하여 문서를 찾아보세요</p>
              <p className={`text-sm mt-2 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-500'}`}>자연어 질의를 통해 관련 문서를 찾을 수 있습니다</p>
            </div>
          ) : (
            <div className="space-y-6">
              {keywordResults.length > 0 && (
                <div className="space-y-3">
                  <h3 className={`text-sm font-semibold ${theme === 'dark' ? 'text-slate-300' : 'text-slate-700'}`}>키워드 일치 문서</h3>
                  {keywordResults.map((result, i) => (
                    <a key={i} href={api.getDownloadUrl(result.file_uuid)} target="_blank" rel="noopener noreferrer"
                      className={`block p-4 rounded-xl border transition-all cursor-pointer ${
                        theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                      }`}>
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                          <FileText className="size-4 text-white" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-start justify-between gap-2">
                            <div>
                              <h4 className={`font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>{(result.display_name || result.source_filename || '').normalize('NFC')}</h4>
                              {result.uploaded_at && <p className={`text-xs mt-1 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>{formatDate(result.uploaded_at)}</p>}
                            </div>
                            <Badge variant="outline" className={`${theme === 'dark' ? 'border-slate-600' : 'border-slate-400'} ${getSimilarityColor(result.score)}`}>
                              {(result.score * 100).toFixed(0)}% 일치
                            </Badge>
                          </div>
                        </div>
                      </div>
                    </a>
                  ))}
                </div>
              )}

              {similarResults.length > 0 && (
                <div className="space-y-3">
                  <h3 className={`text-sm font-semibold flex items-center gap-2 ${theme === 'dark' ? 'text-slate-300' : 'text-slate-700'}`}>
                    <TrendingUp className="size-4 text-violet-400" /> 연관 문서
                  </h3>
                  {similarResults.map((result, i) => (
                    <a key={i} href={result.link || api.getDownloadUrl(result.file_uuid || result.file)} target="_blank" rel="noopener noreferrer"
                      className={`block p-4 rounded-xl border transition-all cursor-pointer ${
                        theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'
                      }`}>
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600">
                          <FileText className="size-4 text-white" />
                        </div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-start justify-between gap-2">
                            <h4 className={`font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>
                              {(result.display_name || result.source_filename || '').normalize('NFC')}
                            </h4>
                            <Badge variant="outline" className={`${theme === 'dark' ? 'border-slate-600' : 'border-slate-400'} ${getSimilarityColor(result.score)}`}>
                              {(result.score * 100).toFixed(0)}%
                            </Badge>
                          </div>
                          {result.uploaded_at && <p className={`text-xs mt-1 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>{formatDate(result.uploaded_at)}</p>}
                        </div>
                      </div>
                    </a>
                  ))}
                </div>
              )}

              {keywordResults.length === 0 && similarResults.length === 0 && (
                <div className="text-center py-12">
                  <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>검색 결과가 없습니다</p>
                </div>
              )}
            </div>
          )}
        </ScrollArea>
      </div>
    </Card>
  );
}
