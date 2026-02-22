import {
  Dialog,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogFooter,
} from './ui/dialog';
import { Button } from './ui/button';
import { ScrollArea } from './ui/scroll-area';
import { Badge } from './ui/badge';
import { FileText, X } from 'lucide-react';

interface SimilarDocument {
  id: string;
  filename: string;
  similarity: number;
  preview: string;
}

interface SimilarDocumentsViewerProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  documents: SimilarDocument[];
  sourceFilename?: string;
}

export function SimilarDocumentsViewer({ 
  open, 
  onOpenChange, 
  documents,
  sourceFilename 
}: SimilarDocumentsViewerProps) {
  const getSimilarityColor = (similarity: number) => {
    if (similarity >= 90) return 'text-green-400 border-green-500';
    if (similarity >= 70) return 'text-violet-400 border-violet-500';
    return 'text-slate-400 border-slate-500';
  };

  const handleGenerate = () => {
    console.log('Generate related content');
  };

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="bg-slate-900 border-slate-700 text-slate-100 max-w-3xl max-h-[85vh]">
        <DialogHeader>
          <div className="flex items-start justify-between gap-4">
            <div>
              <DialogTitle className="text-xl text-white">
                유사한 문서 (상위 {documents.length}개)
              </DialogTitle>
              {sourceFilename && (
                <p className="text-sm text-slate-400 mt-1">기준: {sourceFilename}</p>
              )}
            </div>
          </div>
        </DialogHeader>

        <ScrollArea className="h-[500px] pr-4">
          <div className="space-y-3">
            {documents.map((doc) => (
              <div
                key={doc.id}
                className="p-4 rounded-xl bg-slate-800/50 border border-slate-700 hover:border-slate-600 transition-all cursor-pointer"
              >
                <div className="flex items-start gap-3">
                  <div className="p-2 rounded-lg bg-gradient-to-r from-violet-600 to-fuchsia-600 shrink-0">
                    <FileText className="size-4 text-white" />
                  </div>
                  <div className="flex-1 min-w-0 space-y-2">
                    <div className="flex items-start justify-between gap-2">
                      <h3 className="font-medium text-slate-200">{doc.filename}</h3>
                      <Badge 
                        variant="outline" 
                        className={getSimilarityColor(doc.similarity)}
                      >
                        유사도: {doc.similarity}%
                      </Badge>
                    </div>
                    <p className="text-sm text-slate-400 line-clamp-3 leading-relaxed">
                      {doc.preview}
                    </p>
                  </div>
                </div>
              </div>
            ))}
          </div>
        </ScrollArea>

        <DialogFooter className="gap-2">
          <Button
            onClick={handleGenerate}
            className="bg-gradient-to-r from-violet-600 to-fuchsia-600 hover:from-violet-500 hover:to-fuchsia-500"
          >
            생성
          </Button>
          <Button
            variant="outline"
            onClick={() => onOpenChange(false)}
            className="bg-slate-800 border-slate-700 text-slate-300 hover:bg-slate-700"
          >
            <X className="size-4 mr-2" />
            닫기
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
