CREATE TABLE IF NOT EXISTS reminders (
    id SERIAL PRIMARY KEY,
    user_id TEXT NOT NULL,
    channel_id TEXT NOT NULL,
    reminder TEXT,
    remind_at BIGINT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX IF NOT EXISTS idx_reminders_remind_at ON reminders (remind_at);
CREATE INDEX IF NOT EXISTS idx_reminders_user_id ON reminders (user_id);
