-- AIRouter Schema — Migration 002: OAuth provider connections

CREATE TABLE IF NOT EXISTS provider_connections (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    provider_name VARCHAR(100) NOT NULL,
    auth_type VARCHAR(20) NOT NULL DEFAULT 'oauth',  -- 'oauth', 'apikey', 'cookie'
    display_name VARCHAR(255) NOT NULL DEFAULT '',
    email VARCHAR(255) DEFAULT '',
    priority INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT true,
    data JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_provider_connections_provider ON provider_connections(provider_name);
CREATE INDEX IF NOT EXISTS idx_provider_connections_active ON provider_connections(is_active);
