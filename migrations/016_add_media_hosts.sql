CREATE TABLE IF NOT EXISTS media_hosts (
    service TEXT NOT NULL,
    domain TEXT NOT NULL,
    added_by TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (service, domain)
);
