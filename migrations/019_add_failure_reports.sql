CREATE TABLE IF NOT EXISTS embed_failures (
    id SERIAL PRIMARY KEY,
    service TEXT NOT NULL,
    code TEXT NOT NULL,
    url TEXT,
    guild_id TEXT,
    detail TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_embed_failures_guild ON embed_failures (guild_id, created_at);
CREATE INDEX IF NOT EXISTS idx_embed_failures_created ON embed_failures (created_at);

ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS report_channel_id TEXT;
