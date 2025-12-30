# FFmpeg Binaries

이 디렉토리에는 RecordRoute가 사용하는 FFmpeg 바이너리가 위치합니다.

## 개발 환경 설정

개발 환경에서는 시스템에 설치된 FFmpeg를 사용합니다. FFmpeg가 PATH에 등록되어 있어야 합니다.

### Windows
```bash
# Chocolatey 사용
choco install ffmpeg

# 또는 공식 사이트에서 다운로드
# https://ffmpeg.org/download.html#build-windows
```

### macOS
```bash
# Homebrew 사용
brew install ffmpeg
```

### Linux
```bash
# Ubuntu/Debian
sudo apt-get install ffmpeg

# Fedora
sudo dnf install ffmpeg

# Arch Linux
sudo pacman -S ffmpeg
```

## 프로덕션 빌드 설정

프로덕션 빌드를 위해서는 플랫폼별 FFmpeg 바이너리를 이 디렉토리에 배치해야 합니다.

### 디렉토리 구조
```
bin/ffmpeg/
├── win32/
│   └── ffmpeg.exe
├── darwin/
│   └── ffmpeg
└── linux/
    └── ffmpeg
```

### FFmpeg 다운로드

#### Windows (win32)
1. https://github.com/BtbN/FFmpeg-Builds/releases 에서 최신 `ffmpeg-master-latest-win64-gpl.zip` 다운로드
2. 압축 해제 후 `bin/ffmpeg.exe`를 `bin/ffmpeg/win32/ffmpeg.exe`로 복사

#### macOS (darwin)
1. Homebrew로 FFmpeg 설치: `brew install ffmpeg`
2. FFmpeg 바이너리 위치 확인: `which ffmpeg` (보통 `/opt/homebrew/bin/ffmpeg` 또는 `/usr/local/bin/ffmpeg`)
3. 바이너리를 `bin/ffmpeg/darwin/ffmpeg`로 복사

또는 static build 다운로드:
1. https://evermeet.cx/ffmpeg/ 에서 최신 버전 다운로드
2. `bin/ffmpeg/darwin/ffmpeg`로 복사

#### Linux
1. Static build 다운로드: https://johnvansickle.com/ffmpeg/
2. 압축 해제 후 `ffmpeg` 바이너리를 `bin/ffmpeg/linux/ffmpeg`로 복사

### 실행 권한 설정 (macOS/Linux)
```bash
chmod +x bin/ffmpeg/darwin/ffmpeg
chmod +x bin/ffmpeg/linux/ffmpeg
```

## 빌드 프로세스

`build-backend.sh` 또는 `build-all.sh` 스크립트 실행 시, 플랫폼에 맞는 FFmpeg 바이너리가 자동으로 `bin/` 디렉토리로 복사됩니다.

Electron 빌드 시 `package.json`의 `extraResources` 설정에 따라 FFmpeg가 앱에 번들링됩니다.

## 참고사항

- FFmpeg 바이너리는 용량이 크므로 Git 저장소에 포함되지 않습니다 (`.gitignore`에 추가됨)
- 개발자는 각자 플랫폼에 맞는 FFmpeg를 다운로드하여 배치해야 합니다
- CI/CD 환경에서는 빌드 스크립트에서 자동으로 다운로드하도록 설정할 수 있습니다
