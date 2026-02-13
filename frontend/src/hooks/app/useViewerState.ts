import { useState } from 'react';
import type { HistoryRecord, ViewerFileType } from '../../api/types';

export function useViewerState() {
  const [overlayOpen, setOverlayOpen] = useState(false);
  const [overlayFile, setOverlayFile] = useState<string | null>(null);
  const [overlayType, setOverlayType] = useState<ViewerFileType>('stt');
  const [overlayFilename, setOverlayFilename] = useState('');
  const [overlayRecordId, setOverlayRecordId] = useState('');

  const openViewer = (fileIdentifier: string, fileType: ViewerFileType, record: HistoryRecord) => {
    setOverlayFile(fileIdentifier);
    setOverlayType(fileType);
    setOverlayFilename(record.filename);
    setOverlayRecordId(record.id);
    setOverlayOpen(true);
  };

  return {
    overlayOpen,
    setOverlayOpen,
    overlayFile,
    overlayType,
    overlayFilename,
    overlayRecordId,
    openViewer,
  };
}
