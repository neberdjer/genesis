CREATE TABLE IF NOT EXISTS server_settings (
    guild_id TEXT PRIMARY KEY,
    git_diffs_enabled BOOLEAN DEFAULT TRUE,
    git_links_enabled BOOLEAN DEFAULT TRUE,
    twitter_enabled BOOLEAN DEFAULT TRUE,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_server_settings_guild_id ON server_settings(guild_id);
