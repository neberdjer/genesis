use super::shared;
use crate::constants::{FAILURE_TOO_LONG, PAGE_CACHE_TTL_SECONDS};
use poise::serenity_prelude as serenity;
use std::time::Instant;
use tracing::{error, warn};

pub fn is_cache_expired(timestamp: Instant) -> bool {
    timestamp.elapsed().as_secs() > PAGE_CACHE_TTL_SECONDS
}

pub fn split_lock_suffix(rest: &str) -> (&str, Option<u64>) {
    match rest.rsplit_once(':') {
        Some((payload, uid)) => match uid.parse::<u64>() {
            Ok(uid) => (payload, Some(uid)),
            Err(_) => (rest, None),
        },
        None => (rest, None),
    }
}

pub fn pagination_row(
    prev_id: String,
    next_id: String,
    current_page: usize,
    total_pages: usize,
) -> serenity::CreateActionRow<'static> {
    let prev_button = serenity::CreateButton::new(prev_id)
        .label("Previous")
        .style(serenity::ButtonStyle::Primary)
        .disabled(current_page == 0);

    let page_info = serenity::CreateButton::new("page_info")
        .label(format!("{}/{}", current_page + 1, total_pages))
        .style(serenity::ButtonStyle::Secondary)
        .disabled(true);

    let next_button = serenity::CreateButton::new(next_id)
        .label("Next")
        .style(serenity::ButtonStyle::Primary)
        .disabled(current_page >= total_pages - 1);

    serenity::CreateActionRow::Buttons(vec![prev_button, page_info, next_button].into())
}

pub async fn send_first_page(
    ctx: &serenity::Context,
    msg: &serenity::Message,
    service: &'static str,
    first_page: &str,
    buttons: Option<serenity::CreateActionRow<'_>>,
) -> Result<(), &'static str> {
    let Some(content) = shared::try_with_sent_by(first_page, msg.author.id) else {
        warn!("Paged response too long, skipping");
        shared::report_failure(
            ctx,
            msg.guild_id,
            service,
            FAILURE_TOO_LONG,
            None,
            "first page exceeds the message limit",
        );
        return Err(FAILURE_TOO_LONG);
    };

    let mut message_builder = serenity::CreateMessage::new()
        .content(content)
        .reference_message(msg)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));

    if let Some(buttons) = buttons {
        message_builder =
            message_builder.components(vec![serenity::CreateComponent::ActionRow(buttons)]);
    }

    shared::send_reply(ctx, msg, service, message_builder)
        .await
        .map_or(Ok(()), Err)
}

pub async fn respond_ephemeral(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    content: &str,
) {
    let response = serenity::CreateInteractionResponse::Message(
        serenity::CreateInteractionResponseMessage::new()
            .content(content.to_string())
            .ephemeral(true),
    );
    let _ = interaction.create_response(&ctx.http, response).await;
}

pub async fn update_to_page(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    page_content: &str,
    buttons: serenity::CreateActionRow<'_>,
) {
    let footer = shared::extract_sent_by_footer(&interaction.message.content).unwrap_or("");
    let content = shared::fit_with_footer(page_content.to_string(), footer);

    let response = serenity::CreateInteractionResponse::UpdateMessage(
        serenity::CreateInteractionResponseMessage::new()
            .content(content)
            .components(vec![serenity::CreateComponent::ActionRow(buttons)]),
    );

    if let Err(e) = interaction.create_response(&ctx.http, response).await {
        error!("Failed to update paginated message: {}", e);
    }
}
