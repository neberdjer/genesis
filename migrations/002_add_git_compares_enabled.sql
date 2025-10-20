ALTER TABLE server_settings ADD COLUMN IF NOT EXISTS git_compares_enabled BOOLEAN DEFAULT TRUE;
