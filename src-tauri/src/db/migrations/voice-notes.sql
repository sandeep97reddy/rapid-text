-- Migration 3: Create voice_notes and custom_dictionary tables for Rapid Text

CREATE TABLE IF NOT EXISTS voice_notes (
    id TEXT PRIMARY KEY,
    title TEXT NOT NULL,
    content TEXT NOT NULL,
    raw_transcription TEXT,
    created_at INTEGER NOT NULL,
    updated_at INTEGER NOT NULL,
    tags TEXT,
    is_pinned INTEGER DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_voice_notes_created_at ON voice_notes(created_at DESC);

CREATE TABLE IF NOT EXISTS custom_dictionary (
    word TEXT PRIMARY KEY,
    hit_count INTEGER DEFAULT 1,
    created_at INTEGER NOT NULL
);
