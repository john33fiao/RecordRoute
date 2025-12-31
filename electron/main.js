// RecordRoute Electron Main Process
// Phase 3: Rust backend integration

const { app, BrowserWindow, Menu, shell, dialog } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const fs = require('fs');

let rustProcess = null;
let mainWindow = null;
let autoUpdater = null;
let logStream = null;

// Determine if running in development or production
const isDev = !app.isPackaged;

// Project root directory (parent of electron/ folder)
const projectRoot = path.join(__dirname, '..');

/**
 * Setup logging to file
 * Redirects console.log, console.error, console.warn to log file
 */
function setupLogging() {
  const logDir = path.join(projectRoot, 'db', 'log');

  // Create log directory if it doesn't exist
  if (!fs.existsSync(logDir)) {
    fs.mkdirSync(logDir, { recursive: true });
  }

  // Create log file with timestamp (YYYYMMDD-HHmm format)
  const now = new Date();
  const timestamp = now.getFullYear().toString() +
    (now.getMonth() + 1).toString().padStart(2, '0') +
    now.getDate().toString().padStart(2, '0') + '-' +
    now.getHours().toString().padStart(2, '0') +
    now.getMinutes().toString().padStart(2, '0');
  const logFile = path.join(logDir, `electron-${timestamp}.log`);

  logStream = fs.createWriteStream(logFile, { flags: 'a' });

  // Write initial log header
  logStream.write(`=== Electron Log Started at ${new Date().toISOString()} ===\n`);
  logStream.write(`Platform: ${process.platform}\n`);
  logStream.write(`Electron Version: ${process.versions.electron}\n`);
  logStream.write(`Node Version: ${process.versions.node}\n`);
  logStream.write(`App Version: ${app.getVersion()}\n`);
  logStream.write(`Development Mode: ${isDev}\n`);
  logStream.write(`=====================================\n\n`);

  // Store original console methods
  const originalLog = console.log;
  const originalError = console.error;
  const originalWarn = console.warn;

  // Override console.log
  console.log = function(...args) {
    const message = args.map(arg =>
      typeof arg === 'object' ? JSON.stringify(arg, null, 2) : String(arg)
    ).join(' ');

    const timestamp = new Date().toISOString();
    const logMessage = `[${timestamp}] [LOG] ${message}\n`;

    if (logStream) {
      logStream.write(logMessage);
    }
    originalLog.apply(console, args);
  };

  // Override console.error
  console.error = function(...args) {
    const message = args.map(arg =>
      typeof arg === 'object' ? JSON.stringify(arg, null, 2) : String(arg)
    ).join(' ');

    const timestamp = new Date().toISOString();
    const logMessage = `[${timestamp}] [ERROR] ${message}\n`;

    if (logStream) {
      logStream.write(logMessage);
    }
    originalError.apply(console, args);
  };

  // Override console.warn
  console.warn = function(...args) {
    const message = args.map(arg =>
      typeof arg === 'object' ? JSON.stringify(arg, null, 2) : String(arg)
    ).join(' ');

    const timestamp = new Date().toISOString();
    const logMessage = `[${timestamp}] [WARN] ${message}\n`;

    if (logStream) {
      logStream.write(logMessage);
    }
    originalWarn.apply(console, args);
  };

  console.log('Logging system initialized');
}

/**
 * Get the path to the Rust executable
 * - Development: Use cargo-built binary from target/release
 * - Production: Use bundled executable in resources
 */
function getRustBinaryPath() {
  if (isDev) {
    // Development environment: use target/release binary
    const exeName = process.platform === 'win32' ? 'recordroute.exe' : 'recordroute';
    return path.join(projectRoot, 'recordroute-rs', 'target', 'release', exeName);
  } else {
    // Production environment: bundled executable
    const exeName = process.platform === 'win32' ? 'recordroute.exe' : 'recordroute';
    return path.join(process.resourcesPath, 'bin', exeName);
  }
}

/**
 * Start the Rust backend server
 */
