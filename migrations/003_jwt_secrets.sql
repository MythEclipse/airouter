-- AIRouter Schema — Migration 003: JWT signing secrets
--
-- Stores JWT signing secrets with rotation support.
-- Singleton row: id always = 1.

CREATE TABLE IF NOT EXISTS jwt_secrets (
    id INTEGER PRIMARY KEY CHECK (id = 1),
    current_secret TEXT NOT NULL,
    previous_secret TEXT,
    previous_expires_at TIMESTAMPTZ,
    rotated_at TIMESTAMPTZ,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
