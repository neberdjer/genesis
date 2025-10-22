CREATE TABLE IF NOT EXISTS user_blacklist (
    user_id TEXT PRIMARY KEY,
    reason TEXT,
    blacklisted_by TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_user_blacklist_user_id ON user_blacklist(user_id);

CREATE TABLE IF NOT EXISTS server_blacklist (
    guild_id TEXT PRIMARY KEY,
    reason TEXT,
    blacklisted_by TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_server_blacklist_guild_id ON server_blacklist(guild_id);
