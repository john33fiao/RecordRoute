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
goto :eof

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
goto :eof

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
goto :eof

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
goto :eof

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
goto :eof

:resolve_cmake
set "cmake_exe="
for /f "delims=" %%I in ('where.exe cmake.exe 2^>nul') do (
  if not defined cmake_exe set "cmake_exe=%%~fI"
)
for %%R in ("%ProgramFiles%" "%ProgramFiles(x86)%") do (
  if not defined cmake_exe if not "%%~R"=="" if exist "%%~R\CMake\bin\cmake.exe" (
    set "cmake_exe=%%~R\CMake\bin\cmake.exe"
  )
)
for %%R in ("%ProgramFiles%" "%ProgramFiles(x86)%") do (
  if not defined cmake_exe if not "%%~R"=="" (
    for %%Y in (2022 2019) do (
      for %%E in (Community Professional Enterprise BuildTools) do (
        if exist "%%~R\Microsoft Visual Studio\%%Y\%%E\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe" (
          set "cmake_exe=%%~R\Microsoft Visual Studio\%%Y\%%E\Common7\IDE\CommonExtensions\Microsoft\CMake\CMake\bin\cmake.exe"
        )
      )
    )
  )
)
if not defined cmake_exe (
  >&2 echo cmake.exe not found. Install CMake or Visual Studio CMake components.
  exit /b 1
)
goto :eof

:main
for %%I in ("%~dp0.") do set "script_dir=%%~fI"
for %%I in ("%script_dir%\..") do set "repo_root=%%~fI"
set "source_dir=%repo_root%\llama.cpp"

if not exist "%source_dir%" (
  >&2 echo llama.cpp source directory not found: %source_dir%
  exit /b 1
)

call :detect_arch
if errorlevel 1 exit /b %errorlevel%

set "target=windows-%platform_arch%"
set "build_root=%repo_root%\.build\llama\%target%"
set "build_dir=%build_root%\build"
set "runtime_bin=%build_root%\bin"
set "llama_bin=%runtime_bin%\llama-cli.exe"
set "built_llama_bin=%build_dir%\bin\llama-cli.exe"
set "built_llama_release_bin=%build_dir%\bin\Release\llama-cli.exe"
set "build_stamp=%build_root%\build-flags.txt"
set "expected_build_stamp=LLAMA_TLS_PROVIDER=boringssl"
set "cache_ready="

if exist "%build_stamp%" (
  set /p "cached_build_stamp="<"%build_stamp%"
  if /I "!cached_build_stamp!"=="!expected_build_stamp!" set "cache_ready=1"
)

if exist "%llama_bin%" if defined cache_ready (
  echo llama_cli=%llama_bin%
  exit /b 0
)

if exist "%built_llama_bin%" (
  if not exist "%runtime_bin%" mkdir "%runtime_bin%"
  copy /y "%built_llama_bin%" "%llama_bin%" >nul
)
if exist "%llama_bin%" if defined cache_ready (
  echo llama_cli=%llama_bin%
  exit /b 0
)

if exist "%built_llama_release_bin%" (
  if not exist "%runtime_bin%" mkdir "%runtime_bin%"
  copy /y "%built_llama_release_bin%" "%llama_bin%" >nul
)
if exist "%llama_bin%" if defined cache_ready (
  echo llama_cli=%llama_bin%
  exit /b 0
)

if not exist "%build_dir%" mkdir "%build_dir%"
if not exist "%runtime_bin%" mkdir "%runtime_bin%"

call :ensure_msvc_env
if errorlevel 1 exit /b %errorlevel%

call :resolve_cmake
if errorlevel 1 exit /b %errorlevel%

set "jobs=%NUMBER_OF_PROCESSORS%"
if not defined jobs set "jobs=4"

"%cmake_exe%" -S "%source_dir%" -B "%build_dir%" -G "NMake Makefiles" ^
  "-DCMAKE_BUILD_TYPE=Release" ^
  "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY=%runtime_bin%" ^
  -DBUILD_SHARED_LIBS=OFF ^
  -DLLAMA_BUILD_COMMON=ON ^
  -DLLAMA_BUILD_TOOLS=ON ^
  -DLLAMA_BUILD_TESTS=OFF ^
  -DLLAMA_BUILD_SERVER=ON ^
  -DLLAMA_BUILD_EXAMPLES=OFF ^
  -DLLAMA_OPENSSL=OFF ^
  -DLLAMA_BUILD_BORINGSSL=ON
if errorlevel 1 exit /b %errorlevel%

"%cmake_exe%" --build "%build_dir%" --target llama-cli --parallel %jobs%
if errorlevel 1 exit /b %errorlevel%

if exist "%built_llama_bin%" copy /y "%built_llama_bin%" "%llama_bin%" >nul
if exist "%built_llama_release_bin%" copy /y "%built_llama_release_bin%" "%llama_bin%" >nul

if not exist "%llama_bin%" (
  >&2 echo llama-cli binary not found after build: %built_llama_bin%
  exit /b 1
)

> "%build_stamp%" echo %expected_build_stamp%

echo llama_cli=%llama_bin%
exit /b 0
