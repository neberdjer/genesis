use sqlx::{Row, postgres::PgPoolOptions};

pub type Pool = sqlx::PgPool;

pub const SERVICES: &[(&str, &str)] = &[
    ("git_diffs", "GitHub/GitLab commit diffs"),
    ("git_compares", "GitHub/GitLab compare links"),
    ("git_links", "GitHub/GitLab file snippets"),
    ("twitter", "Twitter / X embeds"),
    ("tiktok", "TikTok embeds"),
    ("instagram", "Instagram embeds"),
    ("reddit", "Reddit embeds"),
];

pub const DEFAULT_WELCOME: &str = "Welcome to {server_name}, {user}";

#[derive(Debug, Clone)]
pub struct Settings {
    pub git_diffs: bool,
    pub git_compares: bool,
    pub git_links: bool,
    pub twitter: bool,
    pub tiktok: bool,
    pub instagram: bool,
    pub reddit: bool,
    pub welcome_enabled: bool,
    pub welcome_channel_id: Option<String>,
    pub welcome_message: String,
    pub welcome_role_id: Option<String>,
}

impl Settings {
    pub fn defaults() -> Self {
        Settings {
            git_diffs: true,
            git_compares: true,
            git_links: true,
            twitter: true,
            tiktok: true,
            instagram: true,
            reddit: true,
            welcome_enabled: false,
            welcome_channel_id: None,
            welcome_message: DEFAULT_WELCOME.to_string(),
            welcome_role_id: None,
        }
    }
}

impl Settings {
    pub fn enabled(&self, service: &str) -> bool {
        match service {
            "git_diffs" => self.git_diffs,
            "git_compares" => self.git_compares,
            "git_links" => self.git_links,
            "twitter" => self.twitter,
            "tiktok" => self.tiktok,
            "instagram" => self.instagram,
            "reddit" => self.reddit,
            _ => false,
        }
    }
}

pub async fn connect(url: &str) -> Result<Pool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(10)
        .min_connections(2)
        .acquire_timeout(std::time::Duration::from_secs(5))
        .connect(url)
        .await
}

pub async fn get_settings(pool: &Pool, guild_id: &str) -> Result<Settings, sqlx::Error> {
    let row = sqlx::query(
        r#"
        SELECT git_diffs_enabled, git_compares_enabled, git_links_enabled,
               twitter_enabled, tiktok_enabled, instagram_enabled, reddit_enabled,
               welcome_enabled, welcome_channel_id, welcome_message, welcome_role_id
        FROM server_settings
        WHERE guild_id = $1
        "#,
    )
    .bind(guild_id)
    .fetch_optional(pool)
    .await?;

    Ok(match row {
        Some(r) => Settings {
            git_diffs: r.try_get("git_diffs_enabled")?,
            git_compares: r.try_get("git_compares_enabled")?,
            git_links: r.try_get("git_links_enabled")?,
            twitter: r.try_get("twitter_enabled")?,
            tiktok: r.try_get("tiktok_enabled")?,
            instagram: r.try_get("instagram_enabled")?,
            reddit: r.try_get("reddit_enabled")?,
            welcome_enabled: r.try_get("welcome_enabled")?,
            welcome_channel_id: r.try_get("welcome_channel_id")?,
            welcome_message: r
                .try_get::<Option<String>, _>("welcome_message")?
                .unwrap_or_else(|| DEFAULT_WELCOME.to_string()),
            welcome_role_id: r.try_get("welcome_role_id")?,
        },
        None => Settings::defaults(),
    })
}

pub async fn set_welcome(
    pool: &Pool,
    guild_id: &str,
    enabled: bool,
    channel_id: Option<&str>,
    message: &str,
    role_id: Option<&str>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO server_settings
            (guild_id, welcome_enabled, welcome_channel_id, welcome_message, welcome_role_id)
        VALUES ($1, $2, $3, $4, $5)
        ON CONFLICT (guild_id) DO UPDATE SET
            welcome_enabled = $2,
            welcome_channel_id = $3,
            welcome_message = $4,
            welcome_role_id = $5,
            updated_at = NOW()
        "#,
    )
    .bind(guild_id)
    .bind(enabled)
    .bind(channel_id)
    .bind(message)
    .bind(role_id)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn set_settings(
    pool: &Pool,
    guild_id: &str,
    settings: &Settings,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO server_settings
            (guild_id, git_diffs_enabled, git_compares_enabled, git_links_enabled,
             twitter_enabled, tiktok_enabled, instagram_enabled, reddit_enabled)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8)
        ON CONFLICT (guild_id) DO UPDATE SET
            git_diffs_enabled = $2,
            git_compares_enabled = $3,
            git_links_enabled = $4,
            twitter_enabled = $5,
            tiktok_enabled = $6,
            instagram_enabled = $7,
            reddit_enabled = $8,
            updated_at = NOW()
        "#,
    )
    .bind(guild_id)
    .bind(settings.git_diffs)
    .bind(settings.git_compares)
    .bind(settings.git_links)
    .bind(settings.twitter)
    .bind(settings.tiktok)
    .bind(settings.instagram)
    .bind(settings.reddit)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn list_domains(pool: &Pool, guild_id: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT domain FROM guild_blocked_domains WHERE guild_id = $1 ORDER BY domain",
    )
    .bind(guild_id)
    .fetch_all(pool)
    .await
}

