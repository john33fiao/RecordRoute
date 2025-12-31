# Electron 전환 로드맵

이 문서는 기존 RecordRoute 웹 애플리케이션을 Electron 기반의 데스크톱 애플리케이션으로 전환하기 위한 단계별 계획을 정의합니다.

## ✅ Phase 1: 기본 프로젝트 설정 및 Electron 연동 (완료)

첫 단계는 Electron 프로젝트를 설정하고 기존 프론트엔드를 렌더링하는 것입니다.

### 1.1. Node.js 프로젝트 초기화
프로젝트 루트에서 `package.json` 파일을 생성합니다.

```bash
npm init -y
```

### 1.2. Electron 설치
`electron`을 개발 의존성(devDependency)으로 설치합니다.

```bash
npm install --save-dev electron
```

### 1.3. `main.js` 생성
프로젝트 루트에 Electron 메인 프로세스 역할을 할 `main.js` 파일을 생성합니다.

```javascript
// D:\Cloud\RecordRoute\main.js
const { app, BrowserWindow } = require('electron');
const path = require('path');

function createWindow() {
  const mainWindow = new BrowserWindow({
    width: 1200,
    height: 800,
    webPreferences: {
      // preload: path.join(__dirname, 'preload.js') // 필요시 사용
    }
  });

  // 기존 프론트엔드 파일을 로드합니다.
  mainWindow.loadFile('frontend/upload.html');

  // 개발자 도구를 엽니다. (개발 중에만 사용)
  // mainWindow.webContents.openDevTools();
}

app.whenReady().then(() => {
  createWindow();

  app.on('activate', function () {
    if (BrowserWindow.getAllWindows().length === 0) createWindow();
  });
});

app.on('window-all-closed', function () {
  if (process.platform !== 'darwin') app.quit();
});
```

### 1.4. `package.json` 스크립트 추가
Electron 앱을 쉽게 실행할 수 있도록 `package.json`에 `start` 스크립트를 추가합니다.

```json
// D:\Cloud\RecordRoute\package.json
{
  ...
  "main": "main.js",
  "scripts": {
    "start": "electron .",
    "test": "echo \"Error: no test specified\" && exit 1"
  },
  ...
}
```

### 1.5. 실행 테스트
다음 명령어로 Electron 앱이 정상적으로 실행되고 `upload.html`이 로드되는지 확인합니다. 아직 백엔드 연동 전이므로 기능은 동작하지 않습니다.

```bash
npm start
```

## ✅ Phase 2: Rust 백엔드 연동 (완료)

**주의**: Python 백엔드는 Rust 백엔드로 완전히 대체되었습니다. 아래 내용은 참고용입니다.

Electron 앱이 시작될 때 Rust 서버를 자동으로 실행하고, 앱이 종료될 때 함께 종료되도록 설정합니다.

### 2.1. `main.js`에서 Python 프로세스 실행
`child_process` 모듈을 사용하여 `sttEngine/server.py`를 실행하는 코드를 `main.js`에 추가합니다.

