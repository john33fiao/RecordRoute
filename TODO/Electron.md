# Electron 전환 로드맵

이 문서는 기존 RecordRoute 웹 애플리케이션을 Electron 기반의 데스크톱 애플리케이션으로 전환하기 위한 단계별 계획을 정의합니다.

## Phase 1: 기본 프로젝트 설정 및 Electron 연동

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

## Phase 2: Python 백엔드 연동

Electron 앱이 시작될 때 Python 서버를 자동으로 실행하고, 앱이 종료될 때 함께 종료되도록 설정합니다.

### 2.1. `main.js`에서 Python 프로세스 실행
`child_process` 모듈을 사용하여 `sttEngine/server.py`를 실행하는 코드를 `main.js`에 추가합니다.

```javascript
// D:\Cloud\RecordRoute\main.js (수정)
const { app, BrowserWindow } = require('electron');
const path = require('path');
const { spawn } = require('child_process');

let pythonProcess = null;
const PY_SERVER_PATH = path.join(__dirname, 'sttEngine', 'server.py');
const VENV_PYTHON_PATH = path.join(__dirname, 'venv', 'Scripts', 'python.exe');

function runPythonServer() {
  console.log('Starting Python server...');
  // 가상환경의 Python으로 server.py 실행
  pythonProcess = spawn(VENV_PYTHON_PATH, [PY_SERVER_PATH]);

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
*참고: 위 코드는 Windows 기준(`venv\Scripts\python.exe`)입니다. macOS/Linux의 경우 `venv/bin/python`으로 경로를 수정해야.*

### 2.2. 전체 기능 테스트
`npm start`로 앱을 실행하고, 파일 업로드 및 처리 기능이 기존 웹 버전과 동일하게 동작하는지 확인합니다.

## Phase 3: 애플리케이션 패키징

`electron-builder`를 사용하여 Windows용 `.exe` 파일과 macOS용 `.app` 파일을 생성합니다.

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
2.  **Python 앱 빌드:**
    ```bash
    cd sttEngine
    pyinstaller --name RecordRouteAPI --onefile --noconsole server.py
    ```
3.  생성된 `dist/RecordRouteAPI.exe` 파일을 프로젝트의 특정 위치(예: `bin/`)로 복사합니다.
4.  `main.js`에서 `spawn` 대상을 이 실행 파일로 변경하고, `package.json`의 `build.files`에 해당 파일을 포함시킵니다.

### 3.3. 빌드 실행
다음 명령어로 현재 OS에 맞는 배포용 패키지를 생성합니다.

```bash
npm run dist
```
빌드가 완료되면 `dist` 폴더에 설치 파일이 생성됩니다.

### 3.4. Whisper 모델 내장
기본적으로 Whisper는 모델을 중앙 캐시 폴더에 다운로드하지만, 완전한 독립 실행형 앱을 위해 모델 파일을 패키지에 직접 포함시킵니다.

1.  **모델 파일 준비:**
    - `large-v3` 등 필요한 Whisper 모델(`.pt` 파일)을 수동으로 다운로드합니다. (`whisper` 라이브러리가 다운로드하는 URL을 확인하거나 Hugging Face 등에서 찾을 수 있습니다.)
    - 프로젝트 내에 `models/whisper` 폴더를 만들고 다운로드한 `.pt` 파일을 저장합니다.

2.  **`package.json` 설정 추가:**
    `extraResources`에 `models/whisper` 폴더를 추가하여 빌드 시 포함되도록 합니다.

    ```json
    // D:\Cloud\RecordRoute\package.json
    {
      ...
      "build": {
        ...
        "extraResources": [
          ...,
          {
            "from": "models/whisper",
            "to": "models/whisper"
          }
        ]
        ...
      }
      ...
    }
    ```

3.  **Python 코드 수정:**
    `transcribe.py`에서 모델을 로드하는 방식을 이름 기반에서 경로 기반으로 변경합니다. `main.js`에서와 마찬가지로, 리소스 경로를 Python 스크립트에 인자로 전달하고 해당 경로를 사용합니다.

    ```python
    # sttEngine/workflow/transcribe.py (예시)
    
    # --model_dir_path 인자를 받아 model_dir 변수에 저장했다고 가정
    # model_dir = config.WHISPER_MODELS_PATH 
    
    model_path = os.path.join(model_dir, 'large-v3.pt')
    
    # 모델 로드 방식을 이름에서 파일 경로로 변경
    # model = whisper.load_model("large-v3", device=device) # 변경 전
    model = whisper.load_model(model_path, device=device) # 변경 후
    ```



## Phase 4: 후속 개선 사항
- **네이티브 메뉴 추가:** "파일", "편집" 등의 상단 메뉴를 추가하여 기능 접근성 향상.
- **아이콘 설정:** 애플리케이션 아이콘을 지정.
- **코드 서명:** 배포 시 신뢰할 수 있는 앱으로 인식되도록 macOS 및 Windows 코드 서명 설정.
- **자동 업데이트:** `electron-updater`를 연동하여 새 버전 자동 업데이트 기능 구현.
- **Python 의존성 관리:** `PyInstaller` 사용 시, `whisper` 모델 파일 등 데이터 파일이 누락되지 않도록 `.spec` 파일을 상세히 설정.

## Phase 5: LLM 엔진 전환 (Ollama → llama.cpp)

현재 `Ollama` 서비스에 의존하는 구조에서 벗어나, `llama.cpp`를 직접 연동하여 LLM 추론 기능을 애플리케이션에 내장합니다. 이를 통해 사용자는 별도의 LLM 서비스를 설치하고 실행할 필요가 없어지며, 완전한 독립 실행형(self-contained) 애플리케이션을 만들 수 있습니다.

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

1.  **`llamacpp_utils.py` 생성:** `sttEngine` 내에 `llama.cpp` 모델 로딩 및 추론을 전담하는 유틸리티 모듈을 새로 만듭니다. 이 모듈은 GGUF 형식의 모델 파일을 로드하고, 텍스트 생성(교정, 요약) 및 **임베딩 생성** 기능을 제공하는 함수를 포함합니다.

2.  **기존 코드 수정:**
    -   `sttEngine/ollama_utils.py`를 제거하거나 `llamacpp_utils.py`로 대체합니다.
    -   **텍스트 생성:** `sttEngine/workflow/correct.py`, `sttEngine/workflow/summarize.py`, `sttEngine/one_line_summary.py` 등에서 Ollama API를 호출하던 부분을 새로운 `llamacpp_utils.py`의 텍스트 생성 함수를 사용하도록 수정합니다.
    -   **임베딩 생성 (RAG):** `sttEngine/embedding_pipeline.py`에서 `embed_text_ollama` 함수를 `llama-cpp-python`의 임베딩 기능을 사용하는 새로운 함수로 교체합니다. `llama-cpp-python` 라이브러리는 `Llama.embed()` 메소드를 통해 로컬에서 직접 임베딩 생성을 지원합니다.
    -   API 호출 방식(비동기)에서 직접 함수 호출(동기) 방식으로 로직이 변경됩니다.

### 5.3. 모델 관리
- **GGUF 모델 파일:** `llama.cpp`는 GGUF 형식의 모델 파일을 사용합니다. Hugging Face 등에서 필요한 모델(예: Gemma, Llama3)의 GGUF 파일을 다운로드해야 합니다.
- **모델 디렉토리:** 프로젝트 내에 `models`와 같은 디렉토리를 만들어 다운로드한 GGUF 파일들을 저장합니다.
- **`config.py` 수정:** `sttEngine/config.py`를 수정하여 Ollama 모델 이름 대신 로컬 GGUF 모델 파일의 경로를 사용하도록 변경합니다.

### 5.4. 패키징 영향
`llama.cpp` 전환 시 패키징 방식에 가장 큰 변화가 생깁니다.

- **모델 파일 포함:** `electron-builder`가 최종 패키지에 GGUF 모델 파일들을 포함하도록 설정해야 합니다. 모델 파일은 크기가 매우 크기 때문에(수 GB), 애플리케이션의 전체 용량이 크게 증가하는 점을 감안해야 합니다. (`package.json`의 `build.extraResources` 설정 사용)
- **`llama.cpp` 바이너리:** `llama-cpp-python` 라이브러리에 포함된 C++ 바이너리(DLL, so, dylib 등)가 패키징 시 누락되지 않도록 주의해야 합니다. `PyInstaller` 사용 시 `.spec` 파일에 관련 바이너리를 명시적으로 포함시켜야 할 수 있습니다.

이 단계를 완료하면 RecordRoute는 `ffmpeg`을 제외한 모든 핵심 기능(STT, LLM)을 내장한 완전한 독립 실행형 데스크톱 애플리케이션이 됩니다.

## Phase 6: FFmpeg 내장 및 경로 설정

사용자가 별도로 `ffmpeg`을 설치할 필요가 없도록, `ffmpeg` 실행 파일을 애플리케이션 패키지에 포함시킵니다.

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

1.  **`main.js`에서 리소스 경로 전달:** Electron의 `main.js`에서 Python 자식 프로세스를 생성할 때, 리소스 경로를 인자(argument)로 전달합니다.

    ```javascript
    // D:\Cloud\RecordRoute\main.js (수정)
    // ...
    function runPythonServer() {
      const resourcesPath = path.join(app.getAppPath(), '..'); // 패키징되었을 때의 리소스 루트 경로
      const ffmpegPath = path.join(resourcesPath, 'bin', 'ffmpeg.exe'); // Windows 예시

      // Python 서버 실행 시 ffmpeg 경로를 인자로 전달
      pythonProcess = spawn(VENV_PYTHON_PATH, [
        PY_SERVER_PATH,
        `--ffmpeg_path=${ffmpegPath}`
      ]);
      // ...
    }
    ```
    *실제 구현 시에는 `process.resourcesPath`를 사용하거나, 개발 환경과 배포 환경을 구분하는 로직이 필요합니다.*

2.  **Python 코드에서 인자 파싱 및 사용:** `sttEngine/server.py` 또는 `workflow/transcribe.py`에서 `ffmpeg`을 호출하는 부분을 수정합니다.

    ```python
    # sttEngine/workflow/transcribe.py (예시)
    import subprocess
    import sys

    # server.py 등에서 argparse를 통해 --ffmpeg_path 인자를 받아 전역 변수나 설정으로 저장했다고 가정
    # ffmpeg_path = config.FFMPEG_PATH 
    
    def get_audio_duration(file_path: Path, ffmpeg_path: str = 'ffmpeg'):
        """Get audio file duration using ffprobe."""
        ffprobe_path = ffmpeg_path.replace('ffmpeg', 'ffprobe')
        try:
            result = subprocess.run([
                ffprobe_path, '-v', 'quiet', ... , str(file_path)
            ], ...)
            # ...
        except FileNotFoundError:
            # ffmpeg/ffprobe를 찾을 수 없을 때의 에러 처리
            print(f"'{ffprobe_path}'를 찾을 수 없습니다. PATH를 확인하거나 직접 경로를 지정해주세요.")
            return None
    ```

이 단계를 통해 `ffmpeg`까지 내장되어, 외부 의존성이 거의 없는 완벽한 독립 실행형 애플리케이션을 배포할 수 있게 됩니다.