pub async fn list_git_hosts(pool: &Pool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT domain FROM git_hosts ORDER BY domain")
        .fetch_all(pool)
        .await
}

pub async fn add_domain(
    pool: &Pool,
    guild_id: &str,
    domain: &str,
    blocked_by: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        INSERT INTO guild_blocked_domains (guild_id, domain, blocked_by)
        VALUES ($1, $2, $3)
        ON CONFLICT (guild_id, domain) DO NOTHING
        "#,
    )
    .bind(guild_id)
    .bind(domain)
    .bind(blocked_by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_domain(pool: &Pool, guild_id: &str, domain: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM guild_blocked_domains WHERE guild_id = $1 AND domain = $2")
        .bind(guild_id)
        .bind(domain)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct BlacklistEntry {
    pub id: String,
    pub reason: Option<String>,
}

pub async fn list_user_blacklist(pool: &Pool) -> Result<Vec<BlacklistEntry>, sqlx::Error> {
    let rows = sqlx::query("SELECT user_id, reason FROM user_blacklist ORDER BY created_at DESC")
        .fetch_all(pool)
        .await?;
    Ok(rows
        .into_iter()
        .map(|r| BlacklistEntry {
            id: r.get("user_id"),
            reason: r.get("reason"),
        })
        .collect())
}

pub async fn add_user_blacklist(
    pool: &Pool,
    user_id: &str,
    reason: Option<&str>,
    by: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO user_blacklist (user_id, reason, blacklisted_by) VALUES ($1, $2, $3)
           ON CONFLICT (user_id) DO UPDATE SET reason = $2, blacklisted_by = $3"#,
    )
    .bind(user_id)
    .bind(reason)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_user_blacklist(pool: &Pool, user_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM user_blacklist WHERE user_id = $1")
        .bind(user_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_server_blacklist(pool: &Pool) -> Result<Vec<BlacklistEntry>, sqlx::Error> {
    let rows =
        sqlx::query("SELECT guild_id, reason FROM server_blacklist ORDER BY created_at DESC")
            .fetch_all(pool)
            .await?;
    Ok(rows
        .into_iter()
        .map(|r| BlacklistEntry {
            id: r.get("guild_id"),
            reason: r.get("reason"),
        })
        .collect())
}

pub async fn add_server_blacklist(
    pool: &Pool,
    guild_id: &str,
    reason: Option<&str>,
    by: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO server_blacklist (guild_id, reason, blacklisted_by) VALUES ($1, $2, $3)
           ON CONFLICT (guild_id) DO UPDATE SET reason = $2, blacklisted_by = $3"#,
    )
    .bind(guild_id)
    .bind(reason)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_server_blacklist(pool: &Pool, guild_id: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM server_blacklist WHERE guild_id = $1")
        .bind(guild_id)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_global_domains(pool: &Pool) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>("SELECT domain FROM global_blocked_domains ORDER BY domain")
        .fetch_all(pool)
        .await
}

pub async fn add_global_domain(pool: &Pool, domain: &str, by: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO global_blocked_domains (domain, blocked_by) VALUES ($1, $2)
           ON CONFLICT (domain) DO NOTHING"#,
    )
    .bind(domain)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_global_domain(pool: &Pool, domain: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM global_blocked_domains WHERE domain = $1")
        .bind(domain)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_git_host(pool: &Pool, domain: &str, by: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO git_hosts (domain, added_by) VALUES ($1, $2)
           ON CONFLICT (domain) DO NOTHING"#,
    )
    .bind(domain)
    .bind(by)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn remove_git_host(pool: &Pool, domain: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM git_hosts WHERE domain = $1")
        .bind(domain)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn list_disabled_commands(pool: &Pool, scope: &str) -> Result<Vec<String>, sqlx::Error> {
    sqlx::query_scalar::<_, String>(
        "SELECT command FROM command_overrides WHERE scope = $1 AND enabled = FALSE",
    )
    .bind(scope)
    .fetch_all(pool)
    .await
}

pub async fn disable_command(pool: &Pool, scope: &str, command: &str) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO command_overrides (scope, command, enabled) VALUES ($1, $2, FALSE)
           ON CONFLICT (scope, command) DO UPDATE SET enabled = FALSE, updated_at = NOW()"#,
    )
    .bind(scope)
    .bind(command)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn enable_command(pool: &Pool, scope: &str, command: &str) -> Result<(), sqlx::Error> {
    sqlx::query("DELETE FROM command_overrides WHERE scope = $1 AND command = $2")
        .bind(scope)
        .bind(command)
        .execute(pool)
        .await?;
    Ok(())
}

#[derive(Clone)]
pub struct BotStatus {
    pub status_type: String,
    pub status_text: String,
    pub online_status: String,
}

pub async fn get_bot_status(pool: &Pool) -> Result<Option<BotStatus>, sqlx::Error> {
    let row = sqlx::query(
        "SELECT status_type, status_text, online_status FROM bot_status WHERE id = TRUE",
    )
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|r| BotStatus {
        status_type: r.get("status_type"),
        status_text: r.get("status_text"),
        online_status: r.get("online_status"),
    }))
}

pub async fn set_bot_status(
    pool: &Pool,
    status_type: &str,
    status_text: &str,
    online_status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"INSERT INTO bot_status (id, status_type, status_text, online_status)
           VALUES (TRUE, $1, $2, $3)
           ON CONFLICT (id) DO UPDATE SET
               status_type = $1, status_text = $2, online_status = $3, updated_at = NOW()"#,
    )
    .bind(status_type)
    .bind(status_text)
    .bind(online_status)
    .execute(pool)
    .await?;
    Ok(())
}

pub struct AuditEntry {
    pub actor_name: String,
    pub category: String,
    pub action: String,
    pub at: String,
}

pub async fn ensure_schema(pool: &Pool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS config_audit (
            id BIGSERIAL PRIMARY KEY,
            guild_id TEXT NOT NULL,
            actor_id TEXT NOT NULL,
            actor_name TEXT NOT NULL,
            category TEXT NOT NULL DEFAULT '',
            action TEXT NOT NULL,
            created_at TIMESTAMPTZ NOT NULL DEFAULT now()
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "ALTER TABLE config_audit ADD COLUMN IF NOT EXISTS category TEXT NOT NULL DEFAULT ''",
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS config_audit_guild_idx ON config_audit (guild_id, created_at DESC)",
    )
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_audit(
    pool: &Pool,
    guild_id: &str,
    actor_id: &str,
    actor_name: &str,
    category: &str,
    action: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO config_audit (guild_id, actor_id, actor_name, category, action) VALUES ($1, $2, $3, $4, $5)",
    )
    .bind(guild_id)
    .bind(actor_id)
    .bind(actor_name)
    .bind(category)
    .bind(action)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn count_audit(pool: &Pool, guild_id: &str) -> Result<i64, sqlx::Error> {
    let row = sqlx::query("SELECT COUNT(*) AS n FROM config_audit WHERE guild_id = $1")
        .bind(guild_id)
        .fetch_one(pool)
        .await?;
    row.try_get("n")
}

pub async fn list_audit(
    pool: &Pool,
    guild_id: &str,
    limit: i64,
    offset: i64,
) -> Result<Vec<AuditEntry>, sqlx::Error> {
    let rows = sqlx::query(
        r#"
        SELECT actor_name, category, action, to_char(created_at, 'YYYY-MM-DD HH24:MI') AS at
        FROM config_audit
        WHERE guild_id = $1
        ORDER BY created_at DESC
        LIMIT $2 OFFSET $3
        "#,
    )
    .bind(guild_id)
    .bind(limit)
    .bind(offset)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|r| AuditEntry {
            actor_name: r.try_get("actor_name").unwrap_or_default(),
            category: r.try_get("category").unwrap_or_default(),
            action: r.try_get("action").unwrap_or_default(),
            at: r.try_get("at").unwrap_or_default(),
        })
        .collect())
}