```javascript
// D:\Cloud\RecordRoute\main.js (수정)
const { app, BrowserWindow } = require('electron');
const path = require('path');
const { spawn } = require('child_process');
const fs = require('fs');

let pythonProcess = null;

// 개발 환경인지 패키징된 환경인지 판별
const isDev = !app.isPackaged;

// Python 실행 파일 경로 결정
function getPythonPath() {
  if (isDev) {
    // 개발 환경: 가상환경의 Python 사용
    if (process.platform === 'win32') {
      return path.join(__dirname, 'venv', 'Scripts', 'python.exe');
    } else {
      return path.join(__dirname, 'venv', 'bin', 'python');
    }
  } else {
    // 프로덕션 환경: PyInstaller로 빌드된 실행 파일 사용
    const exeName = process.platform === 'win32' ? 'RecordRouteAPI.exe' : 'RecordRouteAPI';
    return path.join(process.resourcesPath, 'bin', exeName);
  }
}

// server.py 경로 결정
function getServerPath() {
  if (isDev) {
    return path.join(__dirname, 'sttEngine', 'server.py');
  } else {
    // 프로덕션에서는 실행 파일 자체가 서버이므로 null 반환
    return null;
  }
}

function runPythonServer() {
  console.log('Starting Python server...');

  const pythonPath = getPythonPath();
  const serverPath = getServerPath();

  // FFmpeg 경로를 인자로 전달 (Phase 6 대비)
  const ffmpegName = process.platform === 'win32' ? 'ffmpeg.exe' : 'ffmpeg';
  const ffmpegPath = isDev
    ? ffmpegName  // 개발 환경: 시스템 PATH 사용
    : path.join(process.resourcesPath, 'bin', ffmpegName);

  // 모델 디렉토리 경로 전달 (초기화 시 다운로드 위치)
  const modelsPath = isDev
    ? path.join(__dirname, 'models')
    : path.join(app.getPath('userData'), 'models');

  const args = isDev
    ? [serverPath, `--ffmpeg_path=${ffmpegPath}`, `--models_path=${modelsPath}`]
    : [`--ffmpeg_path=${ffmpegPath}`, `--models_path=${modelsPath}`];

  pythonProcess = spawn(pythonPath, args);

  pythonProcess.stdout.on('data', (data) => {
    console.log(`Python: ${data}`);
    // 서버가 준비되었다는 메시지를 감지하면 윈도우 생성
    if (data.toString().includes('Serving HTTP on')) {
      createWindow();
    }
  });

  pythonProcess.stderr.on('data', (data) => {
    console.error(`Python Error: ${data}`);
  });

  pythonProcess.on('error', (err) => {
    console.error('Failed to start Python process:', err);
  });
}

function createWindow() {
  // 윈도우 생성 코드는 동일 (이전 단계 참고)
  // ...
}

app.whenReady().then(runPythonServer);

app.on('window-all-closed', () => {
  // Python 프로세스 종료
  if (pythonProcess) {
    console.log('Killing Python process...');
    pythonProcess.kill();
  }
  if (process.platform !== 'darwin') {
    app.quit();
  }
});
```
*이 코드는 개발/프로덕션 환경을 자동 감지하고, 플랫폼별 실행 파일 확장자를 올바르게 처리합니다.*

### 2.2. 전체 기능 테스트
`npm start`로 앱을 실행하고, 파일 업로드 및 처리 기능이 기존 웹 버전과 동일하게 동작하는지 확인합니다.

## ✅ Phase 3: 애플리케이션 패키징 (완료)

`electron-builder`를 사용하여 Windows용 `.exe` 파일과 macOS용 `.app` 파일을 생성합니다.

**업데이트**: Rust 바이너리 번들링으로 전환 완료

### 3.1. `electron-builder` 설치

```bash
npm install --save-dev electron-builder
```

### 3.2. `package.json` 빌드 설정 추가
패키징 설정을 `package.json`에 추가합니다. Python 백엔드를 함께 패키징하기 위한 설정이 중요합니다.

**방법 1: Python 스크립트 포함 (사용자가 Python 설치 필요)**
가장 간단한 방법이지만, 사용자가 `ffmpeg`과 호환되는 버전의 Python을 직접 설치해야 합니다.

```json
// D:\Cloud\RecordRoute\package.json
{
  ...
  "build": {
    "appId": "com.example.recordroute",
    "productName": "RecordRoute",
    "files": [
      "main.js",
      "frontend/**/*",
      "sttEngine/**/*",
      "!sttEngine/venv/**/*" // 가상환경은 제외
    ],
    "win": {
      "target": "nsis"
    },
    "mac": {
      "target": "dmg"
    }
  },
  "scripts": {
    "start": "electron .",
    "pack": "electron-builder --dir",
    "dist": "electron-builder"
  },
  ...
}
```

**방법 2: PyInstaller로 Python 실행 파일 생성 (권장)**
`PyInstaller`를 사용해 Python 프로젝트 전체를 하나의 실행 파일로 만듭니다. 이 파일을 Electron 앱에 포함시키면, 사용자는 Python을 설치할 필요가 없습니다.

