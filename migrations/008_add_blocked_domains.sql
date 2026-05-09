CREATE TABLE IF NOT EXISTS global_blocked_domains (
    domain TEXT PRIMARY KEY,
    blocked_by TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE IF NOT EXISTS guild_blocked_domains (
    guild_id TEXT NOT NULL,
    domain TEXT NOT NULL,
    blocked_by TEXT NOT NULL,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    PRIMARY KEY (guild_id, domain)
);

CREATE INDEX IF NOT EXISTS idx_guild_blocked_domains_guild ON guild_blocked_domains(guild_id);
