import { type FormEvent, useState } from 'react';
import { FileText, Loader2, Search as SearchIcon } from 'lucide-react';
import { Card } from './ui/card';
import { Input } from './ui/input';
import { Button } from './ui/button';
import { Badge } from './ui/badge';
import { ScrollArea } from './ui/scroll-area';
import { useTheme } from '../contexts/ThemeContext';

type SearchMessageTone = 'info' | 'success' | 'error';

interface SearchMessage {
  tone: SearchMessageTone;
  text: string;
}

interface SummarySearchRequest {
  query: string;
  limit: number;
  min_score?: number;
}

interface SummarySearchResultResponse {
  job_id: string;
  score: number;
  source_file_name: string;
  summary_file_name: string;
  summary_excerpt: string;
}

interface SummarySearchResponse {
  query: string;
  results: SummarySearchResultResponse[];
}

function tryParseJson(text: string) {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

async function searchSummaries(payload: SummarySearchRequest) {
  const response = await fetch('/summary/search', {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
    },
    body: JSON.stringify(payload),
  });
  const text = await response.text();
  const parsed = text ? tryParseJson(text) : null;

  if (!response.ok) {
    const message =
      parsed && typeof parsed === 'object' && 'message' in parsed
        ? String(parsed.message)
        : `${response.status} ${response.statusText}`;
    throw new Error(message);
  }

  return parsed as SummarySearchResponse;
}

function messageClasses(theme: 'light' | 'dark', tone: SearchMessageTone) {
  const palette = {
    info:
      theme === 'dark'
        ? 'border-slate-700 bg-slate-800/80 text-slate-300'
        : 'border-slate-300 bg-slate-50 text-slate-700',
    success:
      theme === 'dark'
        ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-200'
        : 'border-emerald-300 bg-emerald-50 text-emerald-800',
    error:
      theme === 'dark'
        ? 'border-red-500/30 bg-red-500/10 text-red-200'
        : 'border-red-300 bg-red-50 text-red-800',
  } satisfies Record<SearchMessageTone, string>;

  return palette[tone];
}

function scoreBadgeClasses(theme: 'light' | 'dark', score: number) {
  if (score >= 0.9) {
    return theme === 'dark'
      ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-200'
      : 'border-emerald-200 bg-emerald-50 text-emerald-700';
  }

  if (score >= 0.8) {
    return theme === 'dark'
      ? 'border-violet-500/30 bg-violet-500/10 text-violet-200'
      : 'border-violet-200 bg-violet-50 text-violet-700';
  }

  return theme === 'dark'
    ? 'border-slate-600 bg-slate-800/80 text-slate-300'
    : 'border-slate-300 bg-slate-50 text-slate-700';
}

function buildErrorMessage(error: unknown, fallback: string) {
  return error instanceof Error ? error.message : fallback;
}

