import { type ReactNode, useEffect, useState } from 'react';
import { LoaderCircle, Plus, RefreshCw, X } from 'lucide-react';
import { Button } from './ui/button';
import { Card } from './ui/card';
import { Input } from './ui/input';
import { useTheme } from '../contexts/ThemeContext';

type MessageTone = 'success' | 'error';

interface DictionaryKeywords {
  userKeywords: string[];
  autoKeywords: string[];
}

interface DictionaryKeywordListResponse {
  user_keywords?: string[];
  auto_keywords?: string[];
}

interface StatusMessage {
  tone: MessageTone;
  text: string;
}

const emptyKeywords = (): DictionaryKeywords => ({
  userKeywords: [],
  autoKeywords: [],
});

function normalizeKeywords(payload: DictionaryKeywordListResponse | null | undefined): DictionaryKeywords {
  return {
    userKeywords: Array.isArray(payload?.user_keywords)
      ? payload.user_keywords.filter((keyword): keyword is string => Boolean(keyword))
      : [],
    autoKeywords: Array.isArray(payload?.auto_keywords)
      ? payload.auto_keywords.filter((keyword): keyword is string => Boolean(keyword))
      : [],
  };
}

function tryParseJson(text: string) {
  try {
    return JSON.parse(text);
  } catch {
    return text;
  }
}

async function fetchJson<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, init);
  const text = await response.text();
  const payload = text ? tryParseJson(text) : null;

  if (!response.ok) {
    const message =
      payload && typeof payload === 'object' && 'message' in payload
        ? String(payload.message)
        : `${response.status} ${response.statusText}`;
    throw new Error(message);
  }

  return payload as T;
}

function messageClasses(tone: MessageTone, theme: 'light' | 'dark') {
  const toneClasses = {
    success:
      theme === 'dark'
        ? 'border-emerald-500/30 bg-emerald-500/10 text-emerald-200'
        : 'border-emerald-300 bg-emerald-50 text-emerald-800',
    error:
      theme === 'dark'
        ? 'border-red-500/30 bg-red-500/10 text-red-200'
        : 'border-red-300 bg-red-50 text-red-800',
  } satisfies Record<MessageTone, string>;

  return toneClasses[tone];
}

function groupCardClasses(theme: 'light' | 'dark') {
  return theme === 'dark'
    ? 'border-slate-800 bg-slate-950/40'
    : 'border-slate-200 bg-slate-50/90';
}

function chipClasses(theme: 'light' | 'dark') {
  return theme === 'dark'
    ? 'border-slate-700 bg-slate-800/80 text-slate-100'
    : 'border-slate-200 bg-white text-slate-800';
}

function chipActionClasses(theme: 'light' | 'dark', tone: 'promote' | 'danger') {
  const palette = {
    promote:
      theme === 'dark'
        ? 'border-violet-500/40 text-violet-200 hover:bg-violet-500/20 hover:text-violet-100'
        : 'border-violet-200 text-violet-700 hover:bg-violet-50 hover:text-violet-900',
    danger:
      theme === 'dark'
        ? 'border-red-500/40 text-red-200 hover:bg-red-500/20 hover:text-red-100'
        : 'border-red-200 text-red-700 hover:bg-red-50 hover:text-red-900',
  } satisfies Record<'promote' | 'danger', string>;

  return `size-7 rounded-full border transition-colors ${palette[tone]}`;
}

interface DictionaryChipProps {
  keyword: string;
  theme: 'light' | 'dark';
  loading: boolean;
  onPromote?: (keyword: string) => void;
  onDelete: (keyword: string) => void;
}

function DictionaryChip({
  keyword,
  theme,
  loading,
  onPromote,
  onDelete,
}: DictionaryChipProps) {
  return (
    <li
      className={`flex items-center gap-2 rounded-full border px-3 py-2 ${chipClasses(theme)}`}
    >
      <span className="text-sm font-medium">{keyword}</span>
      <div className="flex items-center gap-1">
        {onPromote ? (
          <Button
            type="button"
            variant="ghost"
            size="icon"
            disabled={loading}
            onClick={() => onPromote(keyword)}
            className={chipActionClasses(theme, 'promote')}
            aria-label={`${keyword} 사용자 키워드로 이동`}
          >
            <Plus className="size-3.5" />
          </Button>
        ) : null}
        <Button
          type="button"
          variant="ghost"
          size="icon"
          disabled={loading}
          onClick={() => onDelete(keyword)}
          className={chipActionClasses(theme, 'danger')}
          aria-label={`${keyword} 삭제`}
        >
          <X className="size-3.5" />
        </Button>
      </div>
    </li>
  );
}

