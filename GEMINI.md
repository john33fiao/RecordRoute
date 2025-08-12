# GEMINI.md – RecordRoute Guide for Gemini

## Overview
RecordRoute is a pipeline that converts audio or video recordings into cleaned and summarized meeting notes.  The workflow consists of three stages:

1. **Speech‑to‑Text** – `sttEngine/workflow/transcribe.py` runs OpenAI Whisper and produces Markdown transcripts.
2. **Text Correction** – `sttEngine/workflow/correct.py` refines the transcript using an Ollama model.
3. **Summarization** – `sttEngine/workflow/summarize.py` generates a structured summary.

Intermediate and final files are stored in `whisper_output/`.

## Repository Layout
```
RecordRoute/
├── run.sh                # Unix/macOS/Linux entry point
├── run.bat               # Windows entry point
├── sttEngine/
│   ├── run_workflow.py   # Interactive orchestrator
│   └── workflow/
│       ├── transcribe.py # Stage 1: speech recognition
│       ├── correct.py    # Stage 2: text correction
│       └── summarize.py  # Stage 3: summarization
└── whisper_output/       # Generated Markdown and summaries
```

## Core Modules

### transcribe.py
* Handles `.flac`, `.m4a`, `.mp3`, `.mp4`, `.mpeg`, `.mpga`, `.oga`, `.ogg`, `.wav`, and `.webm` files.
* Prefers the `large-v3-turbo` Whisper model and falls back to `large` if the turbo weights are not found in the platform cache.
* Supports speaker diarization via `pyannote.audio` with a `PYANNOTE_TOKEN` environment variable.
* Converts M4A sources to 16‑kHz mono WAV, merges adjacent identical segments and optionally filters filler words.
* Important CLI flags: `--language`, `--initial_prompt`, `--workers`, `--recursive`, `--filter_fillers`, `--min_seg_length`, `--normalize_punct`, `--diarize`.

### correct.py
* Uses Ollama to edit transcripts while preserving meaning and Markdown structure.
* Default models – Windows: `gemma3:4b`; macOS/Linux: `gemma3:12b-it-qat`.
* Automatically falls back between encodings when reading files and chunks long documents with overlap.
* Saves results as `<name>.corrected.md`.

### summarize.py
* Produces meeting summaries using a Map‑Reduce approach for large inputs.
* Default models – Windows: `gemma3:4b`; macOS/Linux: `gpt-oss:20b`.
* Outputs six fixed sections (Major Topics, Key Points, Decisions, Action Items, Risks/Issues, Next Steps) and supports a `--json` flag.
* Writes `<name>.summary.md` or `.summary.json`.

### run_workflow.py
* Command‑line driver for running stage 1‑3 sequentially or individually.
* Uses `sys.executable` so the same script works on Windows and Unix platforms.
* Streams subprocess output and manages intermediate filenames in `whisper_output/`.

## Running the Pipeline

### Windows
```bash
cd sttEngine
setup.bat     # create virtualenv and install dependencies
run.bat       # launch the workflow
```

### macOS/Linux
```bash
./run.sh      # executes sttEngine/run_workflow.py inside the virtualenv
```

## Key Dependencies
- `openai-whisper` for speech recognition
- `ollama` for local LLM inference
- `pyannote.audio` for optional speaker diarization
- `ffmpeg` for audio format conversion

## Notes
- Set `PYANNOTE_TOKEN` to enable diarization.
- Use `--workers 1` unless multiple GPUs are available.

