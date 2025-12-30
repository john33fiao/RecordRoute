# -*- mode: python ; coding: utf-8 -*-
# PyInstaller spec file for RecordRoute API Server
# Phase 3 & 4: Bundle Python backend into standalone executable

import sys
import os
from PyInstaller.utils.hooks import collect_data_files, collect_submodules

block_cipher = None

# Collect all required data files and hidden imports
datas = []
hiddenimports = []

# ============================================================================
# Core Dependencies
# ============================================================================

# Collect whisper model files and data
# Note: Large model files should be downloaded at runtime via bootstrap
datas += collect_data_files('whisper')
hiddenimports += collect_submodules('whisper')

# Collect sentence-transformers data (for embedding/RAG)
datas += collect_data_files('sentence_transformers')
hiddenimports += collect_submodules('sentence_transformers')

# ============================================================================
# Deep Learning Frameworks
# ============================================================================

# PyTorch and related libraries
hiddenimports += collect_submodules('torch')
hiddenimports += collect_submodules('torchaudio')
hiddenimports += collect_submodules('torchvision')

# Transformers ecosystem
hiddenimports += collect_submodules('transformers')
hiddenimports += collect_submodules('tokenizers')

# ============================================================================
# LLM Integration (Phase 5: llama.cpp)
# ============================================================================

# llama.cpp integration for self-contained LLM execution
hiddenimports += collect_submodules('llama_cpp')
datas += collect_data_files('llama_cpp')

# Hugging Face Hub for model downloads
hiddenimports += collect_submodules('huggingface_hub')
datas += collect_data_files('huggingface_hub')

# ============================================================================
# Utilities & Supporting Libraries
# ============================================================================

# Web server and networking
hiddenimports += collect_submodules('websockets')
hiddenimports += collect_submodules('http')
hiddenimports += collect_submodules('http.server')

# File processing
hiddenimports += collect_submodules('pypdf')
hiddenimports += collect_submodules('multipart')

# Scientific computing
hiddenimports += collect_submodules('numpy')

# Tokenization
hiddenimports += [
    'tiktoken_ext.openai_public',
    'tiktoken_ext',
]

# ============================================================================
# RecordRoute Application Modules
# ============================================================================

# Include workflow modules
datas += [
    ('sttEngine/workflow', 'workflow'),
    ('sttEngine/config.py', '.'),
    ('sttEngine/logger.py', '.'),
    ('sttEngine/search_cache.py', '.'),
    ('sttEngine/vector_search.py', '.'),
    ('sttEngine/embedding_pipeline.py', '.'),
    ('sttEngine/ollama_utils.py', '.'),
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
        # Exclude unnecessary packages to reduce bundle size
        'matplotlib',
        'scipy',
        'pandas',
        'jupyter',
        'notebook',
        'IPython',
        'sphinx',
        'pytest',
        'setuptools',
        # Qt libraries (if not needed)
        'PyQt5',
        'PyQt6',
        'PySide2',
        'PySide6',
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
    debug=False,  # Set to True for debugging PyInstaller issues
    bootloader_ignore_signals=False,
    strip=False,  # Don't strip symbols (better for debugging)
    upx=True,  # Compress with UPX (reduces size)
    console=True,  # Keep console for debugging; set to False for production
    disable_windowed_traceback=False,
    argv_emulation=False,  # macOS-specific, set to True if needed
    target_arch=None,  # Auto-detect architecture
    codesign_identity=None,  # macOS code signing (Phase 4)
    entitlements_file=None,  # macOS entitlements (Phase 4)
)

coll = COLLECT(
    exe,
    a.binaries,
    a.zipfiles,
    a.datas,
    strip=False,
    upx=True,  # Compress executables with UPX
    upx_exclude=[],  # Exclude specific files from UPX compression if needed
    name='RecordRouteAPI',
)