function runRustServer() {
  console.log('Starting Rust server...');

  const rustBinaryPath = getRustBinaryPath();

  // Check if Rust executable exists
  if (!fs.existsSync(rustBinaryPath)) {
    console.error(`Rust executable not found: ${rustBinaryPath}`);
    console.error('Please build the Rust backend:');
    console.error('  cd recordroute-rs');
    console.error('  cargo build --release');

    // Show error dialog
    dialog.showErrorBox(
      'Backend Not Found',
      `Rust backend executable not found at:\n${rustBinaryPath}\n\nPlease build the project first:\ncd recordroute-rs\ncargo build --release`
    );
    return;
  }

  // FFmpeg path configuration
  const ffmpegName = process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg';
  const ffmpegPath = isDev
    ? ffmpegName  // Development: use system PATH
    : path.join(process.resourcesPath, 'bin', ffmpegName);

  // Models directory path
  const modelsPath = isDev
    ? path.join(projectRoot, 'models')
    : path.join(app.getPath('userData'), 'models');

  // Database directory path
  const dbPath = path.join(projectRoot, 'db');

  // Build command arguments
  const args = [
    'serve',
    '--host', '127.0.0.1',
    '--port', '8000',
    '--db-path', dbPath,
  ];

  // Add optional FFmpeg path if needed
  if (process.env.FFMPEG_PATH || !isDev) {
    process.env.FFMPEG_PATH = ffmpegPath;
  }

  console.log('Rust binary path:', rustBinaryPath);
  console.log('Server args:', args);
  console.log('Database path:', dbPath);
  console.log('Models path:', modelsPath);

  // Set environment variables
  process.env.RECORDROUTE_MODELS_DIR = modelsPath;
  process.env.RUST_LOG = isDev ? 'debug' : 'info';

  // Spawn Rust process
  rustProcess = spawn(rustBinaryPath, args, {
    env: { ...process.env },
    cwd: projectRoot
  });

  // Handle stdout
  rustProcess.stdout.on('data', (data) => {
    const output = data.toString();
    console.log(`Rust: ${output}`);

    // Detect when server is ready and create window
    if (output.includes('Server listening on') || output.includes('Listening on')) {
      console.log('Rust server is ready!');
      if (!mainWindow) {
        // Give server a moment to fully initialize
        setTimeout(() => {
          createWindow();
        }, 500);
      }
    }
  });

  // Handle stderr
  rustProcess.stderr.on('data', (data) => {
    const output = data.toString();
    // Rust uses stderr for info logs too, so don't treat all as errors
    if (output.toLowerCase().includes('error') || output.toLowerCase().includes('panic')) {
      console.error(`Rust Error: ${output}`);
    } else {
      console.log(`Rust: ${output}`);
    }
  });

  // Handle process errors
  rustProcess.on('error', (err) => {
    console.error('Failed to start Rust process:', err);
    dialog.showErrorBox(
      'Server Start Failed',
      `Failed to start Rust backend:\n${err.message}`
    );
  });

  // Handle process exit
  rustProcess.on('close', (code) => {
    console.log(`Rust process exited with code ${code}`);
    rustProcess = null;

    if (code !== 0 && code !== null) {
      dialog.showErrorBox(
        'Server Crashed',
        `Rust backend exited unexpectedly with code ${code}.\nCheck the logs for details.`
      );
    }
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
  mainWindow.loadFile(path.join(projectRoot, 'frontend', 'upload.html'));

  // Open DevTools in development (uncomment for debugging)
  if (isDev) {
    // mainWindow.webContents.openDevTools();
  }

  mainWindow.on('closed', () => {
    mainWindow = null;
  });
}

/**
 * Create application menu
 */
function createMenu() {
  const isMac = process.platform === 'darwin';

  const template = [
    // App menu (macOS only)
    ...(isMac ? [{
      label: app.name,
      submenu: [
        { role: 'about', label: 'RecordRoute에 대하여' },
        { type: 'separator' },
        { role: 'services', label: '서비스' },
        { type: 'separator' },
        { role: 'hide', label: 'RecordRoute 가리기' },
        { role: 'hideOthers', label: '다른 앱 가리기' },
        { role: 'unhide', label: '모두 보기' },
        { type: 'separator' },
        { role: 'quit', label: 'RecordRoute 종료' }
      ]
    }] : []),

    // File menu
    {
      label: '파일',
      submenu: [
        {
          label: '파일 업로드',
          accelerator: 'CmdOrCtrl+O',
          click: () => {
            if (mainWindow) {
              mainWindow.webContents.executeJavaScript('document.getElementById("fileInput")?.click()');
            }
          }
        },
        { type: 'separator' },
        {
          label: '새로고침',
          accelerator: 'CmdOrCtrl+R',
          click: () => {
            if (mainWindow) {
              mainWindow.reload();
            }
          }
        },
        { type: 'separator' },
        isMac ? { role: 'close', label: '창 닫기' } : { role: 'quit', label: '종료' }
      ]
    },

    // Edit menu
    {
      label: '편집',
      submenu: [
        { role: 'undo', label: '실행 취소' },
        { role: 'redo', label: '다시 실행' },
        { type: 'separator' },
        { role: 'cut', label: '잘라내기' },
        { role: 'copy', label: '복사' },
        { role: 'paste', label: '붙여넣기' },
        ...(isMac ? [
          { role: 'pasteAndMatchStyle', label: '붙여넣기 및 스타일 일치' },
          { role: 'delete', label: '삭제' },
          { role: 'selectAll', label: '모두 선택' },
          { type: 'separator' },
          {
            label: '말하기',
            submenu: [
              { role: 'startSpeaking', label: '말하기 시작' },
              { role: 'stopSpeaking', label: '말하기 중지' }
            ]
          }
        ] : [
          { role: 'delete', label: '삭제' },
          { type: 'separator' },
          { role: 'selectAll', label: '모두 선택' }
        ])
      ]
    },

    // View menu
    {
      label: '보기',
      submenu: [
        { role: 'reload', label: '다시 로드' },
        { role: 'forceReload', label: '강제 다시 로드' },
        { role: 'toggleDevTools', label: '개발자 도구' },
        { type: 'separator' },
        { role: 'resetZoom', label: '실제 크기' },
        { role: 'zoomIn', label: '확대' },
        { role: 'zoomOut', label: '축소' },
        { type: 'separator' },
        { role: 'togglefullscreen', label: '전체 화면' }
      ]
    },

    // Window menu
    {
      label: '창',
      submenu: [
        { role: 'minimize', label: '최소화' },
        { role: 'zoom', label: '확대/축소' },
        ...(isMac ? [
          { type: 'separator' },
          { role: 'front', label: '모두 앞으로 가져오기' },
          { type: 'separator' },
          { role: 'window', label: '창' }
        ] : [
          { role: 'close', label: '닫기' }
        ])
      ]
    },

    // Help menu
    {
      label: '도움말',
      role: 'help',
      submenu: [
        {
          label: 'RecordRoute 문서',
          click: async () => {
            const docPath = path.join(projectRoot, 'README.md');
            if (fs.existsSync(docPath)) {
              await shell.openPath(docPath);
            }
          }
        },
        {
          label: '프로젝트 GitHub',
          click: async () => {
            await shell.openExternal('https://github.com/john33fiao/RecordRoute');
          }
        },
        { type: 'separator' },
        {
          label: '업데이트 확인',
          click: () => {
            if (isDev) {
              dialog.showMessageBox(mainWindow, {
                type: 'info',
                title: '개발 모드',
                message: '개발 모드에서는 업데이트를 확인할 수 없습니다.',
                buttons: ['확인']
              });
            } else if (autoUpdater) {
              autoUpdater.checkForUpdates();
            }
          }
        },
        { type: 'separator' },
        {
          label: 'RecordRoute 정보',
          click: () => {
            dialog.showMessageBox(mainWindow, {
              type: 'info',
              title: 'RecordRoute 정보',
              message: 'RecordRoute',
              detail: `버전: ${app.getVersion()}\n\n음성, 영상, PDF 파일을 텍스트로 변환하고 회의록으로 요약하는 통합 워크플로우 시스템입니다.\n\nPowered by Rust + Whisper.cpp + Ollama`,
              buttons: ['확인']
            });
          }
        }
      ]
    }
  ];

  const menu = Menu.buildFromTemplate(template);
  Menu.setApplicationMenu(menu);
}

/**
 * Configure auto-updater
 * Checks for updates and notifies the user
 */
function setupAutoUpdater() {
  // Only check for updates in production
  if (isDev) {
    console.log('Auto-updater disabled in development mode');
    return;
  }

  // Load electron-updater only in production mode
  autoUpdater = require('electron-updater').autoUpdater;

  // Configure auto-updater
  autoUpdater.logger = console;
  autoUpdater.autoDownload = false; // Don't auto-download, ask user first
  autoUpdater.autoInstallOnAppQuit = true;

  // Check for updates when app starts
  autoUpdater.checkForUpdates();

  // When update is available
  autoUpdater.on('update-available', (info) => {
    console.log('Update available:', info);

    dialog.showMessageBox(mainWindow, {
      type: 'info',
      title: '업데이트 사용 가능',
      message: `새로운 버전 ${info.version}이 사용 가능합니다.`,
      detail: '지금 다운로드하시겠습니까?',
      buttons: ['다운로드', '나중에'],
      defaultId: 0,
      cancelId: 1
    }).then((result) => {
      if (result.response === 0) {
        autoUpdater.downloadUpdate();
      }
    });
  });

  // When update is not available
  autoUpdater.on('update-not-available', (info) => {
    console.log('Update not available:', info);
  });

  // Download progress
  autoUpdater.on('download-progress', (progressObj) => {
    const message = `다운로드 속도: ${progressObj.bytesPerSecond} - 다운로드됨: ${progressObj.percent}%`;
    console.log(message);

    // Optionally show progress in window title or status bar
    if (mainWindow) {
      mainWindow.setTitle(`RecordRoute - 업데이트 다운로드 중... ${Math.round(progressObj.percent)}%`);
    }
  });

  // When update is downloaded
  autoUpdater.on('update-downloaded', (info) => {
    console.log('Update downloaded:', info);

    // Reset window title
    if (mainWindow) {
      mainWindow.setTitle('RecordRoute');
    }

    dialog.showMessageBox(mainWindow, {
      type: 'info',
      title: '업데이트 준비 완료',
      message: '업데이트가 다운로드되었습니다.',
      detail: '앱을 다시 시작하여 업데이트를 설치하시겠습니까?',
      buttons: ['다시 시작', '나중에'],
      defaultId: 0,
      cancelId: 1
    }).then((result) => {
      if (result.response === 0) {
        // Quit and install update
        autoUpdater.quitAndInstall(false, true);
      }
    });
  });

  // Error handling
  autoUpdater.on('error', (err) => {
    console.error('Auto-updater error:', err);
  });

  // Check for updates every 4 hours
  setInterval(() => {
    autoUpdater.checkForUpdates();
  }, 4 * 60 * 60 * 1000);
}

// App lifecycle events
app.whenReady().then(() => {
  setupLogging();  // Setup logging to file
  createMenu();  // Create native menu
  setupAutoUpdater();  // Setup auto-updater
  runRustServer();

  app.on('activate', function () {
    // On macOS re-create window when dock icon is clicked
    if (BrowserWindow.getAllWindows().length === 0 && rustProcess) {
      createWindow();
    }
  });
});

app.on('window-all-closed', function () {
  // Kill Rust process when app is closing
  if (rustProcess) {
    console.log('Terminating Rust process...');
    rustProcess.kill();
    rustProcess = null;
  }

  // Quit app when all windows are closed, except on macOS
  if (process.platform !== 'darwin') {
    app.quit();
  }
});

// Ensure cleanup on app quit
app.on('quit', () => {
  if (rustProcess) {
    console.log('Cleaning up Rust process on quit...');
    rustProcess.kill();
    rustProcess = null;
  }

  // Close log stream
  if (logStream) {
    logStream.end();
  }
});
