CREATE EXTENSION IF NOT EXISTS vector;

CREATE TABLE IF NOT EXISTS recordings (
    id UUID PRIMARY KEY,
    original_filename TEXT NOT NULL,
    original_content_type TEXT,
    file_size_bytes BIGINT NOT NULL,
    original_rel_path TEXT NOT NULL,
    wav_rel_path TEXT,
    language TEXT,
    transcript TEXT,
    summary JSONB,
    summary_canonical_text TEXT,
    status TEXT NOT NULL,
    current_step TEXT NOT NULL,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    search_document TSVECTOR GENERATED ALWAYS AS (
        to_tsvector(
            'simple',
            coalesce(original_filename, '') || ' ' ||
            coalesce(summary_canonical_text, '') || ' ' ||
            coalesce(transcript, '')
        )
    ) STORED
);

CREATE INDEX IF NOT EXISTS idx_recordings_status_created_at
    ON recordings (status, created_at DESC);

CREATE INDEX IF NOT EXISTS idx_recordings_search_document
    ON recordings USING GIN (search_document);

CREATE TABLE IF NOT EXISTS jobs (
    id UUID PRIMARY KEY,
    recording_id UUID NOT NULL UNIQUE REFERENCES recordings(id) ON DELETE CASCADE,
    status TEXT NOT NULL,
    step TEXT NOT NULL,
    attempt_count INTEGER NOT NULL DEFAULT 0,
    last_error TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    started_at TIMESTAMPTZ,
    completed_at TIMESTAMPTZ,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_jobs_queue
    ON jobs (status, next_attempt_at, created_at);
