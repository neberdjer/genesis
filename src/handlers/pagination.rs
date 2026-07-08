use super::shared;
use crate::constants::{DISCORD_MESSAGE_LIMIT, PAGE_CACHE_TTL_SECONDS};
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
) -> bool {
    let footer = format!("\n-# Sent by <@{}>", msg.author.id);
    if first_page.len() + footer.len() > DISCORD_MESSAGE_LIMIT {
        warn!("Paged response too long, skipping");
        shared::record_embed(ctx, service, false).await;
        return false;
    }

    let content = format!("{}{}", first_page, footer);
    let mut message_builder = serenity::CreateMessage::new()
        .content(content)
        .reference_message(msg)
        .allowed_mentions(serenity::CreateAllowedMentions::new().replied_user(false));

    if let Some(buttons) = buttons {
        message_builder =
            message_builder.components(vec![serenity::CreateComponent::ActionRow(buttons)]);
    }

    shared::send_reply(ctx, msg, service, message_builder).await
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

fn extract_sent_by_footer(content: &str) -> Option<String> {
    let marker = "\n-# Sent by <@";
    let start = content.rfind(marker)?;
    let rest = &content[start..];
    let end = rest.find('>')?;
    Some(rest[..=end].to_string())
}

pub async fn update_to_page(
    ctx: &serenity::Context,
    interaction: &serenity::ComponentInteraction,
    page_content: &str,
    buttons: serenity::CreateActionRow<'_>,
) {
    let content = match extract_sent_by_footer(&interaction.message.content) {
        Some(footer) => format!("{}{}", page_content, footer),
        None => page_content.to_string(),
    };

    let response = serenity::CreateInteractionResponse::UpdateMessage(
        serenity::CreateInteractionResponseMessage::new()
            .content(content)
            .components(vec![serenity::CreateComponent::ActionRow(buttons)]),
    );

    if let Err(e) = interaction.create_response(&ctx.http, response).await {
        error!("Failed to update paginated message: {}", e);
    }
}
