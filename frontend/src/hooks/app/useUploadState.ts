import { useState } from 'react';

export function useUploadState() {
  const [activeTab, setActiveTab] = useState('upload');
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [resetAllOpen, setResetAllOpen] = useState(false);

  return {
    activeTab,
    setActiveTab,
    settingsOpen,
    setSettingsOpen,
    resetAllOpen,
    setResetAllOpen,
  };
}
