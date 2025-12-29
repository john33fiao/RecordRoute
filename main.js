// RecordRoute Electron Main Process
// Phase 2: Python backend integration

const { app, BrowserWindow } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const fs = require('fs');

let pythonProcess = null;
let mainWindow = null;

// Determine if running in development or production
const isDev = !app.isPackaged;

/**
 * Get the path to the Python executable
 * - Development: Use virtual environment Python
 * - Production: Use PyInstaller bundled executable (Phase 3)
 */
function getPythonPath() {
  if (isDev) {
    // Development environment: use virtual environment Python
    if (process.platform === 'win32') {
      return path.join(__dirname, 'venv', 'Scripts', 'python.exe');
    } else {
      return path.join(__dirname, 'venv', 'bin', 'python');
    }
  } else {
    // Production environment: PyInstaller bundled executable
    const exeName = process.platform === 'win32' ? 'RecordRouteAPI.exe' : 'RecordRouteAPI';
    return path.join(process.resourcesPath, 'bin', exeName);
  }
}

/**
 * Get the path to server.py
 * - Development: Direct path to server.py
 * - Production: Bundled in executable, return null
 */
function getServerPath() {
  if (isDev) {
    return path.join(__dirname, 'sttEngine', 'server.py');
  } else {
    // Production: executable is the server itself
    return null;
  }
}

/**
 * Start the Python backend server
 */
function runPythonServer() {
  console.log('Starting Python server...');

  const pythonPath = getPythonPath();
  const serverPath = getServerPath();

  // Check if Python executable exists
  if (!fs.existsSync(pythonPath)) {
    console.error(`Python executable not found: ${pythonPath}`);
    console.error('Please ensure the virtual environment is created:');
    console.error('  python -m venv venv');
    console.error('  source venv/bin/activate  # or venv\\Scripts\\activate on Windows');
    console.error('  pip install -r sttEngine/requirements.txt');
    return;
  }

  // FFmpeg path configuration
  const ffmpegName = process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg';
  const ffmpegPath = isDev
    ? ffmpegName  // Development: use system PATH
    : path.join(process.resourcesPath, 'bin', ffmpegName);

  // Models directory path
  const modelsPath = isDev
    ? path.join(__dirname, 'models')
    : path.join(app.getPath('userData'), 'models');

  // Build command arguments
  const args = isDev
    ? [serverPath, `--ffmpeg_path=${ffmpegPath}`, `--models_path=${modelsPath}`]
    : [`--ffmpeg_path=${ffmpegPath}`, `--models_path=${modelsPath}`];

  console.log('Python path:', pythonPath);
  console.log('Server args:', args);

  // Spawn Python process
  pythonProcess = spawn(pythonPath, args);

  // Handle stdout
  pythonProcess.stdout.on('data', (data) => {
    const output = data.toString();
    console.log(`Python: ${output}`);

    // Detect when server is ready and create window
    if (output.includes('Serving HTTP on')) {
      console.log('Python server is ready!');
      if (!mainWindow) {
        createWindow();
      }
    }
  });

  // Handle stderr
  pythonProcess.stderr.on('data', (data) => {
    console.error(`Python Error: ${data}`);
  });

  // Handle process errors
  pythonProcess.on('error', (err) => {
    console.error('Failed to start Python process:', err);
  });

  // Handle process exit
  pythonProcess.on('close', (code) => {
    console.log(`Python process exited with code ${code}`);
    pythonProcess = null;
  });
}

/**
 * Create the main application window
 */
function createWindow() {
  mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      nodeIntegration: false,
      contextIsolation: true,
      preload: path.join(__dirname, 'preload.js')
    }
  });

  // Load existing frontend file
  mainWindow.loadFile('frontend/upload.html');

  // Open DevTools in development (uncomment for debugging)
  if (isDev) {
    // mainWindow.webContents.openDevTools();
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

// App lifecycle events
app.whenReady().then(() => {
  runPythonServer();

  app.on('activate', function () {
    // On macOS re-create window when dock icon is clicked
    if (BrowserWindow.getAllWindows().length === 0 && pythonProcess) {
      createWindow();
    }
  });
});

app.on('window-all-closed', function () {
  // Kill Python process when app is closing
  if (pythonProcess) {
    console.log('Terminating Python process...');
    pythonProcess.kill();
    pythonProcess = null;
  }

  // Quit app when all windows are closed, except on macOS
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// Ensure cleanup on app quit
app.on('quit', () => {
  if (pythonProcess) {
    console.log('Cleaning up Python process on quit...');
    pythonProcess.kill();
    pythonProcess = null;
  }
});
