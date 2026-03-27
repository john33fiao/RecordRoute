@echo off
setlocal EnableExtensions EnableDelayedExpansion
goto :main

:detect_arch
set "machine=%PROCESSOR_ARCHITECTURE%"
if defined PROCESSOR_ARCHITEW6432 set "machine=%PROCESSOR_ARCHITEW6432%"
set "platform_arch="
if /I "%machine%"=="AMD64" set "platform_arch=x86_64"
if /I "%machine%"=="ARM64" set "platform_arch=aarch64"
if not defined platform_arch set "platform_arch=%machine%"
exit /b 0

:ensure_msvc_env
if not defined VSCMD_ARG_TGT_ARCH call :load_vsdevcmd
if errorlevel 1 exit /b %errorlevel%
if not defined LIB call :load_vsdevcmd
if errorlevel 1 exit /b %errorlevel%
if not defined INCLUDE call :load_vsdevcmd
if errorlevel 1 exit /b %errorlevel%
where cl.exe >nul 2>nul
if errorlevel 1 call :load_vsdevcmd
if errorlevel 1 exit /b %errorlevel%
where nmake.exe >nul 2>nul
if errorlevel 1 call :load_vsdevcmd
if errorlevel 1 exit /b %errorlevel%
where rc.exe >nul 2>nul
if errorlevel 1 call :ensure_windows_sdk_bin
where mt.exe >nul 2>nul
if errorlevel 1 call :ensure_windows_sdk_bin
where cl.exe >nul 2>nul
if errorlevel 1 (
  >&2 echo cl.exe not found. Install Visual Studio C++ tools or run from a Developer Command Prompt.
  exit /b 1
)
where nmake.exe >nul 2>nul
if errorlevel 1 (
  >&2 echo nmake.exe not found. Install Visual Studio C++ tools or run from a Developer Command Prompt.
  exit /b 1
)
where rc.exe >nul 2>nul
if errorlevel 1 (
  >&2 echo rc.exe not found. Install the Windows SDK or the Desktop development with C++ workload.
  exit /b 1
)
where mt.exe >nul 2>nul
if errorlevel 1 (
  >&2 echo mt.exe not found. Install the Windows SDK or the Desktop development with C++ workload.
  exit /b 1
)
exit /b 0

:ensure_windows_sdk_bin
set "sdk_arch=x64"
if /I "%platform_arch%"=="aarch64" set "sdk_arch=arm64"
set "sdk_bin="
set "sdk_root="
set "sdk_version="
for %%R in ("%ProgramFiles(x86)%\Windows Kits\10" "%ProgramFiles%\Windows Kits\10") do (
  if not defined sdk_bin if exist "%%~R\bin" (
    for /f "delims=" %%V in ('dir /b /ad /o-n "%%~R\bin" 2^>nul') do (
      if not defined sdk_bin if exist "%%~R\bin\%%V\%sdk_arch%\rc.exe" if exist "%%~R\bin\%%V\%sdk_arch%\mt.exe" (
        set "sdk_root=%%~R"
        set "sdk_version=%%V"
        set "sdk_bin=%%~R\bin\%%V\%sdk_arch%"
      )
    )
  )
)
if defined sdk_bin (
  set "PATH=!sdk_bin!;!PATH!"
  if exist "!sdk_root!\Lib\!sdk_version!\um\%sdk_arch%\kernel32.lib" set "LIB=!sdk_root!\Lib\!sdk_version!\um\%sdk_arch%;!sdk_root!\Lib\!sdk_version!\ucrt\%sdk_arch%;!LIB!"
  if exist "!sdk_root!\Include\!sdk_version!\ucrt" set "INCLUDE=!sdk_root!\Include\!sdk_version!\ucrt;!sdk_root!\Include\!sdk_version!\um;!sdk_root!\Include\!sdk_version!\shared;!sdk_root!\Include\!sdk_version!\winrt;!sdk_root!\Include\!sdk_version!\cppwinrt;!INCLUDE!"
  set "WindowsSdkDir=!sdk_root!\"
  set "WindowsSDKVersion=!sdk_version!\"
)
exit /b 0

:load_vsdevcmd
if not defined vsdevcmd_path call :resolve_vsdevcmd
if errorlevel 1 exit /b %errorlevel%
if not defined vsdevcmd_path (
  >&2 echo VsDevCmd.bat not found. Install Visual Studio Build Tools or run from a Developer Command Prompt.
  exit /b 1
)
set "vs_arch=x64"
if /I "%platform_arch%"=="aarch64" set "vs_arch=arm64"
call "%vsdevcmd_path%" -arch=%vs_arch% >nul
if errorlevel 1 exit /b %errorlevel%
exit /b 0

:resolve_vsdevcmd
set "vsdevcmd_path="
for %%R in ("%ProgramFiles%" "%ProgramFiles(x86)%") do (
  if not defined vsdevcmd_path if not "%%~R"=="" (
    for %%Y in (2022 2019) do (
      for %%E in (Community Professional Enterprise BuildTools) do (
        if exist "%%~R\Microsoft Visual Studio\%%Y\%%E\Common7\Tools\VsDevCmd.bat" (
          set "vsdevcmd_path=%%~R\Microsoft Visual Studio\%%Y\%%E\Common7\Tools\VsDevCmd.bat"
        )
      )
    )
  )
)
exit /b 0

