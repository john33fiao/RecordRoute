// RecordRoute Electron Preload Script
// Phase 3: Security enhancement with contextIsolation

const { contextBridge, ipcRenderer } = require('electron');

/**
 * Expose safe APIs to the renderer process via contextBridge
 * This enables secure communication between the main and renderer processes
 * while maintaining contextIsolation for security
 */

// Currently, RecordRoute uses HTTP/WebSocket for all communication,
// so IPC is minimal. This provides a foundation for future IPC needs.

contextBridge.exposeInMainWorld('electronAPI', {
  // Platform information
  platform: process.platform,

  // Version information
  versions: {
    node: process.versions.node,
    chrome: process.versions.chrome,
    electron: process.versions.electron
  },

  // Future IPC methods can be added here
  // Example:
  // onProgress: (callback) => ipcRenderer.on('progress', callback),
  // sendMessage: (channel, data) => ipcRenderer.send(channel, data)
});

// Log preload script initialization
console.log('RecordRoute preload script initialized');
console.log('Platform:', process.platform);
console.log('Electron version:', process.versions.electron);
