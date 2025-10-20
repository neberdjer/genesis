#[cfg(feature = "database")]
use sqlx::{postgres::PgPoolOptions, PgPool, Row};
#[cfg(feature = "database")]
use std::fs;
#[cfg(feature = "database")]
use std::path::Path;
#[cfg(feature = "database")]
use std::time::Duration;
#[cfg(feature = "database")]
use tracing::{error, info, warn};

#[cfg(feature = "database")]
#[derive(Debug, Clone)]
pub struct ServerSettings {
    #[allow(dead_code)]
    pub guild_id: String,
    pub git_diffs_enabled: bool,
    pub git_compares_enabled: bool,
    pub git_links_enabled: bool,
    pub twitter_enabled: bool,
}

#[cfg(feature = "database")]
pub async fn connect(database_url: &str) -> Result<PgPool, sqlx::Error> {
    let pool = PgPoolOptions::new()
        .max_connections(10)
        .acquire_timeout(Duration::from_secs(30))
        .idle_timeout(Some(Duration::from_secs(600)))
        .max_lifetime(Some(Duration::from_secs(1800)))
        .connect(database_url)
        .await?;

    create_migrations_table(&pool).await?;
    run_migrations(&pool).await?;

    Ok(pool)
}

#[cfg(feature = "database")]
async fn create_migrations_table(pool: &PgPool) -> Result<(), sqlx::Error> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS schema_migrations (
            version TEXT PRIMARY KEY,
            applied_at TIMESTAMPTZ DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

#[cfg(feature = "database")]
async fn run_migrations(pool: &PgPool) -> Result<(), sqlx::Error> {
    let migrations_dir = Path::new("migrations");

    if !migrations_dir.exists() {
        warn!("Migrations directory not found, skipping migrations");
        return Ok(());
    }

    let mut migration_files = Vec::new();

    match fs::read_dir(migrations_dir) {
        Ok(entries) => {
            for entry in entries {
                if let Ok(entry) = entry {
                    let path = entry.path();
                    if path.extension().and_then(|s| s.to_str()) == Some("sql") {
                        if let Some(file_name) = path.file_name().and_then(|s| s.to_str()) {
                            migration_files.push(file_name.to_string());
                        }
                    }
                }
            }
        }
        Err(e) => {
            error!("Failed to read migrations directory: {}", e);
            return Err(sqlx::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                format!("Failed to read migrations directory: {}", e),
            )));
        }
    }

    migration_files.sort();

    for migration_file in migration_files {
        let applied = sqlx::query_scalar::<_, bool>(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = $1)",
        )
        .bind(&migration_file)
        .fetch_one(pool)
        .await?;

        if applied {
            info!("Migration {} already applied, skipping", migration_file);
            continue;
        }

        let migration_path = migrations_dir.join(&migration_file);
        let migration_sql = match fs::read_to_string(&migration_path) {
            Ok(content) => content,
            Err(e) => {
                error!("Failed to read migration file {}: {}", migration_file, e);
                return Err(sqlx::Error::Io(e));
            }
        };

        info!("Running migration: {}", migration_file);

        let mut tx = pool.begin().await?;

        let statements: Vec<&str> = migration_sql
            .split(';')
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();

        for statement in statements {
            if let Err(e) = sqlx::query(statement).execute(&mut *tx).await {
                error!("Failed to execute migration {}: {}", migration_file, e);
                return Err(e);
            }
        }

        sqlx::query("INSERT INTO schema_migrations (version) VALUES ($1)")
            .bind(&migration_file)
            .execute(&mut *tx)
            .await?;

        tx.commit().await?;
        info!("Successfully applied migration: {}", migration_file);
    }

    Ok(())
}

#[cfg(feature = "database")]
pub async fn get_server_settings(pool: &PgPool, guild_id: &str) -> Result<ServerSettings, sqlx::Error> {
    let result = sqlx::query(
        r#"
        SELECT guild_id, git_diffs_enabled, git_compares_enabled, git_links_enabled, twitter_enabled
        FROM server_settings
        WHERE guild_id = $1
        "#,
    )
    .bind(guild_id)
    .fetch_optional(pool)
    .await?;

    if let Some(row) = result {
        Ok(ServerSettings {
            guild_id: row.try_get("guild_id")?,
            git_diffs_enabled: row.try_get("git_diffs_enabled")?,
            git_compares_enabled: row.try_get("git_compares_enabled")?,
            git_links_enabled: row.try_get("git_links_enabled")?,
            twitter_enabled: row.try_get("twitter_enabled")?,
        })
    } else {
        Ok(ServerSettings {
            guild_id: guild_id.to_string(),
            git_diffs_enabled: true,
            git_compares_enabled: true,
            git_links_enabled: true,
            twitter_enabled: true,
        })
    }
}

#[cfg(feature = "database")]
pub async fn update_server_setting(
    pool: &PgPool,
    guild_id: &str,
    setting: &str,
    enabled: bool,
) -> Result<(), sqlx::Error> {
    let query = match setting {
        "git_diffs" => {
            sqlx::query(
                r#"
                INSERT INTO server_settings (guild_id, git_diffs_enabled)
                VALUES ($1, $2)
                ON CONFLICT (guild_id)
                DO UPDATE SET git_diffs_enabled = $2, updated_at = NOW()
                "#,
            )
            .bind(guild_id)
            .bind(enabled)
        }
        "git_compares" => {
            sqlx::query(
                r#"
                INSERT INTO server_settings (guild_id, git_compares_enabled)
                VALUES ($1, $2)
                ON CONFLICT (guild_id)
                DO UPDATE SET git_compares_enabled = $2, updated_at = NOW()
                "#,
            )
            .bind(guild_id)
            .bind(enabled)
        }
        "git_links" => {
            sqlx::query(
                r#"
                INSERT INTO server_settings (guild_id, git_links_enabled)
                VALUES ($1, $2)
                ON CONFLICT (guild_id)
                DO UPDATE SET git_links_enabled = $2, updated_at = NOW()
                "#,
            )
            .bind(guild_id)
            .bind(enabled)
        }
        "twitter" => {
            sqlx::query(
                r#"
                INSERT INTO server_settings (guild_id, twitter_enabled)
                VALUES ($1, $2)
                ON CONFLICT (guild_id)
                DO UPDATE SET twitter_enabled = $2, updated_at = NOW()
                "#,
            )
            .bind(guild_id)
            .bind(enabled)
        }
        _ => return Err(sqlx::Error::RowNotFound),
    };

    query.execute(pool).await?;
    Ok(())
}
