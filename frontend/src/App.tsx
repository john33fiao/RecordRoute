import { useCallback } from 'react';
import { Upload, Clock, History, Search, Settings, Sparkles } from 'lucide-react';
import { UploadSection } from './components/UploadSection';
import { JobQueue } from './components/JobQueue';
import { HistoryPanel } from './components/HistoryPanel';
import { SearchPanel } from './components/SearchPanel';
import { SettingsDialog } from './components/SettingsDialog';
import { TextOverlay } from './components/TextOverlay';
import { SimilarDocsDialog } from './components/SimilarDocsDialog';
import { ResetAllDialog, SttEditResetDialog } from './components/ConfirmDialogs';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './components/ui/tabs';
import { ThemeProvider, useTheme } from './contexts/ThemeContext';
import { AppProvider, useApp } from './contexts/AppContext';
import type { HistoryRecord } from './api/types';
import { useUploadState } from './hooks/app/useUploadState';
import { useSearchState } from './hooks/app/useSearchState';
import { useViewerState } from './hooks/app/useViewerState';
import { useQueueState } from './hooks/app/useQueueState';

function AppContent() {
  const { theme } = useTheme();
  const { loadHistory } = useApp();

  const uploadState = useUploadState();
  const searchState = useSearchState();
  const viewerState = useViewerState();
  const queueState = useQueueState();

  const handleViewContent = useCallback((fileIdentifier: string, fileType: 'stt' | 'summary', record: HistoryRecord) => {
    viewerState.openViewer(fileIdentifier, fileType, record);
  }, [viewerState]);

  const handleSttEdited = useCallback((recordId: string) => {
    queueState.openSttReset(recordId);
  }, [queueState]);

  return (
    <div className={`min-h-screen ${
      theme === 'dark'
        ? 'bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950 text-slate-100'
        : 'bg-gradient-to-br from-slate-50 via-white to-slate-100 text-slate-900'
    }`}>
      <header className={`border-b sticky top-0 z-50 backdrop-blur-xl ${
        theme === 'dark' ? 'border-slate-800 bg-slate-900/50' : 'border-slate-200 bg-white/50'
      }`}>
        <div className="container mx-auto px-6 py-4 flex items-center justify-between">
          <div className="flex items-center gap-3">
            <div className="relative">
              <div className="absolute inset-0 bg-gradient-to-r from-violet-500 to-fuchsia-500 rounded-lg blur-md opacity-75"></div>
              <div className="relative bg-gradient-to-r from-violet-600 to-fuchsia-600 p-2 rounded-lg">
                <Sparkles className="size-6 text-white" />
              </div>
            </div>
            <div>
              <h1 className="text-2xl font-bold bg-gradient-to-r from-violet-400 to-fuchsia-400 bg-clip-text text-transparent">RecordRoute</h1>
              <p className={`text-xs ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>음성을 회의록으로 변환</p>
            </div>
          </div>
          <button className={`p-2 rounded-lg transition-colors ${theme === 'dark' ? 'hover:bg-slate-800' : 'hover:bg-slate-100'}`}
            onClick={() => uploadState.setSettingsOpen(true)}>
            <Settings className={`size-5 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`} />
          </button>
        </div>
      </header>

      <div className="container mx-auto px-6 py-8">
        <Tabs value={uploadState.activeTab} onValueChange={uploadState.setActiveTab} className="space-y-6">
          <TabsList className={`p-1 border ${
            theme === 'dark' ? 'bg-slate-900/50 border-slate-800' : 'bg-slate-100/50 border-slate-200'
          }`}>
            <TabsTrigger value="upload" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2"><Upload className="size-4" /> 업로드</TabsTrigger>
            <TabsTrigger value="queue" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2"><Clock className="size-4" /> 작업 큐</TabsTrigger>
            <TabsTrigger value="history" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2"><History className="size-4" /> 기록</TabsTrigger>
            <TabsTrigger value="search" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2"><Search className="size-4" /> 검색</TabsTrigger>
          </TabsList>

          <TabsContent value="upload"><UploadSection /></TabsContent>
          <TabsContent value="queue"><JobQueue /></TabsContent>
          <TabsContent value="history">
            <HistoryPanel
              onViewContent={handleViewContent}
              onShowSimilarDocs={searchState.openSimilar}
              onShowResetAll={() => uploadState.setResetAllOpen(true)}
            />
          </TabsContent>
          <TabsContent value="search"><SearchPanel /></TabsContent>
        </Tabs>
      </div>

      <SettingsDialog open={uploadState.settingsOpen} onOpenChange={uploadState.setSettingsOpen} />
      <TextOverlay open={viewerState.overlayOpen} onOpenChange={viewerState.setOverlayOpen}
        fileIdentifier={viewerState.overlayFile} fileType={viewerState.overlayType} filename={viewerState.overlayFilename} recordId={viewerState.overlayRecordId}
        onDeleted={loadHistory} onSttEdited={handleSttEdited} />
      <SimilarDocsDialog open={searchState.similarOpen} onOpenChange={searchState.setSimilarOpen}
        filePath={searchState.similarFilePath} filename={searchState.similarFilename} />
      <ResetAllDialog open={uploadState.resetAllOpen} onOpenChange={uploadState.setResetAllOpen} onComplete={loadHistory} />
      <SttEditResetDialog open={queueState.sttResetOpen} onOpenChange={queueState.setSttResetOpen}
        recordId={queueState.sttResetRecordId} onComplete={loadHistory} />
    </div>
  );
}

export default function App() {
  return (
    <ThemeProvider>
      <AppProvider>
        <AppContent />
      </AppProvider>
    </ThemeProvider>
  );
}
