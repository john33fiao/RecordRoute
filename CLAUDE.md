# CLAUDE.md – RecordRoute Guide for Claude

## Overview
RecordRoute transforms audio or video recordings into corrected and summarized meeting notes.  It runs a three stage pipeline:

1. **Speech‑to‑Text** – `sttEngine/workflow/transcribe.py` uses OpenAI Whisper to turn audio into Markdown.
2. **Text Correction** – `sttEngine/workflow/correct.py` cleans the transcript with an Ollama model.
3. **Summarization** – `sttEngine/workflow/summarize.py` produces a structured meeting summary.

All intermediate and final files are written to the `whisper_output/` directory.

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
* Converts supported media files (`.flac`, `.m4a`, `.mp3`, `.mp4`, `.mpeg`, `.mpga`, `.oga`, `.ogg`, `.wav`, `.webm`).
* Detects platform‑specific Whisper caches and prefers the `large-v3-turbo` model, falling back to `large` when necessary.
* Optional speaker diarization via `pyannote.audio`; requires a `PYANNOTE_TOKEN` environment variable.
* M4A files are converted to mono 16‑kHz WAV before processing.
* Merges identical adjacent segments, filters filler words and normalizes punctuation.
* CLI options include `--language`, `--initial_prompt`, `--workers`, `--recursive`, `--filter_fillers`, `--min_seg_length`, `--normalize_punct` and `--diarize`.

### correct.py
* Cleans Markdown transcripts using Ollama.
* Default models: `gemma3:4b` on Windows, `gemma3:12b-it-qat` on macOS/Linux.
* Reads files with automatic encoding fallbacks and splits large documents into overlapping chunks.
* Enforces preservation of meaning, speaker labels, timestamps and Markdown structure.
* Outputs `<name>.corrected.md`.

### summarize.py
* Summarizes corrected transcripts with a Map‑Reduce strategy for large inputs.
* Default models: `gemma3:4b` on Windows, `gpt-oss:20b` on macOS/Linux.
* Produces six fixed sections – Major Topics, Key Points, Decisions, Action Items, Risks/Issues and Next Steps.
* Supports `--json` output for machine consumption and writes `<name>.summary.md` or `.summary.json`.

### run_workflow.py
* Interactive runner that executes stage 1‑3 in sequence or individually.
* Uses the current Python interpreter (`sys.executable`) and streams subprocess output.
* Automatically stores results in `whisper_output/` and passes new files to subsequent stages.

## Running the Pipeline

### Windows
```bash
cd sttEngine
setup.bat     # create virtualenv and install dependencies
run.bat       # launch the workflow
```

### macOS/Linux
```bash
./run.sh      # runs sttEngine/run_workflow.py inside the virtualenv
```

## Key Dependencies
- `openai-whisper` – speech recognition
- `ollama` – local LLM inference
- `pyannote.audio` – speaker diarization (optional)
- `ffmpeg` – audio conversion

## Notes
- Speaker diarization requires a Hugging Face token exported as `PYANNOTE_TOKEN`.
- The workflow is optimized for single GPU systems; use `--workers 1` unless running on multiple devices.