1.  **PyInstaller 설치:** `pip install pyinstaller`
2.  **`.spec` 파일 생성 및 수정:**
    ```bash
    cd sttEngine
    pyinstaller --name RecordRouteAPI server.py
    ```
    생성된 `RecordRouteAPI.spec` 파일을 열어 다음과 같이 수정합니다:

    ```python
    # RecordRouteAPI.spec
    # -*- mode: python ; coding: utf-8 -*-

    block_cipher = None

    a = Analysis(
        ['server.py'],
        pathex=[],
        binaries=[],
        datas=[
            ('workflow', 'workflow'),  # workflow 디렉토리 포함
            ('config.py', '.'),
        ],
        hiddenimports=[
            'whisper',
            'llama_cpp',
            'sentence_transformers',
            'multipart',
        ],
        hookspath=[],
        hooksconfig={},
        runtime_hooks=[],
        excludes=[],
        win_no_prefer_redirects=False,
        win_private_assemblies=False,
        cipher=block_cipher,
        noarchive=False,
    )

    pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

    exe = EXE(
        pyz,
        a.scripts,
        a.binaries,
        a.zipfiles,
        a.datas,
        [],
        name='RecordRouteAPI',
        debug=False,  # 배포 시 False, 개발 시 True
        bootloader_ignore_signals=False,
        strip=False,
        upx=True,
        upx_exclude=[],
        runtime_tmpdir=None,
        console=True,  # 배포 시 False로 변경 (개발 중에는 True로 디버깅)
        disable_windowed_traceback=False,
        argv_emulation=False,
        target_arch=None,
        codesign_identity=None,
        entitlements_file=None,
    )
    ```

3.  **Python 앱 빌드:**
    ```bash
    pyinstaller RecordRouteAPI.spec
    ```
4.  생성된 `dist/RecordRouteAPI.exe`(Windows) 또는 `dist/RecordRouteAPI`(macOS/Linux) 파일을 프로젝트 루트의 `bin/` 디렉토리로 복사합니다.
5.  `package.json`의 `build.extraResources`에 해당 파일을 포함시킵니다 (Phase 2.1에서 이미 구현됨).

### 3.3. 빌드 실행
다음 명령어로 현재 OS에 맞는 배포용 패키지를 생성합니다.

```bash
npm run dist
```
빌드가 완료되면 `dist` 폴더에 설치 파일이 생성됩니다.

### 3.4. 모델 부트스트래핑 (초기화 시 다운로드)
Whisper 모델과 LLM 모델은 크기가 매우 크므로(합계 10GB+), 애플리케이션 패키지에 포함시키지 않고 첫 실행 시 자동으로 다운로드하는 방식을 사용합니다.

1.  **부트스트래핑 UI 추가:**
    `frontend/` 디렉토리에 `bootstrap.html` 파일을 생성하여 초기화 진행 상황을 표시합니다.

    ```html
    <!-- frontend/bootstrap.html -->
    <!DOCTYPE html>
    <html>
    <head>
        <title>RecordRoute 초기화</title>
        <style>
            body {
                font-family: Arial, sans-serif;
                text-align: center;
                padding: 50px;
                background: #f5f5f5;
            }
            .progress {
                width: 80%;
                margin: 20px auto;
                background: #ddd;
                border-radius: 5px;
                overflow: hidden;
            }
            .progress-bar {
                height: 30px;
                background: #4CAF50;
                width: 0%;
                transition: width 0.3s;
            }
        </style>
    </head>
    <body>
        <h1>RecordRoute 초기 설정</h1>
        <p id="status">필요한 모델을 다운로드하는 중입니다...</p>
        <div class="progress">
            <div class="progress-bar" id="progressBar"></div>
        </div>
        <p id="details"></p>
        <script>
            // /bootstrap_status 엔드포인트를 폴링하여 진행 상황 업데이트
            setInterval(async () => {
                const res = await fetch('/bootstrap_status');
                const data = await res.json();
                document.getElementById('status').textContent = data.message;
                document.getElementById('progressBar').style.width = data.progress + '%';
                document.getElementById('details').textContent = data.current_task || '';

                if (data.complete) {
                    window.location.href = '/upload.html';
                }
            }, 1000);
        </script>
    </body>
    </html>
    ```

