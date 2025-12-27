ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS welcome_enabled BOOLEAN DEFAULT FALSE;
ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS welcome_channel_id TEXT;
ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS welcome_message TEXT DEFAULT 'Welcome to {server_name}, {user}';
ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS welcome_role_id TEXT;
