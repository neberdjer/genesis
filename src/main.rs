mod commands;
mod constants;
mod db;
mod handlers;

use constants::{DEFAULT_ENVIRONMENT, DEFAULT_PREFIX};
use handlers::{
    handle_commit_diffs, handle_diff_pagination, handle_git_links, handle_instagram_links,
    handle_member_join, handle_tiktok_links, handle_twitter_links,
};
use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info, warn};

use sqlx::PgPool;
use std::sync::Arc;
use std::time::Instant;

pub struct Data {
    pub pool: Arc<PgPool>,
    pub start_time: Instant,
    pub owner_id: Option<serenity::UserId>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;

type Context<'a> = poise::Context<'a, Data, Error>;

struct Handler;

#[async_trait::async_trait]
impl serenity::EventHandler for Handler {
    async fn dispatch(&self, ctx: &serenity::Context, event: &serenity::FullEvent) {
        let data = ctx.data::<Data>();
        match event {
            serenity::FullEvent::Message { new_message, .. } => {
                if new_message.author.bot() {
                    return;
                }
                tokio::join!(
                    handle_commit_diffs(ctx, new_message, Some(&data.pool)),
                    handle_git_links(ctx, new_message, Some(&data.pool)),
                    handle_twitter_links(ctx, new_message, Some(&data.pool)),
                    handle_tiktok_links(ctx, new_message, Some(&data.pool)),
                    handle_instagram_links(ctx, new_message, Some(&data.pool)),
                );
            }
            serenity::FullEvent::InteractionCreate {
                interaction: serenity::Interaction::Component(component),
                ..
            } => {
                handle_diff_pagination(ctx, component).await;
            }
            serenity::FullEvent::GuildMemberAddition { new_member, .. } => {
                handle_member_join(ctx, new_member, Some(&data.pool)).await;
            }
            _ => {}
        }
    }
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            error!("Command '{}' failed: {}", ctx.command().name, error);
            let _ = ctx.say("An internal error occurred.").await;
        }
        poise::FrameworkError::MissingBotPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            warn!(
                "Missing permissions for '{}': {}",
                ctx.command().name,
                missing_permissions
            );
            let _ = ctx
                .say(format!(
                    "I'm missing the following permissions: **{}**.",
                    missing_permissions
                ))
                .await;
        }
        poise::FrameworkError::MissingUserPermissions {
            missing_permissions,
            ctx,
            ..
        } => {
            let perms = missing_permissions
                .map(|p| p.to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let _ = ctx
                .say(format!(
                    "You're missing the following permissions: **{}**.",
                    perms
                ))
                .await;
        }
        poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            if let Some(error) = error {
                error!("Check failed for '{}': {}", ctx.command().name, error);
                let _ = ctx
                    .say("You don't have permission to run this command.")
                    .await;
            }
        }
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                error!("Unhandled error handler failure: {}", e);
            }
        }
    }
}

/// Show help for all commands or a specific command
#[poise::command(slash_command, prefix_command)]
async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help for"] command: Option<String>,
) -> Result<(), Error> {
    let commands = &ctx.framework().options().commands;

    if let Some(name) = command {
        let target = name.trim().trim_start_matches('/');
        if let Some(cmd) = commands.iter().find(|c| c.name == target) {
            let desc = cmd.description.as_deref().unwrap_or("(no description)");
            let mut body = format!("**/{}** — {}", cmd.name, desc);
            if !cmd.subcommands.is_empty() {
                body.push_str("\n\n**Subcommands:**");
                for sub in &cmd.subcommands {
                    let sub_desc = sub.description.as_deref().unwrap_or("");
                    body.push_str(&format!("\n- `/{} {}` — {}", cmd.name, sub.name, sub_desc));
                }
            }
            ctx.say(body).await?;
        } else {
            ctx.say(format!("Command `{}` not found.", target)).await?;
        }
        return Ok(());
    }

    let mut lines = vec!["**Commands:**".to_string()];
    for cmd in commands {
        if cmd.hide_in_help {
            continue;
        }
        let desc = cmd.description.as_deref().unwrap_or("");
        lines.push(format!("- `/{}` — {}", cmd.name, desc));
    }
    lines.push("\nUse `/help <command>` for details on a specific command.".to_string());
    ctx.say(lines.join("\n")).await?;
    Ok(())
}

#[poise::command(prefix_command, owners_only)]
async fn register_commands(ctx: Context<'_>) -> Result<(), Error> {
    let commands = &ctx.framework().options().commands;
    poise::builtins::register_globally(ctx.http(), commands).await?;
    ctx.say("Commands registered globally.").await?;
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    rustls::crypto::ring::default_provider()
        .install_default()
        .expect("Failed to install rustls crypto provider");
    tracing_subscriber::fmt::init();

    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| DEFAULT_ENVIRONMENT.to_string());
    let prefix = env::var("PREFIX").unwrap_or_else(|_| DEFAULT_PREFIX.to_string());

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not found");
    info!("Connecting to database and running migrations...");
    let pool = db::connect(&database_url).await?;
    info!("Database connected and migrations applied successfully");

    info!(
        "Starting bot in {} mode with prefix '{}'",
        environment, prefix
    );

    let owner_id = env::var("OWNER_ID")
        .ok()
        .and_then(|id| id.parse::<u64>().ok())
        .map(serenity::UserId::new);

    let data = Data {
        pool: Arc::new(pool),
        start_time: Instant::now(),
        owner_id,
    };

    let mut all_commands = commands::all_commands();
    all_commands.push(help());
    all_commands.push(register_commands());

    let options = poise::FrameworkOptions {
        commands: all_commands,
        prefix_options: poise::PrefixFrameworkOptions {
            prefix: Some(prefix.into()),
            ..Default::default()
        },
        on_error: |error| Box::pin(on_error(error)),
        ..Default::default()
    };

    let token =
        serenity::Token::from_env("DISCORD_TOKEN").expect("DISCORD_TOKEN not found or invalid");

    let intents = serenity::GatewayIntents::non_privileged()
        | serenity::GatewayIntents::MESSAGE_CONTENT
        | serenity::GatewayIntents::GUILD_MEMBERS;

    let http = serenity::HttpBuilder::new(token.clone())
        .default_allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false))
        .build();

    let mut client = serenity::ClientBuilder::new_with_http(token, Arc::new(http), intents)
        .framework(Box::new(poise::Framework::new(options)))
        .event_handler(Arc::new(Handler))
        .data(Arc::new(data) as _)
        .await
        .map_err(|e| {
            error!("Failed to create client: {}", e);
            e
        })?;

    client.start().await.map_err(|e| {
        error!("Bot error: {}", e);
        e.into()
    })
}
