use crate::config::Config;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

const API: &str = "https://discord.com/api/v10";
const PERM_ADMINISTRATOR: u64 = 1 << 3;
const PERM_MANAGE_GUILD: u64 = 1 << 5;

#[derive(Deserialize)]
struct TokenResponse {
    access_token: String,
}

#[derive(Deserialize, Serialize, Clone)]
pub struct User {
    pub id: String,
    pub username: String,
    pub avatar: Option<String>,
}

impl User {
    pub fn avatar_url(&self) -> String {
        match &self.avatar {
            Some(hash) => format!(
                "https://cdn.discordapp.com/avatars/{}/{}.png?size=64",
                self.id, hash
            ),
            None => "https://cdn.discordapp.com/embed/avatars/0.png".to_string(),
        }
    }
}

#[derive(Deserialize)]
struct PartialGuild {
    id: String,
    name: String,
    icon: Option<String>,
    #[serde(default)]
    owner: bool,
    #[serde(default)]
    permissions: String,
    #[serde(default)]
    approximate_member_count: u64,
}

fn guild_icon_url(id: &str, icon: &Option<String>) -> String {
    match icon {
        Some(hash) => format!("https://cdn.discordapp.com/icons/{id}/{hash}.png?size=64"),
        None => "https://cdn.discordapp.com/embed/avatars/0.png".to_string(),
    }
}

#[derive(Clone)]
pub struct DashGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub bot_present: bool,
    pub can_manage: bool,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum GuildState {
    Configure,
    AddBot,
    Active,
    CantAdd,
}

impl DashGuild {
    pub fn state(&self) -> GuildState {
        match (self.bot_present, self.can_manage) {
            (true, true) => GuildState::Configure,
            (false, true) => GuildState::AddBot,
            (true, false) => GuildState::Active,
            (false, false) => GuildState::CantAdd,
        }
    }

    fn rank(&self) -> u8 {
        match self.state() {
            GuildState::Configure => 0,
            GuildState::AddBot => 1,
            GuildState::Active => 2,
            GuildState::CantAdd => 3,
        }
    }

    pub fn icon_url(&self) -> String {
        guild_icon_url(&self.id, &self.icon)
    }
}

pub async fn exchange_code(
    http: &reqwest::Client,
    config: &Config,
    code: &str,
) -> Result<String, reqwest::Error> {
    let params = [
        ("client_id", config.client_id.as_str()),
        ("client_secret", config.client_secret.as_str()),
        ("grant_type", "authorization_code"),
        ("code", code),
        ("redirect_uri", config.redirect_uri.as_str()),
    ];

    let resp: TokenResponse = http
        .post(format!("{}/oauth2/token", API))
        .form(&params)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(resp.access_token)
}

pub async fn current_user(
    http: &reqwest::Client,
    access_token: &str,
) -> Result<User, reqwest::Error> {
    http.get(format!("{}/users/@me", API))
        .bearer_auth(access_token)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await
}

async fn fetch_all_guilds(
    http: &reqwest::Client,
    authorization: &str,
    with_counts: bool,
) -> Result<Vec<PartialGuild>, reqwest::Error> {
    const PAGE: usize = 200;
    let counts = if with_counts { "&with_counts=true" } else { "" };
    let mut all: Vec<PartialGuild> = Vec::new();
    let mut after = String::from("0");

    loop {
        let page: Vec<PartialGuild> = http
            .get(format!(
                "{}/users/@me/guilds?limit={}&after={}{}",
                API, PAGE, after, counts
            ))
            .header("Authorization", authorization)
            .send()
            .await?
            .error_for_status()?
            .json()
            .await?;

        let len = page.len();
        let next = page.iter().filter_map(|g| g.id.parse::<u64>().ok()).max();
        all.extend(page);

        if len < PAGE {
            break;
        }
        match next {
            Some(max) if max.to_string() != after => after = max.to_string(),
            _ => break,
        }
    }

    Ok(all)
}

async fn user_guilds(
    http: &reqwest::Client,
    access_token: &str,
) -> Result<Vec<PartialGuild>, reqwest::Error> {
    fetch_all_guilds(http, &format!("Bearer {}", access_token), false).await
}