2.  **Python 부트스트래핑 로직 추가:**
    `sttEngine/bootstrap.py` 파일을 생성하여 모델 다운로드 로직을 구현합니다.

    ```python
    # sttEngine/bootstrap.py
    import os
    import whisper
    import subprocess
    from pathlib import Path

    class BootstrapManager:
        def __init__(self, models_path, progress_callback=None):
            self.models_path = Path(models_path)
            self.models_path.mkdir(parents=True, exist_ok=True)
            self.progress_callback = progress_callback or (lambda x: None)

        def check_whisper_model(self, model_name="large-v3"):
            """Whisper 모델 존재 여부 확인"""
            cache_dir = self.models_path / "whisper"
            model_file = cache_dir / f"{model_name}.pt"
            return model_file.exists()

        def download_whisper_model(self, model_name="large-v3"):
            """Whisper 모델 다운로드"""
            self.progress_callback({"message": "Whisper 모델 다운로드 중...", "progress": 10})

            # WHISPER_CACHE_DIR 환경변수 설정
            cache_dir = self.models_path / "whisper"
            os.environ['WHISPER_CACHE_DIR'] = str(cache_dir)

            # whisper.load_model()을 호출하면 자동으로 다운로드됨
            whisper.load_model(model_name, download_root=str(cache_dir))

            self.progress_callback({"message": "Whisper 모델 다운로드 완료", "progress": 40})

        def check_llm_model(self, model_name):
            """LLM 모델 존재 여부 확인"""
            model_file = self.models_path / "llm" / f"{model_name}.gguf"
            return model_file.exists()

        def download_llm_model(self, model_name, hf_repo):
            """Hugging Face에서 LLM 모델 다운로드"""
            from huggingface_hub import hf_hub_download

            self.progress_callback({"message": f"LLM 모델 {model_name} 다운로드 중...", "progress": 50})

            llm_dir = self.models_path / "llm"
            llm_dir.mkdir(parents=True, exist_ok=True)

            # Hugging Face에서 GGUF 파일 다운로드
            hf_hub_download(
                repo_id=hf_repo,
                filename=f"{model_name}.gguf",
                local_dir=str(llm_dir),
                local_dir_use_symlinks=False
            )

            self.progress_callback({"message": f"LLM 모델 다운로드 완료", "progress": 90})

        def run_bootstrap(self):
            """전체 부트스트래핑 프로세스 실행"""
            self.progress_callback({"message": "초기화 시작...", "progress": 0})

            # Whisper 모델 확인 및 다운로드
            if not self.check_whisper_model():
                self.download_whisper_model()
            else:
                self.progress_callback({"message": "Whisper 모델 이미 존재", "progress": 40})

            # LLM 모델 확인 및 다운로드 (예: gemma3:4b)
            if not self.check_llm_model("gemma-3-4b-it-Q4_K_M"):
                self.download_llm_model(
                    "gemma-3-4b-it-Q4_K_M",
                    "lmstudio-community/gemma-2-9b-it-GGUF"
                )
            else:
                self.progress_callback({"message": "LLM 모델 이미 존재", "progress": 90})

            self.progress_callback({"message": "초기화 완료!", "progress": 100, "complete": True})
    ```

3.  **`server.py`에 부트스트래핑 엔드포인트 추가:**
    서버 시작 시 모델 존재 여부를 확인하고, 없으면 부트스트래핑을 실행합니다.

    ```python
    # sttEngine/server.py에 추가
    from bootstrap import BootstrapManager
    import threading

    bootstrap_status = {"message": "준비 중...", "progress": 0, "complete": False}

    def run_bootstrap_if_needed(models_path):
        """백그라운드에서 부트스트래핑 실행"""
        global bootstrap_status

        def update_progress(status):
            global bootstrap_status
            bootstrap_status.update(status)

        manager = BootstrapManager(models_path, progress_callback=update_progress)
        manager.run_bootstrap()

    # 서버 시작 시 부트스트래핑 스레드 실행
    # bootstrap_thread = threading.Thread(target=run_bootstrap_if_needed, args=(models_path,))
    # bootstrap_thread.start()
    ```

