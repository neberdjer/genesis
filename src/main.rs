mod commands;
mod constants;
mod db;
mod handlers;

use constants::{
    DEFAULT_ENVIRONMENT, DEFAULT_ONLINE_STATUS, DEFAULT_PREFIX, DEFAULT_STATUS_TEXT,
    DEFAULT_STATUS_TYPE, STATUS_POLL_SECONDS, TOGGLEABLE_COMMANDS,
};
use handlers::{
    handle_commit_diffs, handle_diff_pagination, handle_git_links, handle_instagram_links,
    handle_member_join, handle_tiktok_links, handle_twitter_links,
};
use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info, warn};

use sqlx::PgPool;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

pub struct Data {
    pub pool: Arc<PgPool>,
    pub start_time: Instant,
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
            serenity::FullEvent::MessageDelete {
                deleted_message_id,
                guild_id,
                ..
            } => {
                handlers::reply_watch::handle_delete(ctx, *deleted_message_id, *guild_id).await;
            }
            serenity::FullEvent::Ready { .. } => {
                static POLLER_STARTED: AtomicBool = AtomicBool::new(false);
                if !POLLER_STARTED.swap(true, Ordering::SeqCst) {
                    tokio::spawn(status_poller(ctx.clone()));
                }
            }
            _ => {}
        }
    }
}

async fn status_poller(ctx: serenity::Context) {
    let data = ctx.data::<Data>();
    let mut interval = tokio::time::interval(Duration::from_secs(STATUS_POLL_SECONDS));
    let mut last: Option<(String, String, String)> = None;

    loop {
        interval.tick().await;
        match db::get_bot_status(&data.pool).await {
            Ok(Some(status)) => {
                if last.as_ref() != Some(&status) {
                    let (activity, online) = commands::moderation::status::presence_from_parts(
                        &status.0, &status.1, &status.2,
                    );
                    ctx.set_presence(Some(activity), online);
                    last = Some(status);
                }
            }
            Ok(None) => {}
            Err(e) => warn!("Failed to poll bot status: {}", e),
        }
    }
}

async fn record_command_usage(ctx: Context<'_>) {
    db::record_command(&ctx.data().pool, &ctx.command().qualified_name).await;
}

async fn command_enabled_check(ctx: Context<'_>) -> Result<bool, Error> {
    let name = ctx.command().name.to_string();
    if !TOGGLEABLE_COMMANDS.contains(&name.as_str()) {
        return Ok(true);
    }

    let guild_id = ctx.guild_id().map(|g| g.to_string());
    if db::is_command_enabled(&ctx.data().pool, guild_id.as_deref(), &name).await {
        return Ok(true);
    }

    let _ = ctx
        .send(
            poise::CreateReply::default()
                .content("This command has been disabled in this server by an administrator.")
                .ephemeral(true),
        )
        .await;
    Ok(false)
}

async fn reply_ephemeral(ctx: Context<'_>, message: &str) {
    let _ = ctx
        .send(
            poise::CreateReply::default()
                .content(message.to_string())
                .ephemeral(true),
        )
        .await;
}

async fn on_error(error: poise::FrameworkError<'_, Data, Error>) {
    match error {
        poise::FrameworkError::Command { error, ctx, .. } => {
            error!("Command '{}' failed: {}", ctx.command().name, error);
            reply_ephemeral(
                ctx,
                "Something went wrong while running that command. Please try again.",
            )
            .await;
        }
        poise::FrameworkError::ArgumentParse {
            error, input, ctx, ..
        } => {
            let usage = format!(
                "{}{} {}",
                ctx.prefix(),
                ctx.command().qualified_name,
                param_signature(ctx.command())
            );
            let detail = match input {
                Some(input) => format!("I couldn't understand `{}`: {}", input, error),
                None => format!("That command was used incorrectly: {}", error),
            };
            reply_ephemeral(ctx, &format!("{}\nUsage: `{}`", detail, usage.trim())).await;
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
            reply_ephemeral(
                ctx,
                &format!(
                    "I'm missing the following permissions: **{}**.",
                    missing_permissions
                ),
            )
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
            reply_ephemeral(
                ctx,
                &format!("You're missing the following permissions: **{}**.", perms),
            )
            .await;
        }
        poise::FrameworkError::CommandCheckFailed { error, ctx, .. } => {
            if let Some(error) = error {
                error!("Check failed for '{}': {}", ctx.command().name, error);
                reply_ephemeral(ctx, "You don't have permission to run this command.").await;
            }
        }
        other => {
            if let Err(e) = poise::builtins::on_error(other).await {
                error!("Unhandled error handler failure: {}", e);
            }
        }
    }
}

