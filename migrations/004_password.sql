-- AIRouter Schema — Migration 004: Password management for server_config
--
-- Adds password management columns to existing server_config singleton.
-- All columns nullable/defaulted so this is safe to run on existing data.

ALTER TABLE server_config
    ADD COLUMN IF NOT EXISTS password_hash TEXT,
    ADD COLUMN IF NOT EXISTS password_changed_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS must_change_password BOOLEAN NOT NULL DEFAULT FALSE;