4.  **Python 코드에서 모델 경로 사용:**
    `transcribe.py`와 다른 워크플로우에서 환경변수를 통해 모델 경로를 참조합니다.

    ```python
    # sttEngine/workflow/transcribe.py
    import os
    import whisper

    # main.js에서 전달받은 models_path 사용
    models_path = os.environ.get('MODELS_PATH', os.path.expanduser('~/.cache/recordroute/models'))
    whisper_cache = os.path.join(models_path, 'whisper')

    # Whisper에 캐시 디렉토리 알려주기
    os.environ['WHISPER_CACHE_DIR'] = whisper_cache

    # 모델 로드 (자동으로 캐시 디렉토리 사용)
    model = whisper.load_model("large-v3", download_root=whisper_cache, device=device)
    ```

**장점:**
- 애플리케이션 패키지 크기를 10GB+ 줄임 (100MB 이하로 유지)
- 사용자가 필요한 모델만 선택적으로 다운로드 가능
- 업데이트 시 모델을 다시 다운로드할 필요 없음 (사용자 데이터 디렉토리에 저장)



## Phase 4: 후속 개선 사항 (진행 예정)
- [ ] **네이티브 메뉴 추가:** "파일", "편집" 등의 상단 메뉴를 추가하여 기능 접근성 향상.
- [ ] **아이콘 설정:** 애플리케이션 아이콘을 지정.
- [ ] **코드 서명:** 배포 시 신뢰할 수 있는 앱으로 인식되도록 macOS 및 Windows 코드 서명 설정.
- [ ] **자동 업데이트:** `electron-updater`를 연동하여 새 버전 자동 업데이트 기능 구현.
- ✅ **Rust 의존성 관리:** Rust 단일 바이너리로 전환 완료 (Python 의존성 제거)

## ✅ Phase 5: LLM 엔진 (Ollama 기반 유지) - 완료

**업데이트**: Rust 백엔드에서는 Ollama API를 사용하는 방식을 유지하기로 결정했습니다.
- llama.cpp 직접 통합 대신 Ollama HTTP API를 통한 연동
- 사용자가 Ollama 서비스를 별도로 실행해야 하지만, 더 안정적이고 유연한 구조
- Rust reqwest 기반 HTTP 클라이언트 구현 완료

### 5.1. 의존성 변경
- **`ollama` 라이브러리 제거:** `sttEngine/requirements.txt`에서 `ollama`를 삭제합니다.
- **`llama-cpp-python` 설치:** `llama.cpp`의 Python 바인딩인 `llama-cpp-python` 라이브러리를 추가합니다. 이 라이브러리는 C++ 컴파일러가 필요할 수 있으나, pre-built wheel을 사용하면 컴파일 과정을 생략할 수 있습니다.

```bash
# sttEngine/venv/Scripts/activate
pip uninstall ollama
pip install llama-cpp-python
# requirements.txt 업데이트
pip freeze > requirements.txt
```

### 5.2. 코드 리팩토링
`Ollama` API를 호출하던 부분을 `llama-cpp-python`을 직접 호출하는 코드로 변경합니다.

