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
exit /b 0

:read_build_stamp
set "cached_backend="
if not exist "%build_stamp%" exit /b 0
for /f "usebackq tokens=1,* delims==" %%A in ("%build_stamp%") do (
  if /I "%%~A"=="GGML_BACKEND" set "cached_backend=%%~B"
)
exit /b 0

:write_build_stamp
> "%build_stamp%" (
  echo SCHEMA=2
  echo GGML_BACKEND=%~1
  echo LLAMA_TLS_PROVIDER=boringssl
)
exit /b 0

:restore_cached_binary
set "backend_name=%~1"
set "backend_build_dir=%build_root%\build\%backend_name%"
set "built_llama_bin=%backend_build_dir%\bin\llama-cli.exe"
set "built_llama_release_bin=%backend_build_dir%\bin\Release\llama-cli.exe"
set "built_llama_embedding_bin=%backend_build_dir%\bin\llama-embedding.exe"
set "built_llama_embedding_release_bin=%backend_build_dir%\bin\Release\llama-embedding.exe"
if not exist "%runtime_bin%" mkdir "%runtime_bin%"
if exist "%built_llama_bin%" (
  copy /y "%built_llama_bin%" "%llama_bin%" >nul
)
if exist "%built_llama_embedding_bin%" (
  copy /y "%built_llama_embedding_bin%" "%llama_embedding_bin%" >nul
)
if exist "%llama_bin%" if exist "%llama_embedding_bin%" (
  echo llama_cli=%llama_bin%
  echo llama_embedding=%llama_embedding_bin%
  exit /b 0
)
if exist "%built_llama_release_bin%" (
  copy /y "%built_llama_release_bin%" "%llama_bin%" >nul
)
if exist "%built_llama_embedding_release_bin%" (
  copy /y "%built_llama_embedding_release_bin%" "%llama_embedding_bin%" >nul
)
if exist "%llama_bin%" if exist "%llama_embedding_bin%" (
  echo llama_cli=%llama_bin%
  echo llama_embedding=%llama_embedding_bin%
  exit /b 0
)
exit /b 1

:apply_backend_flags
set "backend_name=%~1"
set "backend_cmake_flags="
if /I "%backend_name%"=="cuda" set "backend_cmake_flags=-DGGML_CUDA=ON -DGGML_METAL=OFF"
if /I "%backend_name%"=="cpu" set "backend_cmake_flags=-DGGML_CUDA=OFF -DGGML_METAL=OFF"
if not defined backend_cmake_flags (
  >&2 echo Unsupported llama backend: %backend_name%
  exit /b 1
)
exit /b 0

:maybe_reset_stale_backend_build_dir
set "cache_path=%backend_build_dir%\CMakeCache.txt"
if not exist "%cache_path%" exit /b 0
set "configured_source="
for /f "usebackq tokens=2 delims==" %%I in (`findstr /b /c:"CMAKE_HOME_DIRECTORY:INTERNAL=" "%cache_path%"`) do (
  if not defined configured_source set "configured_source=%%~I"
)
if not defined configured_source exit /b 0
if /I "%configured_source%"=="%source_dir%" exit /b 0
echo Resetting stale llama build cache for %backend_name%...
set "preserve_deps=%build_root%\_deps-preserve-%backend_name%"
if exist "%preserve_deps%" rmdir /s /q "%preserve_deps%"
if exist "%backend_build_dir%\_deps" move "%backend_build_dir%\_deps" "%preserve_deps%" >nul
if exist "%backend_build_dir%" rmdir /s /q "%backend_build_dir%"
mkdir "%backend_build_dir%"
if exist "%preserve_deps%" move "%preserve_deps%" "%backend_build_dir%\_deps" >nul
exit /b 0

:build_backend
set "backend_name=%~1"
call :apply_backend_flags "%backend_name%"
if errorlevel 1 exit /b %errorlevel%

set "backend_build_dir=%build_root%\build\%backend_name%"
set "backend_runtime_bin=%backend_build_dir%\bin"
set "built_llama_bin=%backend_runtime_bin%\llama-cli.exe"
set "built_llama_release_bin=%backend_runtime_bin%\Release\llama-cli.exe"
set "built_llama_embedding_bin=%backend_runtime_bin%\llama-embedding.exe"
set "built_llama_embedding_release_bin=%backend_runtime_bin%\Release\llama-embedding.exe"

