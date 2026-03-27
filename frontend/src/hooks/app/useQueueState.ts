import { useState } from 'react';

export function useQueueState() {
  const [sttResetOpen, setSttResetOpen] = useState(false);
  const [sttResetRecordId, setSttResetRecordId] = useState<string | null>(null);

  const openSttReset = (recordId: string) => {
    setSttResetRecordId(recordId);
    setSttResetOpen(true);
  };

  return {
    sttResetOpen,
    setSttResetOpen,
    sttResetRecordId,
    openSttReset,
  };
}
