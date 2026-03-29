import { useState } from 'react';
import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
} from './ui/dialog';
import { Button } from './ui/button';
import { Tabs, TabsContent, TabsList, TabsTrigger } from './ui/tabs';
import { ScrollArea } from './ui/scroll-area';
import { Copy, Download, Edit, Trash2, X } from 'lucide-react';
import { Badge } from './ui/badge';

interface STTResult {
  id: string;
  filename: string;
  content: Array<{
    start: string;
    end: string;
    text: string;
  }>;
}

interface STTViewerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  result: STTResult | null;
}

export function STTViewer({ open, onOpenChange, result }: STTViewerProps) {
  const [activeTab, setActiveTab] = useState('raw');

  if (!result) return null;

  const handleCopy = () => {
    const text = result.content.map(item => item.text).join('\n');
    navigator.clipboard.writeText(text);
  };

  const handleDownload = () => {
    const text = result.content
      .map(item => `[${item.start} - ${item.end}] ${item.text}`)
      .join('\n');
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${result.filename}_stt.txt`;
    a.click();
  };

  const handleEdit = () => {
    // Edit logic
    console.log('Edit STT result');
  };

  const handleDelete = () => {
    // Delete logic
    console.log('Delete STT result');
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="bg-slate-900 border-slate-700 text-slate-100 max-w-4xl max-h-[85vh]">
        <DialogHeader>
          <div className="flex items-start justify-between gap-4">
            <div>
              <DialogTitle className="text-xl text-white">STT 결과</DialogTitle>
              <p className="text-sm text-slate-400 mt-1">{result.filename}</p>
            </div>
            <Badge variant="outline" className="border-violet-500 text-violet-400">
              {result.id}
            </Badge>
          </div>
        </DialogHeader>

        <Tabs value={activeTab} onValueChange={setActiveTab} className="space-y-4">
          <div className="flex items-center justify-between">
            <TabsList className="bg-slate-800/50 border border-slate-700">
              <TabsTrigger value="raw">복사</TabsTrigger>
              <TabsTrigger value="edit">수정</TabsTrigger>
              <TabsTrigger value="download">다운로드</TabsTrigger>
              <TabsTrigger value="delete" className="text-red-400 data-[state=active]:text-red-400">
                삭제
              </TabsTrigger>
            </TabsList>

            <div className="flex items-center gap-2">
              <Button
                size="sm"
                variant="outline"
                onClick={handleCopy}
                className="bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700"
              >
                <Copy className="size-4 mr-2" />
                복사
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleEdit}
                className="bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700"
              >
                <Edit className="size-4 mr-2" />
                수정
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleDownload}
                className="bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700"
              >
                <Download className="size-4 mr-2" />
                다운로드
              </Button>
              <Button
                size="sm"
                variant="outline"
                onClick={handleDelete}
                className="bg-red-900/20 border-red-700 text-red-400 hover:bg-red-900/30"
              >
                <Trash2 className="size-4 mr-2" />
                삭제
              </Button>
            </div>
          </div>

          <TabsContent value={activeTab} className="mt-4">
            <ScrollArea className="h-[500px] rounded-lg border border-slate-700 bg-slate-950/50 p-6">
              <div className="space-y-4 font-mono text-sm">
                {result.content.map((item, index) => (
                  <div key={index} className="space-y-1">
                    <div className="flex items-center gap-2 text-slate-500">
                      <span>[{item.start}</span>
                      <span>-</span>
                      <span>{item.end}]</span>
                    </div>
                    <div className="text-slate-200 leading-relaxed pl-4">
                      {item.text}
                    </div>
                  </div>
                ))}
              </div>
            </ScrollArea>
          </TabsContent>
        </Tabs>

        <div className="flex justify-end pt-4 border-t border-slate-700">
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            className="bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700"
          >
            <X className="size-4 mr-2" />
            닫기
          </Button>
        </div>
      </DialogContent>
    </Dialog>
  );
}
