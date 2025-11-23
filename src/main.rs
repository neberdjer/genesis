mod commands;
mod constants;
mod db;
mod handlers;

use constants::{DEFAULT_ENVIRONMENT, DEFAULT_PREFIX};
use handlers::{
    handle_commit_diffs, handle_diff_pagination, handle_git_links, handle_instagram_links,
    handle_tiktok_links, handle_twitter_links,
};
use poise::serenity_prelude as serenity;
use std::env;
use tracing::{error, info};

use sqlx::PgPool;
use std::sync::Arc;

#[derive(Clone)]
pub struct Data {
    pub pool: Arc<PgPool>,
}

type Error = Box<dyn std::error::Error + Send + Sync>;

type Context<'a> = poise::Context<'a, Data, Error>;

fn event_handler(
    ctx: &serenity::Context,
    event: &poise::serenity_prelude::FullEvent,
    _framework: poise::FrameworkContext<'_, Data, Error>,
    data: &Data,
) -> impl std::future::Future<Output = Result<(), Error>> + Send {
    async move {
        match event {
            poise::serenity_prelude::FullEvent::Message { new_message } => {
                handle_commit_diffs(ctx, new_message, Some(&data.pool)).await;
                handle_git_links(ctx, new_message, Some(&data.pool)).await;
                handle_twitter_links(ctx, new_message, Some(&data.pool)).await;
                handle_tiktok_links(ctx, new_message, Some(&data.pool)).await;
                handle_instagram_links(ctx, new_message, Some(&data.pool)).await;
            }
            poise::serenity_prelude::FullEvent::InteractionCreate { interaction } => {
                if let serenity::Interaction::Component(component) = interaction {
                    handle_diff_pagination(ctx, component).await;
                }
            }
            _ => {}
        }
        Ok(())
    }
}

async fn setup_handler(
    ctx: &serenity::Context,
    ready: &serenity::Ready,
    framework: &poise::Framework<Data, Error>,
    environment: String,
    prefix: String,
    data: Data,
) -> Result<Data, Error> {
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
    Ok(data)
}

#[tokio::main]
async fn main() -> Result<(), Error> {
    dotenv::dotenv().ok();
    tracing_subscriber::fmt::init();

    let environment = env::var("ENVIRONMENT").unwrap_or_else(|_| DEFAULT_ENVIRONMENT.to_string());
    let token = env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not found");
    let prefix = env::var("PREFIX").unwrap_or_else(|_| DEFAULT_PREFIX.to_string());

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not found");
    info!("Connecting to database and running migrations...");
    let pool = db::connect(&database_url).await?;
    info!("Database connected and migrations applied successfully");

    let data = Data {
        pool: Arc::new(pool),
    };

    let env_clone = environment.clone();
    let prefix_clone = prefix.clone();

    let framework = poise::Framework::<Data, Error>::builder()
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
            let data_clone = data.clone();
            Box::pin(setup_handler(ctx, ready, framework, env, pref, data_clone))
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