:find_posix_shell
set "posix_shell="
if defined RECORDROUTE_POSIX_SHELL (
  if exist "%RECORDROUTE_POSIX_SHELL%" (
    "%RECORDROUTE_POSIX_SHELL%" -lc "command -v make >/dev/null 2>&1"
    if not errorlevel 1 set "posix_shell=%RECORDROUTE_POSIX_SHELL%"
  )
)
for %%P in (
  "C:\msys64\usr\bin\bash.exe"
  "C:\msys64\usr\bin\sh.exe"
  "C:\msys64\clang64.exe"
  "C:\msys64\ucrt64.exe"
  "C:\msys64\mingw64.exe"
  "%ProgramFiles%\Git\bin\bash.exe"
  "%ProgramFiles%\Git\usr\bin\bash.exe"
  "%ProgramFiles%\Git\usr\bin\sh.exe"
  "%ProgramFiles(x86)%\Git\bin\bash.exe"
  "%ProgramFiles(x86)%\Git\usr\bin\bash.exe"
  "%ProgramFiles(x86)%\Git\usr\bin\sh.exe"
) do (
  if not defined posix_shell if exist %%~P (
    "%%~fP" -lc "command -v make >/dev/null 2>&1"
    if not errorlevel 1 set "posix_shell=%%~fP"
  )
)
if not defined posix_shell (
  for /f "delims=" %%I in ('where.exe sh.exe 2^>nul') do (
    if not defined posix_shell (
      "%%~fI" -lc "command -v make >/dev/null 2>&1"
      if not errorlevel 1 set "posix_shell=%%~fI"
    )
  )
)
if not defined posix_shell (
  for /f "delims=" %%I in ('where.exe bash.exe 2^>nul') do (
    if /I not "%%~fI"=="%SystemRoot%\System32\bash.exe" if not defined posix_shell (
      "%%~fI" -lc "command -v make >/dev/null 2>&1"
      if not errorlevel 1 set "posix_shell=%%~fI"
    )
  )
)
if not defined posix_shell (
  >&2 echo GNU make was not found in any detected POSIX shell environment. Install make in MSYS2 or set RECORDROUTE_POSIX_SHELL to a shell that can run make.
  exit /b 1
)
exit /b 0

:run_posix_shell
"%posix_shell%" -lc "%shell_cmd%"
goto :eof

:run_posix_shell_with_windows_path
rem MSYS and Git Bash can reset PATH on startup, so re-append the MSVC PATH explicitly.
set "shell_env_cmd=win_path=$(cygpath -up \"$RECORDROUTE_WINDOWS_PATH\") && if [ -n \"$win_path\" ]; then export PATH=\"$PATH:$win_path\"; fi"
"%posix_shell%" -lc "%shell_env_cmd% && %shell_cmd%"
goto :eof

:main
for %%I in ("%~dp0.") do set "script_dir=%%~fI"
for %%I in ("%script_dir%\..") do set "repo_root=%%~fI"
set "source_dir=%repo_root%\modules\ffmpeg"

if not exist "%source_dir%" (
  >&2 echo ffmpeg source directory not found: %source_dir%
  exit /b 1
)

call :detect_arch
if errorlevel 1 exit /b %errorlevel%

set "target=windows-%platform_arch%"
set "build_root=%repo_root%\.build\ffmpeg\%target%"
set "build_dir=%build_root%\build"
set "install_dir=%build_root%\install"
set "ffmpeg_bin=%install_dir%\bin\ffmpeg.exe"
set "ffprobe_bin=%install_dir%\bin\ffprobe.exe"

if exist "%ffmpeg_bin%" if exist "%ffprobe_bin%" (
  echo ffmpeg=%ffmpeg_bin%
  echo ffprobe=%ffprobe_bin%
  exit /b 0
)

if not exist "%build_dir%" mkdir "%build_dir%"
if not exist "%install_dir%" mkdir "%install_dir%"

call :ensure_msvc_env
if errorlevel 1 exit /b %errorlevel%

call :find_posix_shell
if errorlevel 1 exit /b %errorlevel%

set "RECORDROUTE_WINDOWS_PATH=!PATH!"
set "shell_cmd=command -v make >/dev/null 2>&1 && command -v cygpath >/dev/null 2>&1 && command -v cmp >/dev/null 2>&1"
call :run_posix_shell
if errorlevel 1 (
  >&2 echo GNU make, cmp, or cygpath not found in the selected POSIX shell environment. Use MSYS2 bash with diffutils installed or set RECORDROUTE_POSIX_SHELL to a shell that provides GNU make, cmp, and cygpath.
  exit /b 1
)
set "shell_cmd=command -v cl.exe >/dev/null 2>&1"
call :run_posix_shell_with_windows_path
if errorlevel 1 (
  >&2 echo The selected POSIX shell could not resolve cl.exe after importing the MSVC PATH. Use MSYS2 or Git Bash and make sure Visual Studio C++ tools are installed.
  exit /b 1
)

set "jobs=%NUMBER_OF_PROCESSORS%"
if not defined jobs set "jobs=4"
set "shell_cmd=source_dir=$(cygpath -u '%source_dir%') && build_dir=$(cygpath -u '%build_dir%') && install_dir=$(cygpath -u '%install_dir%') && cd \"$build_dir\" && export MSYS2_ARG_CONV_EXCL='*' && \"$source_dir/configure\" --toolchain=msvc --prefix=\"$install_dir\" --disable-ffplay --disable-doc --disable-network --disable-autodetect --disable-debug && unset MSYS2_ARG_CONV_EXCL && make -j%jobs% all && make install"
call :run_posix_shell_with_windows_path
if errorlevel 1 exit /b %errorlevel%

if not exist "%ffmpeg_bin%" (
  >&2 echo ffmpeg binary not found after build: %ffmpeg_bin%
  exit /b 1
)

if not exist "%ffprobe_bin%" (
  >&2 echo ffmpeg binaries not found after build: %install_dir%\bin
  exit /b 1
)

echo ffmpeg=%ffmpeg_bin%
echo ffprobe=%ffprobe_bin%
exit /b 0
