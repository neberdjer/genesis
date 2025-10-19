mod commands;
mod constants;
mod git_handler;
mod handlers;

use constants::{DEFAULT_ENVIRONMENT, DEFAULT_PREFIX};
use handlers::handle_git_links;
use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, (), Error>;

fn event_handler(
    ctx: &serenity::Context,
    event: &poise::serenity_prelude::FullEvent,
    _framework: poise::FrameworkContext<'_, (), Error>,
    _data: &(),
) -> impl std::future::Future<Output = Result<(), Error>> + Send {
    async move {
        if let poise::serenity_prelude::FullEvent::Message { new_message } = event {
            handle_git_links(ctx, new_message).await;
        }
        Ok(())
    }
}

async fn setup_handler(
    ctx: &serenity::Context,
    ready: &serenity::Ready,
    framework: &poise::Framework<(), Error>,
    environment: String,
    prefix: String,
) -> Result<(), Error> {
    info!(
        "Bot connected as {} in {} mode with prefix '{}'",
        ready.user.name, environment, prefix
    );
    info!(
        "Registering {} commands globally",
        framework.options().commands.len()
    );
    poise::builtins::register_globally(ctx, &framework.options().commands).await?;
    info!("Commands registered successfully");
    Ok(())
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| DEFAULT_ENVIRONMENT.to_string());
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not found");
    let prefix = env::var("PREFIX").unwrap_or_else(|_| DEFAULT_PREFIX.to_string());

    let env_clone = environment.clone();
    let prefix_clone = prefix.clone();

    let framework = poise::Framework::<(), Error>::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all_commands(),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some(prefix.into()),
                ..Default::default()
            },
            event_handler: |ctx, event, framework, data| {
                Box::pin(event_handler(ctx, event, framework, data))
            },
            ..Default::default()
        })
        .setup(move |ctx, ready, framework| {
            let env = env_clone.clone();
            let pref = prefix_clone.clone();
            Box::pin(setup_handler(ctx, ready, framework, env, pref))
        })
        .build();

    let intents =
        serenity::GatewayIntents::non_privileged() | serenity::GatewayIntents::MESSAGE_CONTENT;

    let mut client = serenity::ClientBuilder::new(token, intents)
        .framework(framework)
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
