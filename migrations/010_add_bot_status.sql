CREATE TABLE IF NOT EXISTS bot_status (
    id BOOLEAN PRIMARY KEY DEFAULT TRUE CHECK (id),
    status_type TEXT NOT NULL,
    status_text TEXT NOT NULL,
    online_status TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