1.  **`llamacpp_utils.py` 생성:** `sttEngine` 내에 `llama.cpp` 모델 로딩 및 추론을 전담하는 유틸리티 모듈을 새로 만듭니다. 이 모듈은 GGUF 형식의 모델 파일을 로드하고, 텍스트 생성(교정, 요약) 기능을 제공하는 함수를 포함합니다.

    ```python
    # sttEngine/llamacpp_utils.py
    from llama_cpp import Llama
    import os
    from pathlib import Path

    class LlamaCppEngine:
        def __init__(self, models_path):
            self.models_path = Path(models_path) / "llm"
            self.model = None

        def load_model(self, model_filename, n_ctx=2048, n_gpu_layers=-1):
            """GGUF 모델 로드"""
            model_path = self.models_path / model_filename
            if not model_path.exists():
                raise FileNotFoundError(f"모델 파일을 찾을 수 없습니다: {model_path}")

            self.model = Llama(
                model_path=str(model_path),
                n_ctx=n_ctx,
                n_gpu_layers=n_gpu_layers,  # GPU 가속 (-1 = 모든 레이어)
                verbose=False
            )
            return self.model

        def generate_text(self, prompt, max_tokens=2048, temperature=0.7, stop=None):
            """텍스트 생성"""
            if not self.model:
                raise RuntimeError("먼저 load_model()을 호출하세요")

            response = self.model(
                prompt,
                max_tokens=max_tokens,
                temperature=temperature,
                stop=stop or [],
                echo=False
            )
            return response['choices'][0]['text'].strip()

    # 전역 인스턴스 (싱글톤 패턴)
    _engine = None

    def get_engine(models_path):
        global _engine
        if _engine is None:
            _engine = LlamaCppEngine(models_path)
        return _engine
    ```

2.  **기존 코드 수정:**
    -   `sttEngine/ollama_utils.py`를 제거하거나 `llamacpp_utils.py`로 대체합니다.
    -   **텍스트 생성:** `sttEngine/workflow/correct.py`, `sttEngine/workflow/summarize.py`, `sttEngine/one_line_summary.py` 등에서 Ollama API를 호출하던 부분을 새로운 `llamacpp_utils.py`의 텍스트 생성 함수를 사용하도록 수정합니다.

    ```python
    # sttEngine/workflow/correct.py (예시)
    from llamacpp_utils import get_engine
    import os

    models_path = os.environ.get('MODELS_PATH')
    engine = get_engine(models_path)
    engine.load_model("gemma-3-4b-it-Q4_K_M.gguf")

    # 기존: ollama.chat() 호출
    # response = ollama.chat(model="gemma3:4b", messages=[...])

    # 변경 후: llama.cpp 직접 호출
    prompt = f"다음 텍스트를 교정해주세요:\n\n{text}"
    corrected_text = engine.generate_text(prompt, max_tokens=4096, temperature=0.3)
    ```

    -   **임베딩 생성 (RAG):** `sttEngine/embedding_pipeline.py`는 검증된 `sentence-transformers`를 계속 사용합니다. `llama.cpp`의 임베딩은 품질이 불안정할 수 있으므로, 특화된 임베딩 모델을 사용하는 것이 더 안정적입니다.

    ```python
    # sttEngine/embedding_pipeline.py (기존 유지)
    from sentence_transformers import SentenceTransformer

    # 임베딩 모델은 sentence-transformers 계속 사용
    model = SentenceTransformer('paraphrase-multilingual-mpnet-base-v2')
    embeddings = model.encode(texts)
    ```

    -   API 호출 방식(비동기)에서 직접 함수 호출(동기) 방식으로 로직이 변경됩니다.

### 5.3. 모델 관리
- **GGUF 모델 파일:** `llama.cpp`는 GGUF 형식의 모델 파일을 사용합니다. Phase 3.4에서 구현한 부트스트래핑 시스템이 Hugging Face에서 자동으로 필요한 GGUF 모델을 다운로드합니다.

- **모델 디렉토리:**
  - **개발 환경:** 프로젝트 루트의 `models/` 디렉토리
  - **프로덕션 환경:** `app.getPath('userData')/models/` 디렉토리 (사용자별 데이터 폴더)
  - 구조:
    ```
    models/
    ├── whisper/
    │   └── large-v3.pt
    └── llm/
        ├── gemma-3-4b-it-Q4_K_M.gguf
        └── gemma-3-12b-it-Q4_K_M.gguf
    ```