pub async fn bot_guild_ids(
    http: &reqwest::Client,
    config: &Config,
) -> Result<HashSet<String>, reqwest::Error> {
    let guilds = fetch_all_guilds(http, &format!("Bot {}", config.bot_token), false).await?;
    Ok(guilds.into_iter().map(|g| g.id).collect())
}

#[derive(Clone)]
pub struct BotGuild {
    pub id: String,
    pub name: String,
    pub icon: Option<String>,
    pub member_count: u64,
}

impl BotGuild {
    pub fn icon_url(&self) -> String {
        guild_icon_url(&self.id, &self.icon)
    }
}

pub async fn bot_guilds_detailed(
    http: &reqwest::Client,
    config: &Config,
) -> Result<Vec<BotGuild>, reqwest::Error> {
    let guilds = fetch_all_guilds(http, &format!("Bot {}", config.bot_token), true).await?;
    Ok(guilds
        .into_iter()
        .map(|g| BotGuild {
            id: g.id,
            name: g.name,
            icon: g.icon,
            member_count: g.approximate_member_count,
        })
        .collect())
}

#[derive(Deserialize)]
pub struct GuildChannel {
    pub id: String,
    pub name: String,
    #[serde(rename = "type")]
    pub kind: u8,
    #[serde(default)]
    pub position: i64,
}

pub async fn guild_channels(
    http: &reqwest::Client,
    config: &Config,
    guild_id: &str,
) -> Result<Vec<GuildChannel>, reqwest::Error> {
    let mut channels: Vec<GuildChannel> = http
        .get(format!("{}/guilds/{}/channels", API, guild_id))
        .header("Authorization", format!("Bot {}", config.bot_token))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    channels.retain(|c| c.kind == 0 || c.kind == 5);
    channels.sort_by_key(|c| c.position);
    Ok(channels)
}

#[derive(Deserialize)]
pub struct GuildRole {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub position: i64,
    #[serde(default)]
    pub managed: bool,
}

pub async fn guild_roles(
    http: &reqwest::Client,
    config: &Config,
    guild_id: &str,
) -> Result<Vec<GuildRole>, reqwest::Error> {
    let mut roles: Vec<GuildRole> = http
        .get(format!("{}/guilds/{}/roles", API, guild_id))
        .header("Authorization", format!("Bot {}", config.bot_token))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;
    roles.retain(|r| r.id != guild_id && !r.managed);
    roles.sort_by_key(|r| std::cmp::Reverse(r.position));
    Ok(roles)
}

#[derive(Clone, Copy)]
pub struct AppStats {
    pub guild_count: u64,
    pub user_install_count: u64,
}

#[derive(Deserialize)]
struct Application {
    #[serde(default)]
    approximate_guild_count: u64,
    #[serde(default)]
    approximate_user_install_count: u64,
}

pub async fn app_stats(
    http: &reqwest::Client,
    config: &Config,
) -> Result<AppStats, reqwest::Error> {
    let app: Application = http
        .get(format!("{}/applications/@me", API))
        .header("Authorization", format!("Bot {}", config.bot_token))
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    Ok(AppStats {
        guild_count: app.approximate_guild_count,
        user_install_count: app.approximate_user_install_count,
    })
}

pub async fn dashboard_guilds(
    http: &reqwest::Client,
    access_token: &str,
    bot_ids: &HashSet<String>,
) -> Result<Vec<DashGuild>, reqwest::Error> {
    let guilds = user_guilds(http, access_token).await?;

    let mut result: Vec<DashGuild> = guilds
        .into_iter()
        .map(|g| {
            let can_manage = g.owner
                || g.permissions.parse::<u64>().unwrap_or(0)
                    & (PERM_ADMINISTRATOR | PERM_MANAGE_GUILD)
                    != 0;
            DashGuild {
                bot_present: bot_ids.contains(&g.id),
                can_manage,
                id: g.id,
                name: g.name,
                icon: g.icon,
            }
        })
        .collect();

    result.sort_by(|a, b| {
        a.rank()
            .cmp(&b.rank())
            .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
    });

    Ok(result)
}
