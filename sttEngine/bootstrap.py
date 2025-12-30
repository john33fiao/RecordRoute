#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
RecordRoute Model Bootstrap Script
Phase 5: llama.cpp GGUF model initialization

This script ensures all required models are available before the application starts.
It checks for Whisper, sentence-transformers, and GGUF models.
"""

import os
import sys
import subprocess
from pathlib import Path

# Import project modules
sys.path.append(str(Path(__file__).parent.parent))
from sttEngine.llamacpp_utils import MODELS_DIR as GGUF_MODELS_DIR


def get_models_path():
    """Get the models directory path based on environment"""
    if getattr(sys, 'frozen', False):
        # Running as PyInstaller bundle
        from electron import app
        return os.path.join(app.getPath('userData'), 'models')
    else:
        # Running in development
        return os.path.join(os.path.dirname(os.path.dirname(__file__)), 'models')


def ensure_directory(path):
    """Ensure directory exists"""
    os.makedirs(path, exist_ok=True)
    return path


def check_whisper_model(model_name='base'):
    """Check if Whisper model is available"""
    try:
        import whisper
        models_dir = os.path.join(os.path.expanduser('~'), '.cache', 'whisper')
        model_file = f"{model_name}.pt"
        model_path = os.path.join(models_dir, model_file)

        if os.path.exists(model_path):
            print(f"✓ Whisper model '{model_name}' found at {model_path}")
            return True
        else:
            print(f"✗ Whisper model '{model_name}' not found")
            return False
    except ImportError:
        print("✗ Whisper library not installed")
        return False


def download_whisper_model(model_name='base'):
    """Download Whisper model"""
    print(f"\n📥 Downloading Whisper model '{model_name}'...")
    print("This may take a few minutes depending on your internet connection.")

    try:
        import whisper
        print("Loading model (this will trigger download if needed)...")
        model = whisper.load_model(model_name)
        print(f"✓ Whisper model '{model_name}' downloaded successfully")
        return True
    except Exception as e:
        print(f"✗ Failed to download Whisper model: {e}")
        return False


def check_gguf_models_dir():
    """Check if GGUF models directory exists and create if needed"""
    try:
        GGUF_MODELS_DIR.mkdir(parents=True, exist_ok=True)
        print(f"✓ GGUF models directory ready at {GGUF_MODELS_DIR}")
        return True
    except Exception as e:
        print(f"✗ Failed to create GGUF models directory: {e}")
        return False


def list_gguf_models():
    """List available GGUF models"""
    try:
        if not GGUF_MODELS_DIR.exists():
            return []

        gguf_files = list(GGUF_MODELS_DIR.glob("*.gguf"))
        return [f.name for f in gguf_files]
    except Exception as e:
        print(f"✗ Failed to list GGUF models: {e}")
        return []


def check_gguf_model(model_filename):
    """Check if specific GGUF model file exists"""
    model_path = GGUF_MODELS_DIR / model_filename
    if model_path.exists():
        size_mb = model_path.stat().st_size / (1024 * 1024)
        print(f"✓ GGUF model '{model_filename}' found ({size_mb:.1f}MB)")
        return True
    else:
        print(f"✗ GGUF model '{model_filename}' not found")
        return False


def check_embedding_model():
    """Check if embedding model is available"""
    try:
        from sentence_transformers import SentenceTransformer

        # Try to load the model (will download if not present)
        model_name = "BAAI/bge-m3"
        print(f"Checking embedding model '{model_name}'...")

        models_cache = os.path.join(os.path.expanduser('~'), '.cache', 'huggingface')
        if os.path.exists(models_cache):
            print(f"✓ Hugging Face cache exists at {models_cache}")

        # This will download if needed
        model = SentenceTransformer(model_name)
        print(f"✓ Embedding model '{model_name}' is ready")
        return True
    except Exception as e:
        print(f"✗ Failed to load embedding model: {e}")
        return False


def bootstrap_models():
    """Main bootstrap function"""
    print("=" * 60)
    print("RecordRoute Model Bootstrap (Phase 5: llama.cpp)")
    print("=" * 60)

    # Check and download Whisper model
    print("\n[1/3] Checking Whisper STT model...")
    if not check_whisper_model('base'):
        if not download_whisper_model('base'):
            print("⚠ Warning: Whisper model download failed. STT may not work.")

    # Check GGUF models
    print("\n[2/3] Checking GGUF models for llama.cpp...")
    if check_gguf_models_dir():
        available_models = list_gguf_models()
        if available_models:
            print(f"Found {len(available_models)} GGUF model(s):")
            for model in available_models:
                check_gguf_model(model)
        else:
            print("⚠ No GGUF models found. Please download GGUF models manually.")
            print(f"  Model directory: {GGUF_MODELS_DIR}")
            print("\n  Recommended models:")
            print("  - Windows: gemma-2-2b-it-Q4_K_M.gguf")
            print("  - Unix/macOS: qwen2.5-14b-instruct-q4_k_m.gguf")
            print("\n  Download from Hugging Face:")
            print("  - https://huggingface.co/models?search=gguf")
            print(f"  - Save files to: {GGUF_MODELS_DIR}")

    # Check embedding model
    print("\n[3/3] Checking embedding model...")
    if not check_embedding_model():
        print("⚠ Warning: Embedding model check failed. Vector search may not work.")

    print("\n" + "=" * 60)
    print("Bootstrap complete!")
    print("=" * 60)
    print("\nNote: Missing models will be downloaded automatically on first use.")
    print("For GGUF models, please download manually from Hugging Face.")
    print()


if __name__ == '__main__':
    try:
        bootstrap_models()
    except KeyboardInterrupt:
        print("\n\nBootstrap interrupted by user.")
        sys.exit(1)
    except Exception as e:
        print(f"\n\nBootstrap failed with error: {e}")
        import traceback
        traceback.print_exc()
        sys.exit(1)
