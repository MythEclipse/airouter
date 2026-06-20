-- AIRouter Schema — Migration 001

CREATE EXTENSION IF NOT EXISTS pgcrypto;

CREATE TABLE IF NOT EXISTS providers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(255) NOT NULL UNIQUE,
    provider_type VARCHAR(100) NOT NULL,
    api_key TEXT NOT NULL DEFAULT '',
    base_url TEXT NOT NULL DEFAULT '',
    models TEXT[] NOT NULL DEFAULT '{}',
    extra_headers JSONB NOT NULL DEFAULT '{}',
    capabilities TEXT[] NOT NULL DEFAULT '{}',
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
-- Ensure unique constraints exist even if table already existed
DROP INDEX IF EXISTS idx_providers_name;
CREATE UNIQUE INDEX IF NOT EXISTS idx_providers_name ON providers(name);

CREATE TABLE IF NOT EXISTS routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    model VARCHAR(255) NOT NULL,
    strategy VARCHAR(50) NOT NULL DEFAULT 'fallback',
    provider VARCHAR(255),
    providers TEXT[],
    combo JSONB NOT NULL DEFAULT '{}',
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(model)
);
-- Ensure unique constraint exists even if table already existed
CREATE UNIQUE INDEX IF NOT EXISTS idx_routes_model ON routes(model);

CREATE TABLE IF NOT EXISTS api_keys (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    key_name VARCHAR(255) NOT NULL,
    key_hash VARCHAR(64) NOT NULL UNIQUE,
    key_prefix VARCHAR(20) NOT NULL,
    enabled BOOLEAN NOT NULL DEFAULT true,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_api_keys_key_hash ON api_keys(key_hash);

CREATE TABLE IF NOT EXISTS server_config (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    host VARCHAR(255) NOT NULL DEFAULT '0.0.0.0',
    port INTEGER NOT NULL DEFAULT 3000,
    default_max_tokens INTEGER,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
ALTER TABLE server_config ADD COLUMN IF NOT EXISTS default_max_tokens INTEGER;

CREATE TABLE IF NOT EXISTS rate_limit_config (
    id INTEGER PRIMARY KEY DEFAULT 1 CHECK (id = 1),
    enabled BOOLEAN NOT NULL DEFAULT true,
    requests_per_minute BIGINT NOT NULL DEFAULT 60,
    burst_size INTEGER NOT NULL DEFAULT 20,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