- **`config.py` 수정:** Ollama 모델 이름 대신 GGUF 모델 파일명을 사용하도록 변경합니다.

    ```python
    # sttEngine/config.py (수정)
    import os
    import platform

    PLATFORM_TYPE = "Windows" if platform.system() == "Windows" else "Unix"
    MODELS_PATH = os.environ.get('MODELS_PATH', os.path.expanduser('~/.cache/recordroute/models'))

    # Ollama 모델 이름 → GGUF 파일명으로 변경
    if PLATFORM_TYPE == "Windows":
        DEFAULT_SUMMARY_MODEL = "gemma-3-4b-it-Q4_K_M.gguf"
        DEFAULT_CORRECT_MODEL = "gemma-3-4b-it-Q4_K_M.gguf"
    else:
        DEFAULT_SUMMARY_MODEL = "gemma-3-12b-it-Q4_K_M.gguf"
        DEFAULT_CORRECT_MODEL = "gemma-3-12b-it-Q4_K_M.gguf"
    ```

### 5.4. 패키징 영향
`llama.cpp` 전환 시 패키징 방식에서 고려해야 할 사항들:

- **모델 파일은 패키지에 포함하지 않음:** Phase 3.4의 부트스트래핑 전략에 따라, GGUF 모델 파일들은 첫 실행 시 다운로드됩니다. 따라서 최종 패키지 크기는 100MB 이하로 유지됩니다.

- **`llama.cpp` 바이너리 포함:** `llama-cpp-python` 라이브러리에 포함된 C++ 바이너리(DLL, so, dylib 등)는 패키징 시 반드시 포함되어야 합니다. PyInstaller `.spec` 파일에서 이미 `hiddenimports`에 `llama_cpp`를 추가했으므로 자동으로 포함됩니다.

- **추가 의존성:** `huggingface-hub` 라이브러리를 `requirements.txt`에 추가하여 모델 다운로드를 지원합니다:
  ```bash
  pip install huggingface-hub
  pip freeze > requirements.txt
  ```

- **PyInstaller `.spec` 파일 업데이트:**
  ```python
  # RecordRouteAPI.spec의 hiddenimports 섹션에 추가
  hiddenimports=[
      'whisper',
      'llama_cpp',
      'sentence_transformers',
      'multipart',
      'huggingface_hub',  # 추가
  ],
  ```

이 단계를 완료하면 RecordRoute는 `ffmpeg`을 제외한 모든 핵심 기능(STT, LLM)을 갖춘 경량 독립 실행형 데스크톱 애플리케이션이 됩니다. 모델은 첫 실행 시 자동으로 다운로드되어 사용자 데이터 디렉토리에 저장됩니다.

## ⏸️ Phase 6: FFmpeg 내장 및 경로 설정 (진행 예정)

사용자가 별도로 `ffmpeg`을 설치할 필요가 없도록, `ffmpeg` 실행 파일을 애플리케이션 패키지에 포함시킵니다.

**현재 상태**: FFmpeg는 시스템 PATH에 설치된 것을 사용 (향후 번들링 예정)

