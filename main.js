// RecordRoute Electron Main Process
// Phase 1: Basic window creation and frontend loading

const { app, BrowserWindow } = require('electron');
const path = require('path');

function createWindow() {
  const mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      // preload: path.join(__dirname, 'preload.js') // Will be used in later phases
      nodeIntegration: false,
      contextIsolation: true
    }
  });

  // Load existing frontend file
  mainWindow.loadFile('frontend/upload.html');

  // Open DevTools in development (uncomment for debugging)
  // mainWindow.webContents.openDevTools();
}

app.whenReady().then(() => {
  createWindow();

  app.on('activate', function () {
    // On macOS re-create window when dock icon is clicked
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', function () {
  // Quit app when all windows are closed, except on macOS
  if (process.platform !== 'darwin') app.quit();
});