export function SearchPanel() {
  const { theme } = useTheme();
  const [query, setQuery] = useState('');
  const [limit, setLimit] = useState('10');
  const [minScore, setMinScore] = useState('');
  const [isSearching, setIsSearching] = useState(false);
  const [results, setResults] = useState<SummarySearchResultResponse[]>([]);
  const [message, setMessage] = useState<SearchMessage>({
    tone: 'info',
    text: '검색어를 입력하고 검색하면 유사도 검색 결과가 표시됩니다.',
  });

  const handleSubmit = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();

    const trimmedQuery = query.trim();
    if (!trimmedQuery) {
      setResults([]);
      setMessage({
        tone: 'error',
        text: '검색어를 입력해 주세요.',
      });
      return;
    }

    const rawLimit = limit.trim();
    const normalizedLimit = rawLimit === '' ? 10 : Number(rawLimit);
    if (!Number.isInteger(normalizedLimit) || normalizedLimit < 1) {
      setResults([]);
      setMessage({
        tone: 'error',
        text: '결과 수는 1 이상의 정수로 입력해 주세요.',
      });
      return;
    }

    const clampedLimit = Math.min(normalizedLimit, 50);
    if (rawLimit === '' || clampedLimit !== normalizedLimit) {
      setLimit(String(clampedLimit));
    }

    const rawMinScore = minScore.trim();
    let parsedMinScore: number | undefined;
    if (rawMinScore) {
      parsedMinScore = Number(rawMinScore);
      if (!Number.isFinite(parsedMinScore)) {
        setResults([]);
        setMessage({
          tone: 'error',
          text: '최소 점수는 숫자로 입력해 주세요.',
        });
        return;
      }
    }

    const payload: SummarySearchRequest = {
      query: trimmedQuery,
      limit: clampedLimit,
    };
    if (parsedMinScore !== undefined) {
      payload.min_score = parsedMinScore;
    }

    setResults([]);
    setIsSearching(true);
    setMessage({
      tone: 'info',
      text: '요약 검색을 실행 중입니다.',
    });

    try {
      const response = await searchSummaries(payload);
      setResults(response.results);
      setQuery(trimmedQuery);
      setMessage({
        tone: response.results.length > 0 ? 'success' : 'info',
        text:
          response.results.length > 0
            ? `${response.results.length}개의 결과를 찾았습니다. 검색 대상 Job에는 embedding이 미리 생성돼 있어야 합니다.`
            : '결과가 없습니다. 검색 대상 summary에 embedding이 생성돼 있는지 확인해 주세요.',
      });
    } catch (error) {
      setResults([]);
      setMessage({
        tone: 'error',
        text: buildErrorMessage(error, '검색 중 오류가 발생했습니다.'),
      });
    } finally {
      setIsSearching(false);
    }
  };

  return (
    <Card className={`backdrop-blur-sm ${
      theme === 'dark'
        ? 'border-slate-800 bg-slate-900/50'
        : 'border-slate-200 bg-white/50'
    }`}>
      <div className="space-y-6 p-6">
        <div className="space-y-2">
          <div className="flex items-center justify-between gap-4">
            <h2 className={`text-xl font-semibold ${
              theme === 'dark' ? 'text-white' : 'text-slate-900'
            }`}>
              문서 검색
            </h2>
            <Badge
              variant="outline"
              className={theme === 'dark'
                ? 'border-slate-700 text-slate-300'
                : 'border-slate-300 text-slate-700'
              }
            >
              /summary/search
            </Badge>
          </div>
          <p className={`text-sm ${
            theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
          }`}>
            임베딩 기반 유사도 검색으로 관련 summary를 찾습니다.
          </p>
        </div>

        <form onSubmit={handleSubmit} className="space-y-4">
          <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_120px_140px_auto] xl:items-end">
            <div className="space-y-2">
              <label
                htmlFor="summary-search-query"
                className={`text-sm font-medium ${
                  theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                }`}
              >
                query
              </label>
              <div className="relative">
                <SearchIcon className="absolute left-3 top-1/2 size-5 -translate-y-1/2 text-slate-500" />
                <Input
                  id="summary-search-query"
                  type="text"
                  value={query}
                  onChange={(event) => setQuery(event.target.value)}
                  placeholder="검색어를 입력하세요"
                  disabled={isSearching}
                  className={`pl-10 ${
                    theme === 'dark'
                      ? 'border-slate-700 bg-slate-800/50 text-slate-200 placeholder:text-slate-500'
                      : 'border-slate-300 bg-slate-50 text-slate-900 placeholder:text-slate-500'
                  }`}
                />
              </div>
            </div>

            <div className="space-y-2">
              <label
                htmlFor="summary-search-limit"
                className={`text-sm font-medium ${
                  theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                }`}
              >
                limit
              </label>
              <Input
                id="summary-search-limit"
                type="number"
                min="1"
                max="50"
                step="1"
                value={limit}
                onChange={(event) => setLimit(event.target.value)}
                disabled={isSearching}
                className={theme === 'dark'
                  ? 'border-slate-700 bg-slate-800/50 text-slate-200'
                  : 'border-slate-300 bg-slate-50 text-slate-900'
                }
              />
            </div>

            <div className="space-y-2">
              <label
                htmlFor="summary-search-min-score"
                className={`text-sm font-medium ${
                  theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                }`}
              >
                min_score
              </label>
              <Input
                id="summary-search-min-score"
                type="number"
                step="0.01"
                value={minScore}
                onChange={(event) => setMinScore(event.target.value)}
                placeholder="선택"
                disabled={isSearching}
                className={theme === 'dark'
                  ? 'border-slate-700 bg-slate-800/50 text-slate-200 placeholder:text-slate-500'
                  : 'border-slate-300 bg-slate-50 text-slate-900 placeholder:text-slate-500'
                }
              />
            </div>

            <Button
              type="submit"
              disabled={isSearching}
              className="bg-gradient-to-r from-violet-600 to-fuchsia-600 px-6 hover:from-violet-500 hover:to-fuchsia-500"
            >
              {isSearching ? (
                <>
                  <Loader2 className="mr-2 size-4 animate-spin" />
                  검색 중...
                </>
              ) : (
                <>
                  <SearchIcon className="mr-2 size-4" />
                  검색
                </>
              )}
            </Button>
          </div>

          <p className={`text-xs ${
            theme === 'dark' ? 'text-slate-500' : 'text-slate-500'
          }`}>
            limit은 기본값 10, 최대 50이며 min_score는 비워 두면 요청에서 제외됩니다.
          </p>
        </form>

        <div className={`rounded-xl border px-4 py-3 text-sm ${messageClasses(theme, message.tone)}`}>
          {message.text}
        </div>

        <div className="space-y-4">
          <div className="flex items-center justify-between gap-4">
            <h3 className={`text-sm font-semibold uppercase tracking-[0.16em] ${
              theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
            }`}>
              검색 결과
            </h3>
            <Badge
              variant="outline"
              className={theme === 'dark'
                ? 'border-slate-700 text-slate-300'
                : 'border-slate-300 text-slate-700'
              }
            >
              {results.length}건
            </Badge>
          </div>

          <ScrollArea className="h-[560px] pr-4">
            {isSearching ? (
              <div className="py-16 text-center">
                <div className={`mb-4 inline-flex rounded-full p-4 ${
                  theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'
                }`}>
                  <Loader2 className={`size-8 animate-spin ${
                    theme === 'dark' ? 'text-slate-400' : 'text-slate-500'
                  }`} />
                </div>
                <p className={theme === 'dark' ? 'text-slate-300' : 'text-slate-700'}>
                  요약 검색을 실행하고 있습니다.
                </p>
              </div>
            ) : results.length === 0 ? (
              <div className="py-16 text-center">
                <div className={`mb-4 inline-flex rounded-full p-4 ${
                  theme === 'dark' ? 'bg-slate-800' : 'bg-slate-100'
                }`}>
                  <SearchIcon className={`size-8 ${
                    theme === 'dark' ? 'text-slate-500' : 'text-slate-400'
                  }`} />
                </div>
                <p className={theme === 'dark' ? 'text-slate-300' : 'text-slate-700'}>
                  {message.tone === 'error' ? '검색을 완료하지 못했습니다.' : '표시할 검색 결과가 없습니다.'}
                </p>
                <p className={`mt-2 text-sm ${
                  theme === 'dark' ? 'text-slate-500' : 'text-slate-500'
                }`}>
                  {message.text}
                </p>
              </div>
            ) : (
              <div className="space-y-3">
                {results.map((result) => (
                  <article
                    key={`${result.job_id}:${result.summary_file_name}`}
                    className={`rounded-xl border p-4 transition-all ${
                      theme === 'dark'
                        ? 'border-slate-700 bg-slate-800/50 hover:border-slate-600'
                        : 'border-slate-200 bg-slate-50 hover:border-slate-300'
                    }`}
                  >
                    <div className="flex items-start gap-3">
                      <div className="rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600 p-2">
                        <FileText className="size-4 text-white" />
                      </div>
                      <div className="min-w-0 flex-1 space-y-4">
                        <div className="flex items-start justify-between gap-3">
                          <div className="min-w-0 space-y-1">
                            <h4 className={`truncate font-medium ${
                              theme === 'dark' ? 'text-slate-100' : 'text-slate-900'
                            }`}>
                              {result.source_file_name}
                            </h4>
                            <p className={`truncate text-sm ${
                              theme === 'dark' ? 'text-slate-400' : 'text-slate-600'
                            }`}>
                              {result.summary_file_name}
                            </p>
                          </div>
                          <Badge
                            variant="outline"
                            className={scoreBadgeClasses(theme, result.score)}
                          >
                            {result.score.toFixed(4)}
                          </Badge>
                        </div>

                        <div className="grid gap-3 text-sm sm:grid-cols-2">
                          <div className="space-y-1">
                            <p className={`text-xs uppercase tracking-[0.16em] ${
                              theme === 'dark' ? 'text-slate-500' : 'text-slate-500'
                            }`}>
                              원본 파일
                            </p>
                            <p className={`break-all ${
                              theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                            }`}>
                              {result.source_file_name}
                            </p>
                          </div>
                          <div className="space-y-1">
                            <p className={`text-xs uppercase tracking-[0.16em] ${
                              theme === 'dark' ? 'text-slate-500' : 'text-slate-500'
                            }`}>
                              요약 파일
                            </p>
                            <p className={`break-all ${
                              theme === 'dark' ? 'text-slate-200' : 'text-slate-800'
                            }`}>
                              {result.summary_file_name}
                            </p>
                          </div>
                        </div>

                        <div className={`rounded-lg border px-4 py-3 text-sm leading-6 ${
                          theme === 'dark'
                            ? 'border-slate-700 bg-slate-900/70 text-slate-300'
                            : 'border-slate-200 bg-white/90 text-slate-700'
                        }`}>
                          {result.summary_excerpt}
                        </div>
                      </div>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </ScrollArea>
        </div>
      </div>
    </Card>
  );
}
