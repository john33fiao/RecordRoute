#!/usr/bin/env python3
# -*- coding: utf-8 -*-
"""
RecordRoute Model Bootstrap Script
Phase 3: Automatic model download and initialization

This script ensures all required models are available before the application starts.
It downloads Whisper and embedding models on first run or when models are missing.
"""

import os
import sys
import subprocess
from pathlib import Path


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


def check_ollama_running():
    """Check if Ollama service is running"""
    try:
        import ollama
        # Try to list models to check if Ollama is accessible
        ollama.list()
        print("✓ Ollama service is running")
        return True
    except Exception as e:
        print(f"✗ Ollama service not accessible: {e}")
        return False


def check_ollama_model(model_name):
    """Check if specific Ollama model is available"""
    try:
        import ollama
        models = ollama.list()
        model_names = [m['name'] for m in models.get('models', [])]

        # Check if model exists (with or without tag)
        base_name = model_name.split(':')[0]
        found = any(base_name in name for name in model_names)

        if found:
            print(f"✓ Ollama model '{model_name}' is available")
            return True
        else:
            print(f"✗ Ollama model '{model_name}' not found")
            return False
    except Exception as e:
        print(f"✗ Failed to check Ollama model: {e}")
        return False


def download_ollama_model(model_name):
    """Download Ollama model"""
    print(f"\n📥 Downloading Ollama model '{model_name}'...")
    print("This may take several minutes to hours depending on model size and connection.")

    try:
        import ollama
        # Pull the model
        print(f"Pulling model '{model_name}'...")
        ollama.pull(model_name)
        print(f"✓ Ollama model '{model_name}' downloaded successfully")
        return True
    except Exception as e:
        print(f"✗ Failed to download Ollama model: {e}")
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
    print("RecordRoute Model Bootstrap")
    print("=" * 60)

    # Check and download Whisper model
    print("\n[1/3] Checking Whisper STT model...")
    if not check_whisper_model('base'):
        if not download_whisper_model('base'):
            print("⚠ Warning: Whisper model download failed. STT may not work.")

    # Check Ollama
    print("\n[2/3] Checking Ollama LLM service...")
    if check_ollama_running():
        # Check for default models
        required_models = ['gemma2:2b', 'bge-m3:latest']
        for model in required_models:
            if not check_ollama_model(model):
                print(f"⚠ Recommended model '{model}' not found.")
                print(f"  You can download it by running: ollama pull {model}")
    else:
        print("⚠ Warning: Ollama service not running. Summarization will not work.")
        print("  Please install and start Ollama from https://ollama.ai")

    # Check embedding model
    print("\n[3/3] Checking embedding model...")
    if not check_embedding_model():
        print("⚠ Warning: Embedding model check failed. Vector search may not work.")

    print("\n" + "=" * 60)
    print("Bootstrap complete!")
    print("=" * 60)
    print("\nNote: If any models are missing, they will be downloaded on first use.")
    print("You can manually download Ollama models using: ollama pull <model-name>")
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