### 6.1. FFmpeg 바이너리 다운로드
- Windows, macOS, Linux 등 타겟 플랫폼에 맞는 `ffmpeg` 정적 빌드(static build)를 다운로드합니다. (예: [gyan.dev for Windows](https://www.gyan.dev/ffmpeg/builds/))
- 다운로드한 압축 파일에서 `ffmpeg.exe`(Windows) 또는 `ffmpeg`(macOS/Linux) 실행 파일을 추출합니다. `ffprobe`도 함께 추출하면 좋습니다.

### 6.2. 프로젝트 내 바이너리 배치
- 프로젝트 루트에 `bin`과 같은 디렉토리를 생성하고, 그 안에 플랫폼별로 `ffmpeg` 실행 파일을 저장합니다.
  ```
  RecordRoute/
  ├── bin/
  │   ├── win/
  │   │   ├── ffmpeg.exe
  │   │   └── ffprobe.exe
  │   └── mac/
  │       ├── ffmpeg
  │       └── ffprobe
  ├── sttEngine/
  ...
  ```

### 6.3. `package.json` 빌드 설정 수정
`electron-builder`가 `bin` 디렉토리를 패키지에 포함하도록 `extraResources` 설정을 추가합니다.

```json
// D:\Cloud\RecordRoute\package.json
{
  ...
  "build": {
    ...
    "extraResources": [
      {
        "from": "bin/${os}",
        "to": "bin",
        "filter": [
          "**/*"
        ]
      }
    ]
    ...
  }
  ...
}
```
*`${os}` 변수는 빌드 환경에 따라 `win`, `mac`, `linux` 등으로 자동 변환됩니다.*

### 6.4. Python 코드에서 FFmpeg 경로 참조
시스템 `PATH`에 의존하는 대신, 패키지 내의 `ffmpeg` 경로를 직접 사용하도록 Python 코드를 수정해야 합니다.

**참고:** `main.js`에서 FFmpeg 경로를 전달하는 로직은 이미 Phase 2.1에서 구현되었습니다.

1.  **`server.py`에서 인자 파싱 추가:**

    ```python
    # sttEngine/server.py (수정)
    import argparse
    import os

    # 명령줄 인자 파싱
    parser = argparse.ArgumentParser()
    parser.add_argument('--ffmpeg_path', type=str, default='ffmpeg', help='FFmpeg 실행 파일 경로')
    parser.add_argument('--models_path', type=str, default=None, help='모델 저장 디렉토리 경로')
    args = parser.parse_args()

    # 환경변수로 설정하여 다른 모듈에서 접근 가능하도록
    os.environ['FFMPEG_PATH'] = args.ffmpeg_path
    if args.models_path:
        os.environ['MODELS_PATH'] = args.models_path
    ```

2.  **`config.py`에 FFmpeg 경로 추가:**

    ```python
    # sttEngine/config.py (수정)
    import os
    import platform

    # 기존 설정들...
    PLATFORM_TYPE = "Windows" if platform.system() == "Windows" else "Unix"

    # FFmpeg 경로 (환경변수에서 가져오기, 기본값은 시스템 PATH)
    FFMPEG_PATH = os.environ.get('FFMPEG_PATH', 'ffmpeg')
    FFPROBE_PATH = FFMPEG_PATH.replace('ffmpeg', 'ffprobe')

    # 모델 경로
    MODELS_PATH = os.environ.get('MODELS_PATH', os.path.expanduser('~/.cache/recordroute/models'))
    ```

3.  **`transcribe.py`에서 설정 사용:**

    ```python
    # sttEngine/workflow/transcribe.py (수정)
    import subprocess
    from pathlib import Path
    import config

    def get_audio_duration(file_path: Path):
        """Get audio file duration using ffprobe."""
        try:
            result = subprocess.run([
                config.FFPROBE_PATH,
                '-v', 'quiet',
                '-show_entries', 'format=duration',
                '-of', 'default=noprint_wrappers=1:nokey=1',
                str(file_path)
            ], capture_output=True, text=True, check=True)
            return float(result.stdout.strip())
        except FileNotFoundError:
            print(f"오류: '{config.FFPROBE_PATH}'를 찾을 수 없습니다.")
            return None
        except subprocess.CalledProcessError as e:
            print(f"FFprobe 실행 오류: {e}")
            return None

    def convert_to_wav(input_path: Path, output_path: Path):
        """Convert audio file to WAV format using FFmpeg."""
        try:
            subprocess.run([
                config.FFMPEG_PATH,
                '-i', str(input_path),
                '-ar', '16000',
                '-ac', '1',
                '-c:a', 'pcm_s16le',
                str(output_path)
            ], check=True, capture_output=True)
            return True
        except FileNotFoundError:
            print(f"오류: '{config.FFMPEG_PATH}'를 찾을 수 없습니다.")
            return False
        except subprocess.CalledProcessError as e:
            print(f"FFmpeg 변환 오류: {e}")
            return False
    ```

이 단계를 통해 `ffmpeg`까지 내장되어, 외부 의존성이 거의 없는 완벽한 독립 실행형 애플리케이션을 배포할 수 있게 됩니다.


