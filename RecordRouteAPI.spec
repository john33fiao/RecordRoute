# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec file for RecordRoute API Server
# Phase 3: Bundle Python backend into standalone executable

import sys
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

block_cipher = None

# Collect all required data files and hidden imports
datas = []
hiddenimports = []

# Collect whisper model files and data
datas += collect_data_files('whisper')
hiddenimports += collect_submodules('whisper')

# Collect sentence-transformers data
datas += collect_data_files('sentence_transformers')
hiddenimports += collect_submodules('sentence_transformers')

# Collect torch and related libraries
hiddenimports += collect_submodules('torch')
hiddenimports += collect_submodules('torchaudio')
hiddenimports += collect_submodules('torchvision')

# Collect other ML/AI libraries
hiddenimports += collect_submodules('numpy')
hiddenimports += collect_submodules('transformers')
hiddenimports += collect_submodules('tokenizers')

# Collect ollama
hiddenimports += collect_submodules('ollama')

# Collect websockets
hiddenimports += collect_submodules('websockets')

# Collect pypdf
hiddenimports += collect_submodules('pypdf')

# Additional hidden imports
hiddenimports += [
    'multipart',
    'tiktoken_ext.openai_public',
    'tiktoken_ext',
]

a = Analysis(
    ['sttEngine/server.py'],
    pathex=[],
    binaries=[],
    datas=datas,
    hiddenimports=hiddenimports,
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[
        'matplotlib',
        'scipy',
        'pandas',
        'jupyter',
        'notebook',
        'IPython',
    ],
    win_no_prefer_redirects=False,
    win_private_assemblies=False,
    cipher=block_cipher,
    noarchive=False,
)

pyz = PYZ(a.pure, a.zipped_data, cipher=block_cipher)

exe = EXE(
    pyz,
    a.scripts,
    [],
    exclude_binaries=True,
    name='RecordRouteAPI',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=True,
    console=True,  # Keep console for debugging; set to False for production
    disable_windowed_traceback=False,
    argv_emulation=False,
    target_arch=None,
    codesign_identity=None,
    entitlements_file=None,
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,
    upx_exclude=[],
    name='RecordRouteAPI',
)