interface DictionaryGroupProps {
  title: string;
  description: string;
  emptyMessage: string;
  hasItems: boolean;
  children?: ReactNode;
  theme: 'light' | 'dark';
}

function DictionaryGroup({
  title,
  description,
  emptyMessage,
  hasItems,
  children,
  theme,
}: DictionaryGroupProps) {
  return (
    <section className={`rounded-2xl border p-5 ${groupCardClasses(theme)}`}>
      <div className="space-y-1">
        <h3 className={`text-base font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
          {title}
        </h3>
        <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
          {description}
        </p>
      </div>

      <div className="mt-4">
        {hasItems ? (
          <ul className="flex flex-wrap gap-2">{children}</ul>
        ) : (
          <p className={`text-sm ${theme === 'dark' ? 'text-slate-500' : 'text-slate-500'}`}>
            {emptyMessage}
          </p>
        )}
      </div>
    </section>
  );
}

export function DictionaryPanel() {
  const { theme } = useTheme();
  const [keyword, setKeyword] = useState('');
  const [keywords, setKeywords] = useState<DictionaryKeywords>(emptyKeywords);
  const [loading, setLoading] = useState(false);
  const [loaded, setLoaded] = useState(false);
  const [message, setMessage] = useState<StatusMessage | null>(null);

  const refreshKeywords = async ({
    showMessage = false,
    signal,
  }: {
    showMessage?: boolean;
    signal?: AbortSignal;
  } = {}) => {
    setLoading(true);
    try {
      const payload = await fetchJson<DictionaryKeywordListResponse>('/dictionary/keywords', { signal });
      setKeywords(normalizeKeywords(payload));
      setLoaded(true);
      if (showMessage) {
        setMessage({ tone: 'success', text: '키워드 목록을 갱신했습니다.' });
      }
    } catch (error) {
      if (error instanceof DOMException && error.name === 'AbortError') {
        return;
      }
      setMessage({
        tone: 'error',
        text: error instanceof Error ? error.message : '키워드 목록을 불러오지 못했습니다.',
      });
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const controller = new AbortController();
    void refreshKeywords({ signal: controller.signal });

    return () => controller.abort();
  }, []);

  const submitKeyword = async (event: React.FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    const trimmed = keyword.trim();
    if (!trimmed) {
      setMessage({ tone: 'error', text: '추가할 키워드를 입력해 주세요.' });
      return;
    }

    setLoading(true);
    try {
      const payload = await fetchJson<DictionaryKeywordListResponse>('/dictionary/keywords', {
        method: 'POST',
        headers: { 'content-type': 'application/json' },
        body: JSON.stringify({ keyword: trimmed }),
      });
      setKeywords(normalizeKeywords(payload));
      setLoaded(true);
      setKeyword('');
      setMessage({ tone: 'success', text: '키워드를 추가했습니다.' });
    } catch (error) {
      setMessage({
        tone: 'error',
        text: error instanceof Error ? error.message : '키워드 추가에 실패했습니다.',
      });
    } finally {
      setLoading(false);
    }
  };

  const applyKeywordMutation = async ({
    path,
    init,
    successMessage,
  }: {
    path: string;
    init: RequestInit;
    successMessage: string;
  }) => {
    setLoading(true);
    try {
      const payload = await fetchJson<DictionaryKeywordListResponse>(path, init);
      setKeywords(normalizeKeywords(payload));
      setLoaded(true);
      setMessage({ tone: 'success', text: successMessage });
    } catch (error) {
      setMessage({
        tone: 'error',
        text: error instanceof Error ? error.message : '키워드 작업에 실패했습니다.',
      });
    } finally {
      setLoading(false);
    }
  };

  const handleDeleteUserKeyword = async (targetKeyword: string) => {
    if (!window.confirm(`${targetKeyword} 키워드를 삭제하시겠습니까?`)) {
      return;
    }

    await applyKeywordMutation({
      path: `/dictionary/keywords/${encodeURIComponent(targetKeyword)}`,
      init: { method: 'DELETE' },
      successMessage: '키워드를 삭제했습니다.',
    });
  };

  const handlePromoteAutoKeyword = async (targetKeyword: string) => {
    await applyKeywordMutation({
      path: `/dictionary/keywords/auto/${encodeURIComponent(targetKeyword)}/promote`,
      init: { method: 'POST' },
      successMessage: '자동 생성 키워드를 사용자 등록 키워드로 옮겼습니다.',
    });
  };

  const handleDeleteAutoKeyword = async (targetKeyword: string) => {
    await applyKeywordMutation({
      path: `/dictionary/keywords/auto/${encodeURIComponent(targetKeyword)}`,
      init: { method: 'DELETE' },
      successMessage: '자동 생성 키워드를 삭제했습니다.',
    });
  };

  const renderListBody = () => {
    if (!loaded && loading) {
      return (
        <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
          키워드를 불러오는 중입니다.
        </p>
      );
    }

    if (!loaded) {
      return (
        <p className={`text-sm ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
          키워드를 불러오지 못했습니다. 새로고침을 눌러 다시 시도해 주세요.
        </p>
      );
    }

    return (
      <div className="space-y-4">
        <DictionaryGroup
          title="사용자 등록 키워드"
          description="직접 추가한 키워드만 STT 기본 프롬프트에 주입됩니다."
          emptyMessage="사용자가 직접 등록한 키워드가 없습니다."
          hasItems={keywords.userKeywords.length > 0}
          theme={theme}
        >
          {keywords.userKeywords.map((item) => (
            <DictionaryChip
              key={`user-${item}`}
              keyword={item}
              theme={theme}
              loading={loading}
              onDelete={handleDeleteUserKeyword}
            />
          ))}
        </DictionaryGroup>

        <DictionaryGroup
          title="자동 생성 키워드"
          description="LLM이 확인한 후보 키워드입니다. + 버튼으로 사용자 키워드로 옮길 수 있습니다."
          emptyMessage="자동 생성된 키워드가 없습니다."
          hasItems={keywords.autoKeywords.length > 0}
          theme={theme}
        >
          {keywords.autoKeywords.map((item) => (
            <DictionaryChip
              key={`auto-${item}`}
              keyword={item}
              theme={theme}
              loading={loading}
              onPromote={handlePromoteAutoKeyword}
              onDelete={handleDeleteAutoKeyword}
            />
          ))}
        </DictionaryGroup>
      </div>
    );
  };

  return (
    <Card
      className={`backdrop-blur-sm ${
        theme === 'dark'
          ? 'border-slate-800 bg-slate-900/50'
          : 'border-slate-200 bg-white/50'
      }`}
    >
      <div className="p-6 space-y-6">
        <div className="flex flex-col gap-4 md:flex-row md:items-start md:justify-between">
          <div className="space-y-2">
            <div className="space-y-1">
              <h2 className={`text-xl font-semibold ${theme === 'dark' ? 'text-white' : 'text-slate-900'}`}>
                주요 키워드
              </h2>
              <p className={`max-w-2xl text-sm leading-6 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                사용자가 직접 등록한 키워드만 STT 실행 시 Whisper 초기 프롬프트에 자동 주입됩니다.
                자동 생성 키워드는 후보 목록으로만 관리됩니다.
              </p>
            </div>
          </div>

          <Button
            type="button"
            variant="outline"
            disabled={loading}
            onClick={() => void refreshKeywords({ showMessage: true })}
            className={`gap-2 ${
              theme === 'dark'
                ? 'border-slate-700 bg-slate-800/70 text-slate-200 hover:bg-slate-700'
                : 'border-slate-300 bg-white text-slate-700 hover:bg-slate-100'
            }`}
          >
            {loading ? <LoaderCircle className="size-4 animate-spin" /> : <RefreshCw className="size-4" />}
            키워드 새로고침
          </Button>
        </div>

        {message ? (
          <div
            aria-live="polite"
            className={`rounded-xl border px-4 py-3 text-sm ${messageClasses(message.tone, theme)}`}
          >
            {message.text}
          </div>
        ) : null}

        <form className="space-y-3" onSubmit={submitKeyword}>
          <label className="block space-y-2">
            <span className={`text-sm font-medium ${theme === 'dark' ? 'text-slate-200' : 'text-slate-700'}`}>
              Keyword
            </span>
            <div className="flex flex-col gap-3 sm:flex-row">
              <Input
                name="keyword"
                type="text"
                autoComplete="off"
                value={keyword}
                onChange={(event) => setKeyword(event.target.value)}
                placeholder="예: RecordRoute, Dooray, 김현수"
                className={`flex-1 ${
                  theme === 'dark'
                    ? 'border-slate-700 bg-slate-800/50 text-slate-100 placeholder:text-slate-500'
                    : 'border-slate-300 bg-slate-50 text-slate-900 placeholder:text-slate-500'
                }`}
              />
              <Button
                type="submit"
                disabled={loading}
                className="gap-2 bg-gradient-to-r from-violet-600 to-fuchsia-600 text-white hover:from-violet-500 hover:to-fuchsia-500 sm:min-w-28"
              >
                {loading ? <LoaderCircle className="size-4 animate-spin" /> : <Plus className="size-4" />}
                확인
              </Button>
            </div>
          </label>
        </form>

        {renderListBody()}
      </div>
    </Card>
  );
}