call :maybe_reset_stale_backend_build_dir
if errorlevel 1 exit /b %errorlevel%

if not exist "%backend_build_dir%" mkdir "%backend_build_dir%"
if not exist "%backend_runtime_bin%" mkdir "%backend_runtime_bin%"
if not exist "%runtime_bin%" mkdir "%runtime_bin%"

"%cmake_exe%" -S "%source_dir%" -B "%backend_build_dir%" -G "NMake Makefiles" ^
  "-DCMAKE_BUILD_TYPE=Release" ^
  "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY=%backend_runtime_bin%" ^
  -DBUILD_SHARED_LIBS=OFF ^
  -DLLAMA_BUILD_COMMON=ON ^
  -DLLAMA_BUILD_TOOLS=ON ^
  -DLLAMA_BUILD_TESTS=OFF ^
  -DLLAMA_BUILD_SERVER=ON ^
  -DLLAMA_BUILD_EXAMPLES=ON ^
  -DLLAMA_OPENSSL=OFF ^
  -DLLAMA_BUILD_BORINGSSL=ON ^
  %backend_cmake_flags%
if errorlevel 1 exit /b %errorlevel%

"%cmake_exe%" --build "%backend_build_dir%" --target llama-cli llama-embedding --parallel %jobs%
if errorlevel 1 exit /b %errorlevel%

if exist "%built_llama_bin%" copy /y "%built_llama_bin%" "%llama_bin%" >nul
if exist "%built_llama_release_bin%" copy /y "%built_llama_release_bin%" "%llama_bin%" >nul
if exist "%built_llama_embedding_bin%" copy /y "%built_llama_embedding_bin%" "%llama_embedding_bin%" >nul
if exist "%built_llama_embedding_release_bin%" copy /y "%built_llama_embedding_release_bin%" "%llama_embedding_bin%" >nul

if not exist "%llama_bin%" (
  >&2 echo llama-cli binary not found after build: %built_llama_bin%
  exit /b 1
)
if not exist "%llama_embedding_bin%" (
  >&2 echo llama-embedding binary not found after build: %built_llama_embedding_bin%
  exit /b 1
)

call :write_build_stamp "%backend_name%"
echo llama_cli=%llama_bin%
echo llama_embedding=%llama_embedding_bin%
exit /b 0

:main
for %%I in ("%~dp0.") do set "script_dir=%%~fI"
for %%I in ("%script_dir%\..") do set "repo_root=%%~fI"
set "source_dir=%repo_root%\modules\llama.cpp"

if not exist "%source_dir%" (
  >&2 echo llama.cpp source directory not found: %source_dir%
  exit /b 1
)

call :detect_arch
if errorlevel 1 exit /b %errorlevel%

set "target=windows-%platform_arch%"
set "build_root=%repo_root%\.build\llama\%target%"
set "runtime_bin=%build_root%\bin"
set "llama_bin=%runtime_bin%\llama-cli.exe"
set "llama_embedding_bin=%runtime_bin%\llama-embedding.exe"
set "build_stamp=%build_root%\build-flags.txt"

call :read_build_stamp
if /I not "%cached_backend%"=="cuda" if /I not "%cached_backend%"=="cpu" set "cached_backend="

if defined cached_backend if exist "%llama_bin%" if exist "%llama_embedding_bin%" (
  echo llama_cli=%llama_bin%
  echo llama_embedding=%llama_embedding_bin%
  exit /b 0
)

if defined cached_backend (
  call :restore_cached_binary "%cached_backend%"
  if not errorlevel 1 exit /b 0
)

call :ensure_msvc_env
if errorlevel 1 exit /b %errorlevel%

call :resolve_cmake
if errorlevel 1 exit /b %errorlevel%

set "jobs=%NUMBER_OF_PROCESSORS%"
if not defined jobs set "jobs=4"

call :build_backend cuda
if not errorlevel 1 exit /b 0

call :build_backend cpu
if not errorlevel 1 exit /b 0

>&2 echo failed to build llama-cli with supported backends: cuda cpu
exit /b 1
