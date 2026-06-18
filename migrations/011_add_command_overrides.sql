CREATE TABLE IF NOT EXISTS command_overrides (
    scope TEXT NOT NULL,
    command TEXT NOT NULL,
    enabled BOOLEAN NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (scope, command)
);

CREATE INDEX IF NOT EXISTS idx_command_overrides_command ON command_overrides(command);
