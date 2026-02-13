import { useMemo, useState } from 'react';
import { Search as SearchIcon, FileText, TrendingUp } from 'lucide-react';
import { Card } from './ui/card';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { ScrollArea } from './ui/scroll-area';
import { useTheme } from '../contexts/ThemeContext';
import * as api from '../api/client';
import type { KeywordMatch, SimilarDocument, SearchResponse } from '../api/types';
import { TextOverlay } from './TextOverlay';

export function SearchPanel() {
  const { theme } = useTheme();
  const [query, setQuery] = useState('');
  const [limit, setLimit] = useState(10);
  const [startDate, setStartDate] = useState('');
  const [endDate, setEndDate] = useState('');
  const [sortBy, setSortBy] = useState<'similarity' | 'date'>('similarity');
  const [sortOrder, setSortOrder] = useState<'asc' | 'desc'>('desc');
  const [minScore, setMinScore] = useState('');
  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(5);
  const [includeTiming, setIncludeTiming] = useState(false);

  const [searching, setSearching] = useState(false);
  const [keywordResults, setKeywordResults] = useState<KeywordMatch[]>([]);
  const [similarResults, setSimilarResults] = useState<SimilarDocument[]>([]);
  const [hasSearched, setHasSearched] = useState(false);
  const [searchMeta, setSearchMeta] = useState<SearchResponse | null>(null);

  const [detailOpen, setDetailOpen] = useState(false);
  const [detailFileIdentifier, setDetailFileIdentifier] = useState<string | null>(null);
  const [detailTitle, setDetailTitle] = useState('');
  const [detailRecordId, setDetailRecordId] = useState('');

  const handleSearch = async () => {
    if (!query.trim()) return;
    setSearching(true);
    try {
      const data = await api.search({
        query: query.trim(),
        limit,
        start_date: startDate || undefined,
        end_date: endDate || undefined,
        sort_by: sortBy,
        sort_order: sortOrder,
        min_score: minScore === '' ? undefined : Number(minScore),
        page,
        page_size: pageSize,
        include_timing: includeTiming,
      });
      setKeywordResults(data.keywordMatches || []);
      setSimilarResults(data.similarDocuments || []);
      setHasSearched(true);
      setSearchMeta(data);
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

  const getIdentifierFromDownloadLink = (link?: string | null) => {
    if (!link) return null;
    const prefix = '/download/';
    if (!link.startsWith(prefix)) return null;
    const raw = link.slice(prefix.length);
    return raw ? raw : null;
  };

  const openKeywordDetail = (result: KeywordMatch) => {
    setDetailFileIdentifier(result.file_uuid || result.file);
    setDetailTitle((result.display_name || result.source_filename || '').normalize('NFC'));
    setDetailRecordId('');
    setDetailOpen(true);
  };

  const openSimilarDetail = (result: SimilarDocument) => {
    const identifier = result.file_uuid || getIdentifierFromDownloadLink(result.link) || result.file;
    setDetailFileIdentifier(identifier);
    setDetailTitle((result.display_name || result.source_filename || '').normalize('NFC'));
    setDetailRecordId(result.record_id || '');
    setDetailOpen(true);
  };

  const filterSummary = useMemo(() => {
    const parts = [
      `기간:${startDate || '전체'}~${endDate || '전체'}`,
      `최소점수:${minScore || '없음'}`,
      `정렬:${sortBy}/${sortOrder}`,
      `페이지:${page}·크기:${pageSize}`,
      `limit:${limit}`,
      includeTiming ? 'timing:on' : 'timing:off',
    ];
    return parts.join(' · ');
  }, [startDate, endDate, minScore, sortBy, sortOrder, page, pageSize, limit, includeTiming]);

  return (
    <Card className={`backdrop-blur-sm ${theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'}`}>
      <div className="p-6 space-y-6">
        <div className="space-y-2">
          <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>문서 검색</h2>
          <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>임베딩 기반 의미 검색으로 관련 문서를 찾아보세요</p>
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

        <div className="grid grid-cols-2 md:grid-cols-5 gap-2 text-xs">
          <Input type="number" min={1} value={limit} onChange={(e) => setLimit(Math.max(1, Number(e.target.value || 1)))} placeholder="limit" />
          <Input type="date" value={startDate} onChange={(e) => setStartDate(e.target.value)} />
          <Input type="date" value={endDate} onChange={(e) => setEndDate(e.target.value)} />
          <Input type="number" min={0} max={1} step={0.01} value={minScore} onChange={(e) => setMinScore(e.target.value)} placeholder="min score" />
          <label className="flex items-center gap-2 px-2">
            <input type="checkbox" checked={includeTiming} onChange={(e) => setIncludeTiming(e.target.checked)} /> timing
          </label>
          <select value={sortBy} onChange={(e) => setSortBy(e.target.value as 'similarity' | 'date')} className="border rounded px-2 py-1 bg-transparent">
            <option value="similarity">similarity</option>
            <option value="date">date</option>
          </select>
          <select value={sortOrder} onChange={(e) => setSortOrder(e.target.value as 'asc' | 'desc')} className="border rounded px-2 py-1 bg-transparent">
            <option value="desc">desc</option>
            <option value="asc">asc</option>
          </select>
          <Input type="number" min={1} value={page} onChange={(e) => setPage(Math.max(1, Number(e.target.value || 1)))} placeholder="page" />
          <Input type="number" min={1} value={pageSize} onChange={(e) => setPageSize(Math.max(1, Number(e.target.value || 1)))} placeholder="page size" />
        </div>

        <div className={`text-xs ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>현재 필터: {filterSummary}</div>

        <ScrollArea className="h-[600px] pr-4">
          {!hasSearched ? (
            <div className="text-center py-12">
              <div className={`inline-block p-4 rounded-full mb-4 ${theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'}`}>
                <SearchIcon className={`size-8 ${theme === 'dark' ? 'text-slate-600' : 'text-slate-400'}`} />
              </div>
              <p className={theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}>검색어를 입력하여 문서를 찾아보세요</p>
            </div>
          ) : (
            <div className="space-y-6">
              {searchMeta?.pagination && (
                <div className={`text-xs ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                  정렬: {(searchMeta.sort?.by || 'similarity')} / {(searchMeta.sort?.order || 'desc')} · 페이지: {searchMeta.pagination.page} · 반환: {searchMeta.pagination.returned}
                  {searchMeta.cache?.hit ? ' · 캐시 히트' : ''}
                </div>
              )}
              {keywordResults.length > 0 && (
                <div className="space-y-3">
                  <h3 className={`text-sm font-semibold ${theme === 'dark' ? 'text-slate-300' : 'text-slate-700'}`}>키워드 일치 문서</h3>
                  {keywordResults.map((result, i) => (
                    <button key={i} type="button" onClick={() => openKeywordDetail(result)}
                      className={`block w-full text-left p-4 rounded-xl border transition-all cursor-pointer ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'}`}>
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600"><FileText className="size-4 text-white" /></div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-start justify-between gap-2">
                            <div>
                              <h4 className={`font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>{(result.display_name || result.source_filename || '').normalize('NFC')}</h4>
                              {result.uploaded_at && <p className={`text-xs mt-1 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>{formatDate(result.uploaded_at)}</p>}
                            </div>
                            <Badge variant="outline" className={`${theme === 'dark' ? 'border-slate-600' : 'border-slate-400'} text-violet-400`}>{result.count}회 등장</Badge>
                          </div>
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              )}

              {similarResults.length > 0 && (
                <div className="space-y-3">
                  <h3 className={`text-sm font-semibold flex items-center gap-2 ${theme === 'dark' ? 'text-slate-300' : 'text-slate-700'}`}><TrendingUp className="size-4 text-violet-400" /> 연관 문서</h3>
                  {similarResults.map((result, i) => (
                    <button key={i} type="button" onClick={() => openSimilarDetail(result)}
                      className={`block w-full text-left p-4 rounded-xl border transition-all cursor-pointer ${theme === 'dark' ? 'bg-slate-800/50 border-slate-700 hover:border-slate-600' : 'bg-slate-50 border-slate-200 hover:border-slate-300'}`}>
                      <div className="flex items-start gap-3">
                        <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600"><FileText className="size-4 text-white" /></div>
                        <div className="flex-1 min-w-0">
                          <div className="flex items-start justify-between gap-2">
                            <h4 className={`font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-800'}`}>{(result.display_name || result.source_filename || '').normalize('NFC')}</h4>
                            <Badge variant="outline" className={`${theme === 'dark' ? 'border-slate-600' : 'border-slate-400'} ${getSimilarityColor(result.score)}`}>{(result.score * 100).toFixed(0)}%</Badge>
                          </div>
                          {result.uploaded_at && <p className={`text-xs mt-1 ${theme === 'dark' ? 'text-slate-500' : 'text-slate-600'}`}>{formatDate(result.uploaded_at)}</p>}
                        </div>
                      </div>
                    </button>
                  ))}
                </div>
              )}
            </div>
          )}
        </ScrollArea>
      </div>

      <TextOverlay
        open={detailOpen}
        onOpenChange={setDetailOpen}
        fileIdentifier={detailFileIdentifier}
        fileType="stt"
        filename={detailTitle}
        recordId={detailRecordId}
      />
    </Card>
  );
}
