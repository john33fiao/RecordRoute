import { useState } from 'react';

export function useSearchState() {
  const [similarOpen, setSimilarOpen] = useState(false);
  const [similarFilePath, setSimilarFilePath] = useState<string | null>(null);
  const [similarFilename, setSimilarFilename] = useState('');

  const openSimilar = (filePath: string, filename: string) => {
    setSimilarFilePath(filePath);
    setSimilarFilename(filename);
    setSimilarOpen(true);
  };

  return {
    similarOpen,
    setSimilarOpen,
    similarFilePath,
    similarFilename,
    openSimilar,
  };
}
