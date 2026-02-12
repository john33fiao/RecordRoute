import { useState, useCallback } from 'react';
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

function AppContent() {
  const [activeTab, setActiveTab] = useState('upload');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { theme } = useTheme();
  const { loadHistory } = useApp();

  // TextOverlay state
  const [overlayOpen, setOverlayOpen] = useState(false);
  const [overlayFile, setOverlayFile] = useState<string | null>(null);
  const [overlayType, setOverlayType] = useState<'stt' | 'summary'>('stt');
  const [overlayFilename, setOverlayFilename] = useState('');
  const [overlayRecordId, setOverlayRecordId] = useState('');

  // SimilarDocs state
  const [similarOpen, setSimilarOpen] = useState(false);
  const [similarFilePath, setSimilarFilePath] = useState<string | null>(null);
  const [similarFilename, setSimilarFilename] = useState('');

  // ConfirmDialogs state
  const [resetAllOpen, setResetAllOpen] = useState(false);
  const [sttResetOpen, setSttResetOpen] = useState(false);
  const [sttResetRecordId, setSttResetRecordId] = useState<string | null>(null);

  const handleViewContent = useCallback((fileIdentifier: string, fileType: 'stt' | 'summary', record: HistoryRecord) => {
    setOverlayFile(fileIdentifier);
    setOverlayType(fileType);
    setOverlayFilename(record.filename);
    setOverlayRecordId(record.id);
    setOverlayOpen(true);
  }, []);

  const handleShowSimilarDocs = useCallback((filePath: string, filename: string) => {
    setSimilarFilePath(filePath);
    setSimilarFilename(filename);
    setSimilarOpen(true);
  }, []);

  const handleSttEdited = useCallback((recordId: string) => {
    setSttResetRecordId(recordId);
    setSttResetOpen(true);
  }, []);

  return (
    <div className={`min-h-screen ${
      theme === 'dark'
        ? 'bg-gradient-to-br from-slate-950 via-slate-900 to-slate-950 text-slate-100'
        : 'bg-gradient-to-br from-slate-50 via-white to-slate-100 text-slate-900'
    }`}>
      {/* Header */}
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
              <h1 className="text-2xl font-bold bg-gradient-to-r from-violet-400 to-fuchsia-400 bg-clip-text text-transparent">
                RecordRoute
              </h1>
              <p className={`text-xs ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`}>
                음성을 회의록으로 변환
              </p>
            </div>
          </div>
          <button className={`p-2 rounded-lg transition-colors ${theme === 'dark' ? 'hover:bg-slate-800' : 'hover:bg-slate-100'}`}
            onClick={() => setSettingsOpen(true)}>
            <Settings className={`size-5 ${theme === 'dark' ? 'text-slate-400' : 'text-slate-600'}`} />
          </button>
        </div>
      </header>

      {/* Main Content */}
      <div className="container mx-auto px-6 py-8">
        <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-6">
          <TabsList className={`p-1 border ${
            theme === 'dark' ? 'bg-slate-900/50 border-slate-800' : 'bg-slate-100/50 border-slate-200'
          }`}>
            <TabsTrigger value="upload" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
              <Upload className="size-4" /> 업로드
            </TabsTrigger>
            <TabsTrigger value="queue" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
              <Clock className="size-4" /> 작업 큐
            </TabsTrigger>
            <TabsTrigger value="history" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
              <History className="size-4" /> 기록
            </TabsTrigger>
            <TabsTrigger value="search" className="data-[state=active]:bg-gradient-to-r data-[state=active]:from-violet-600 data-[state=active]:to-fuchsia-600 gap-2">
              <Search className="size-4" /> 검색
            </TabsTrigger>
          </TabsList>

          <TabsContent value="upload"><UploadSection /></TabsContent>
          <TabsContent value="queue"><JobQueue /></TabsContent>
          <TabsContent value="history">
            <HistoryPanel onViewContent={handleViewContent} onShowSimilarDocs={handleShowSimilarDocs} onShowResetAll={() => setResetAllOpen(true)} />
          </TabsContent>
          <TabsContent value="search"><SearchPanel /></TabsContent>
        </Tabs>
      </div>

      {/* Dialogs */}
      <SettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} />
      <TextOverlay open={overlayOpen} onOpenChange={setOverlayOpen}
        fileIdentifier={overlayFile} fileType={overlayType} filename={overlayFilename} recordId={overlayRecordId}
        onDeleted={loadHistory} onSttEdited={handleSttEdited} />
      <SimilarDocsDialog open={similarOpen} onOpenChange={setSimilarOpen}
        filePath={similarFilePath} filename={similarFilename} />
      <ResetAllDialog open={resetAllOpen} onOpenChange={setResetAllOpen} onComplete={loadHistory} />
      <SttEditResetDialog open={sttResetOpen} onOpenChange={setSttResetOpen}
        recordId={sttResetRecordId} onComplete={loadHistory} />
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
