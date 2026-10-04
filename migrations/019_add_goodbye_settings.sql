ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS goodbye_enabled BOOLEAN DEFAULT FALSE;
ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS goodbye_channel_id TEXT;
ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS goodbye_message TEXT DEFAULT '{username} has left {server_name}.';