fn param_signature(cmd: &poise::Command<Data, Error>) -> String {
    cmd.parameters
        .iter()
        .map(|p| {
            if p.required {
                format!("<{}>", p.name)
            } else {
                format!("[{}]", p.name)
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

fn append_arguments(body: &mut String, cmd: &poise::Command<Data, Error>) {
    if cmd.parameters.is_empty() {
        return;
    }
    body.push_str("\n**Arguments:**");
    for param in &cmd.parameters {
        let req = if param.required {
            "required"
        } else {
            "optional"
        };
        let pdesc = param.description.as_deref().unwrap_or("");
        body.push_str(&format!("\n- `{}` ({}) - {}", param.name, req, pdesc));
    }
}

/// Show help for all commands or a specific command
#[poise::command(slash_command, prefix_command)]
async fn help(
    ctx: Context<'_>,
    #[description = "Specific command to show help for"] command: Option<String>,
) -> Result<(), Error> {
    let commands = &ctx.framework().options().commands;
    let prefix = ctx.prefix();

    if let Some(name) = command {
        let target = name
            .trim()
            .trim_start_matches(prefix)
            .trim_start_matches('/');
        let tokens: Vec<&str> = target.split_whitespace().collect();

        let resolved: Option<(String, &poise::Command<Data, Error>)> = match tokens.as_slice() {
            [top] => commands
                .iter()
                .find(|c| c.name == *top)
                .map(|c| (c.name.to_string(), c))
                .or_else(|| {
                    commands.iter().find_map(|c| {
                        c.subcommands
                            .iter()
                            .find(|s| s.name == *top)
                            .map(|s| (format!("{} {}", c.name, s.name), s))
                    })
                }),
            [top, sub, ..] => commands
                .iter()
                .find(|c| c.name == *top)
                .and_then(|c| c.subcommands.iter().find(|s| s.name == *sub))
                .map(|s| (format!("{} {}", top, sub), s)),
            [] => None,
        };

        if let Some((full_name, cmd)) = resolved {
            let desc = cmd.description.as_deref().unwrap_or("(no description)");
            let mut body = format!("**{}{}** - {}", prefix, full_name, desc);

            if !cmd.parameters.is_empty() {
                body.push_str(&format!(
                    "\n\n**Usage:** `{}{} {}`\n",
                    prefix,
                    full_name,
                    param_signature(cmd)
                ));
                append_arguments(&mut body, cmd);
            }

            if !cmd.subcommands.is_empty() {
                body.push_str("\n\n**Subcommands:**");
                for sub in &cmd.subcommands {
                    let sub_desc = sub.description.as_deref().unwrap_or("");
                    let sig = param_signature(sub);
                    if sig.is_empty() {
                        body.push_str(&format!(
                            "\n- `{}{} {}` - {}",
                            prefix, full_name, sub.name, sub_desc
                        ));
                    } else {
                        body.push_str(&format!(
                            "\n- `{}{} {} {}` - {}",
                            prefix, full_name, sub.name, sig, sub_desc
                        ));
                    }
                    let mut arg_block = String::new();
                    append_arguments(&mut arg_block, sub);
                    body.push_str(&arg_block.replace('\n', "\n  "));
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
        lines.push(format!("- `{}{}` - {}", prefix, cmd.name, desc));
    }
    lines.push(format!(
        "\nUse `{}help <command>` for details on a specific command.",
        prefix
    ));
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

    let data = Data {
        pool: Arc::new(pool),
        start_time: Instant::now(),
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
        command_check: Some(|ctx| Box::pin(command_enabled_check(ctx))),
        pre_command: |ctx| Box::pin(record_command_usage(ctx)),
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

    let (status_type, status_text, online_status) = db::get_bot_status(&data.pool)
        .await
        .ok()
        .flatten()
        .unwrap_or_else(|| {
            (
                DEFAULT_STATUS_TYPE.to_string(),
                DEFAULT_STATUS_TEXT.to_string(),
                DEFAULT_ONLINE_STATUS.to_string(),
            )
        });
    let (activity, status) = commands::moderation::status::presence_from_parts(
        &status_type,
        &status_text,
        &online_status,
    );

    let mut client = serenity::ClientBuilder::new_with_http(token, Arc::new(http), intents)
        .framework(Box::new(poise::Framework::new(options)))
        .event_handler(Arc::new(Handler))
        .activity(activity)
        .status(status)
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
