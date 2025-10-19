mod commands;

use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};

type Error = Box<dyn std::error::Error + Send + Sync>;
type Context<'a> = poise::Context<'a, (), Error>;

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();

    tracing_subscriber::fmt::init();

    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| "prod".to_string());
    let token = env::var("DISCORD_TOKEN").expect("Expected DISCORD_TOKEN in environment");
    let prefix = env::var("PREFIX").unwrap_or_else(|_| "!".to_string());

    let framework = poise::Framework::<(), Error>::builder()
        .options(poise::FrameworkOptions {
            commands: commands::all_commands(),
            prefix_options: poise::PrefixFrameworkOptions {
                prefix: Some(prefix.clone().into()),
                ..Default::default()
            },
            ..Default::default()
        })
        .setup(|ctx, ready, framework| {
            Box::pin(async move {
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
            })
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

    if let Err(e) = client.start().await {
        error!("Bot encountered an error: {}", e);
        return Err(e.into());
    }

    Ok(())
}
