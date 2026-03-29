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
import { Copy, Download, Trash2, X } from 'lucide-react';
import { Badge } from './ui/badge';

interface SummarySection {
  title: string;
  content: string[];
}

interface SummaryResult {
  id: string;
  filename: string;
  sections: SummarySection[];
}

interface SummaryViewerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  result: SummaryResult | null;
}

export function SummaryViewer({ open, onOpenChange, result }: SummaryViewerProps) {
  const [activeTab, setActiveTab] = useState('view');

  if (!result) return null;

  const handleCopy = () => {
    const text = result.sections
      .map(section => {
        const items = section.content.map(item => `  • ${item}`).join('\n');
        return `# ${section.title}\n${items}`;
      })
      .join('\n\n');
    navigator.clipboard.writeText(text);
  };

  const handleDownload = () => {
    const text = result.sections
      .map(section => {
        const items = section.content.map(item => `  • ${item}`).join('\n');
        return `# ${section.title}\n${items}`;
      })
      .join('\n\n');
    const blob = new Blob([text], { type: 'text/plain' });
    const url = URL.createObjectURL(blob);
    const a = document.createElement('a');
    a.href = url;
    a.download = `${result.filename}_summary.txt`;
    a.click();
  };

  const handleDelete = () => {
    console.log('Delete summary result');
    onOpenChange(false);
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="bg-slate-900 border-slate-700 text-slate-100 max-w-4xl max-h-[85vh]">
        <DialogHeader>
          <div className="flex items-start justify-between gap-4">
            <div>
              <DialogTitle className="text-xl text-white">요약 결과</DialogTitle>
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
              <TabsTrigger value="view">복사</TabsTrigger>
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
              <div className="space-y-6">
                {result.sections.map((section, index) => (
                  <div key={index} className="space-y-3">
                    <h3 className="text-lg font-semibold text-violet-400 flex items-center gap-2">
                      <span className="text-slate-500">#</span>
                      {section.title}
                    </h3>
                    <ul className="space-y-2 pl-4">
                      {section.content.map((item, itemIndex) => (
                        <li 
                          key={itemIndex}
                          className="text-slate-300 leading-relaxed flex gap-3"
                        >
                          <span className="text-violet-500 shrink-0">•</span>
                          <span>{item}</span>
                        </li>
                      ))}
                    </ul>
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
