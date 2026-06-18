use std::collections::HashSet;
use std::env;

#[derive(Clone)]
pub struct Config {
    pub client_id: String,
    pub client_secret: String,
    pub bot_token: String,
    pub redirect_uri: String,
    pub invite_permissions: String,
    pub contact_email: String,
    pub support_invite: String,
    pub cookie_secure: bool,
    pub owner_ids: HashSet<String>,
}

impl Config {
    pub fn from_env() -> Self {
        let base_url = env::var("BASE_URL").unwrap_or_else(|_| "http://localhost:8080".to_string());
        let redirect_uri = env::var("DISCORD_REDIRECT_URI")
            .unwrap_or_else(|_| format!("{}/auth/callback", base_url));
        let cookie_secure = base_url.starts_with("https://");

        Self {
            client_id: env::var("DISCORD_CLIENT_ID").expect("DISCORD_CLIENT_ID not set"),
            client_secret: env::var("DISCORD_CLIENT_SECRET")
                .expect("DISCORD_CLIENT_SECRET not set"),
            bot_token: env::var("DISCORD_TOKEN").expect("DISCORD_TOKEN not set"),
            redirect_uri,
            invite_permissions: env::var("INVITE_PERMISSIONS")
                .unwrap_or_else(|_| "274877991936".to_string()),
            contact_email: env::var("CONTACT_EMAIL")
                .unwrap_or_else(|_| "support@example.com".to_string()),
            support_invite: env::var("SUPPORT_INVITE").unwrap_or_default(),
            cookie_secure,
            owner_ids: env::var("OWNER_ID")
                .unwrap_or_default()
                .split(',')
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(String::from)
                .collect(),
        }
    }

    pub fn is_owner(&self, user_id: &str) -> bool {
        self.owner_ids.contains(user_id)
    }

    pub fn invite_url(&self) -> String {
        format!(
            "https://discord.com/oauth2/authorize?client_id={}&permissions={}&integration_type=0&scope=bot%20applications.commands",
            self.client_id, self.invite_permissions
        )
    }

    pub fn invite_url_for_guild(&self, guild_id: &str) -> String {
        format!(
            "{}&guild_id={}&disable_guild_select=true",
            self.invite_url(),
            guild_id
        )
    }

    pub fn support_invite_url(&self) -> Option<String> {
        let v = self.support_invite.trim();
        if v.is_empty() {
            return None;
        }
        if v.starts_with("http://") || v.starts_with("https://") {
            return Some(v.to_string());
        }
        let code = v
            .trim_start_matches("discord.gg/")
            .trim_start_matches("discord.com/invite/")
            .trim_start_matches('/');
        Some(format!("https://discord.gg/{code}"))
    }

    pub fn user_install_url(&self) -> String {
        format!(
            "https://discord.com/oauth2/authorize?client_id={}&integration_type=1&scope=applications.commands",
            self.client_id
        )
    }

    pub fn oauth_authorize_url(&self, state: &str) -> String {
        format!(
            "https://discord.com/oauth2/authorize?client_id={}&redirect_uri={}&response_type=code&scope=identify%20guilds&state={}",
            self.client_id,
            urlencode(&self.redirect_uri),
            state
        )
    }
}

pub fn urlencode(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for byte in s.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{:02X}", byte)),
        }
    }
    out
}
