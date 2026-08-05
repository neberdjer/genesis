mod catalog;
mod config;
mod db;
mod discord;
mod session;
mod views;

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum_extra::extract::PrivateCookieJar;
use axum_extra::extract::cookie::Key;
use config::Config;
use rand::Rng;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::env;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};
use tower::Layer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tracing::{error, info};

type BotGuildCache = Arc<tokio::sync::Mutex<Option<(Instant, Arc<HashSet<String>>)>>>;
type BotGuildDetailCache = Arc<tokio::sync::Mutex<Option<(Instant, Arc<Vec<discord::BotGuild>>)>>>;
type UserGuildCache =
    Arc<tokio::sync::Mutex<HashMap<String, (Instant, Arc<Vec<discord::DashGuild>>)>>>;

#[derive(Clone)]
struct AppState {
    pool: db::Pool,
    http: reqwest::Client,
    config: Arc<Config>,
    key: Key,
    bot_guilds: BotGuildCache,
    bot_refreshing: Arc<AtomicBool>,
    app_stats: Arc<tokio::sync::Mutex<Option<(Instant, discord::AppStats)>>>,
    app_stats_refreshing: Arc<AtomicBool>,
    user_guilds: UserGuildCache,
    save_hits: Arc<tokio::sync::Mutex<HashMap<String, Vec<Instant>>>>,
    bot_guilds_detailed: BotGuildDetailCache,
    bot_guilds_detailed_refreshing: Arc<AtomicBool>,
    analytics: Arc<tokio::sync::Mutex<Option<(Instant, db::Analytics)>>>,
    analytics_refreshing: Arc<AtomicBool>,
}

impl axum::extract::FromRef<AppState> for Key {
    fn from_ref(state: &AppState) -> Self {
        state.key.clone()
    }
}

#[tokio::main]
async fn main() {
    dotenv::dotenv().ok();
    dotenv::from_filename("../.env").ok();
    tracing_subscriber::fmt::init();

    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL not set");
    let pool = db::connect(&database_url)
        .await
        .expect("failed to connect to database");
    if let Err(e) = db::ensure_schema(&pool).await {
        error!("failed to ensure web schema: {}", e);
    }

    let cookie_secret = env::var("COOKIE_SECRET").expect("COOKIE_SECRET not set");
    let key = Key::derive_from(cookie_secret.as_bytes());

    let http = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .expect("failed to build http client");

    let state = AppState {
        pool,
        http,
        config: Arc::new(Config::from_env()),
        key,
        bot_guilds: Arc::new(tokio::sync::Mutex::new(None)),
        bot_refreshing: Arc::new(AtomicBool::new(false)),
        app_stats: Arc::new(tokio::sync::Mutex::new(None)),
        app_stats_refreshing: Arc::new(AtomicBool::new(false)),
        user_guilds: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        save_hits: Arc::new(tokio::sync::Mutex::new(HashMap::new())),
        bot_guilds_detailed: Arc::new(tokio::sync::Mutex::new(None)),
        bot_guilds_detailed_refreshing: Arc::new(AtomicBool::new(false)),
        analytics: Arc::new(tokio::sync::Mutex::new(None)),
        analytics_refreshing: Arc::new(AtomicBool::new(false)),
    };

    {
        let warm = state.clone();
        tokio::spawn(async move {
            match warm.bot_guilds().await {
                Ok(guilds) => {
                    let ids: HashSet<String> = guilds.iter().map(|g| g.id.clone()).collect();
                    *warm.bot_guilds.lock().await = Some((Instant::now(), Arc::new(ids)));
                }
                Err(e) => error!("initial bot guild fetch failed: {}", e),
            }
            let _ = warm.app_stats().await;
            let _ = warm.analytics().await;
        });
    }

    let static_dir = env::var("STATIC_DIR").unwrap_or_else(|_| "static".to_string());

    let static_service = SetResponseHeaderLayer::overriding(
        axum::http::header::CACHE_CONTROL,
        axum::http::HeaderValue::from_static("no-cache"),
    )
    .layer(ServeDir::new(static_dir));

    let app = Router::new()
        .route("/", get(landing))
        .route("/login", get(login))
        .route("/auth/callback", get(callback))
        .route("/logout", get(logout))
        .route("/invite", get(invite))
        .route("/dashboard", get(dashboard))
        .route("/dashboard/{guild_id}", get(guild_config))
        .route("/dashboard/{guild_id}/settings", post(save_settings))
        .route("/dashboard/{guild_id}/welcome", post(save_welcome))
        .route(
            "/dashboard/{guild_id}/report-channel",
            post(save_report_channel),
        )
        .route("/dashboard/{guild_id}/domains", post(save_domains))
        .route("/dashboard/{guild_id}/domains/add", post(add_guild_domain))
        .route(
            "/dashboard/{guild_id}/domains/remove",
            post(remove_guild_domain),
        )
        .route("/dashboard/{guild_id}/commands", post(save_commands))
        .route("/dashboard/{guild_id}/delete", post(delete_server))
        .route("/account/delete", post(delete_account))
        .route("/owner", get(owner_page))
        .route("/owner/blacklist/user/add", post(owner_user_add))
        .route("/owner/blacklist/user/remove", post(owner_user_remove))
        .route("/owner/blacklist/server/add", post(owner_server_add))
        .route("/owner/blacklist/server/remove", post(owner_server_remove))
        .route("/owner/domains/add", post(owner_domain_add))
        .route("/owner/domains/remove", post(owner_domain_remove))
        .route("/owner/git-hosts/add", post(owner_git_add))
        .route("/owner/git-hosts/remove", post(owner_git_remove))
        .route("/owner/media-hosts/add", post(owner_media_add))
        .route("/owner/media-hosts/remove", post(owner_media_remove))
        .route("/owner/commands", post(owner_commands_save))
        .route("/owner/status", post(owner_status_save))
        .route("/features", get(features))
        .route("/commands", get(commands))
        .route("/tos", get(tos))
        .route("/privacy", get(privacy))
        .route("/faq", get(faq))
        .nest_service("/static", static_service)
        .with_state(state);

    let bind = env::var("BIND").unwrap_or_else(|_| "0.0.0.0:8080".to_string());
    let listener = tokio::net::TcpListener::bind(&bind)
        .await
        .expect("failed to bind");
    info!("genesis-web listening on {}", bind);
    axum::serve(listener, app).await.expect("server error");
}

fn html(markup: maud::Markup) -> Html<String> {
    Html(markup.into_string())
}

async fn landing(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Query(query): Query<HashMap<String, String>>,
) -> Html<String> {
    let session = session::read_session(&jar);
    state.ensure_bot_guilds_fresh(Duration::from_secs(60)).await;
    let mut stats = state.app_stats().await;
    if let Some(s) = stats.as_mut()
        && let Some(n) = state.bot_guild_count().await
    {
        s.guild_count = n as u64;
    }
    let users = state.bot_user_count().await;
    let analytics = state.analytics().await;
    html(views::landing(
        &state.config,
        stats,
        users,
        analytics,
        session.as_ref().map(|s| &s.user),
        query.get("deleted").map(String::as_str),
    ))
}

async fn features(jar: PrivateCookieJar) -> Html<String> {
    let session = session::read_session(&jar);
    html(views::features(session.as_ref().map(|s| &s.user)))
}

async fn commands(jar: PrivateCookieJar) -> Html<String> {
    let session = session::read_session(&jar);
    html(views::commands(session.as_ref().map(|s| &s.user)))
}

async fn tos(State(state): State<AppState>, jar: PrivateCookieJar) -> Html<String> {
    let session = session::read_session(&jar);
    html(views::tos(&state.config, session.as_ref().map(|s| &s.user)))
}

async fn privacy(State(state): State<AppState>, jar: PrivateCookieJar) -> Html<String> {
    let session = session::read_session(&jar);
    html(views::privacy(
        &state.config,
        session.as_ref().map(|s| &s.user),
    ))
}

async fn faq(State(state): State<AppState>, jar: PrivateCookieJar) -> Html<String> {
    let session = session::read_session(&jar);
    html(views::faq(&state.config, session.as_ref().map(|s| &s.user)))
}

async fn login(State(state): State<AppState>, jar: PrivateCookieJar) -> impl IntoResponse {
    let token: String = {
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| format!("{:x}", rng.gen_range(0..16)))
            .collect()
    };
    let jar = session::write_state(jar, &token, state.config.cookie_secure);
    let url = state.config.oauth_authorize_url(&token);
    (jar, Redirect::to(&url))
}

#[derive(Deserialize)]
struct CallbackQuery {
    code: Option<String>,
    state: Option<String>,
}

async fn callback(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Query(query): Query<CallbackQuery>,
) -> Response {
    let expected = session::read_state(&jar);
    let jar = session::clear_state(jar);

    let (Some(code), Some(returned_state)) = (query.code, query.state) else {
        return html(views::error_page("missing authorization code.", None)).into_response();
    };

    if expected.as_deref() != Some(returned_state.as_str()) {
        return html(views::error_page(
            "invalid oauth state. please try again.",
            None,
        ))
        .into_response();
    }

    let access_token = match discord::exchange_code(&state.http, &state.config, &code).await {
        Ok(t) => t,
        Err(e) => {
            error!("token exchange failed: {}", e);
            return html(views::error_page("discord login failed.", None)).into_response();
        }
    };

    let user = match discord::current_user(&state.http, &access_token).await {
        Ok(u) => u,
        Err(e) => {
            error!("fetching user failed: {}", e);
            return html(views::error_page(
                "could not load your discord profile.",
                None,
            ))
            .into_response();
        }
    };

    let jar = session::write_session(
        jar,
        &session::Session { user, access_token },
        state.config.cookie_secure,
    );
    (jar, Redirect::to("/dashboard")).into_response()
}

async fn logout(jar: PrivateCookieJar) -> impl IntoResponse {
    let jar = session::clear_session(jar);
    (jar, Redirect::to("/"))
}

async fn invite(State(state): State<AppState>) -> Redirect {
    Redirect::to(&state.config.invite_url())
}

async fn delete_server(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
) -> Response {
    let Some(session) = session::read_session(&jar) else {
        return Redirect::to("/login").into_response();
    };
    let access = load_guild(&state, &session, &guild_id).await;
    if let Some(resp) = deny_mutation(&access, &session, &format!("/dashboard/{}", guild_id)) {
        return resp;
    }
    if let Err(e) = db::delete_server_data(&state.pool, &guild_id).await {
        error!("delete_server_data failed: {}", e);
    }
    (
        session::clear_session(jar),
        Redirect::to("/?deleted=server"),
    )
        .into_response()
}

async fn delete_account(State(state): State<AppState>, jar: PrivateCookieJar) -> Response {
    let Some(session) = session::read_session(&jar) else {
        return Redirect::to("/login").into_response();
    };
    if let Err(e) = db::delete_user_data(&state.pool, &session.user.id).await {
        error!("delete_user_data failed: {}", e);
    }
    (
        session::clear_session(jar),
        Redirect::to("/?deleted=account"),
    )
        .into_response()
}

async fn dashboard(State(state): State<AppState>, jar: PrivateCookieJar) -> Response {
    let Some(session) = session::read_session(&jar) else {
        return Redirect::to("/login").into_response();
    };

    state
        .refresh_bot_guilds_if_stale(Duration::from_secs(30))
        .await;

    match state
        .dashboard_guilds(&session.user.id, &session.access_token)
        .await
    {
        Ok(guilds) => {
            let is_owner = state.config.is_owner(&session.user.id);
            html(views::dashboard(
                &session.user,
                &guilds,
                &state.config,
                is_owner,
            ))
            .into_response()
        }
        Err(e) => {
            error!("fetching guilds failed: {}", e);
            html(views::error_page(
                "could not load your servers right now. try again in a moment.",
                Some(&session.user),
            ))
            .into_response()
        }
    }
}

enum GuildAccess {
    Ok(discord::DashGuild),
    Forbidden,
    Unavailable,
}

async fn load_guild(state: &AppState, session: &session::Session, guild_id: &str) -> GuildAccess {
    match state
        .dashboard_guilds(&session.user.id, &session.access_token)
        .await
    {
        Ok(guilds) => guilds
            .iter()
            .find(|g| g.id == guild_id && g.bot_present && g.can_manage)
            .map(|g| GuildAccess::Ok(g.clone()))
            .unwrap_or(GuildAccess::Forbidden),
        Err(e) => {
            error!("fetching guilds failed: {}", e);
            GuildAccess::Unavailable
        }
    }
}

async fn guild_config(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let Some(session) = session::read_session(&jar) else {
        return Redirect::to("/login").into_response();
    };

    let guild = match load_guild(&state, &session, &guild_id).await {
        GuildAccess::Ok(g) => g,
        GuildAccess::Forbidden => return forbidden(&session),
        GuildAccess::Unavailable => return unavailable(&session.user),
    };

    let tab = match query.get("tab").map(String::as_str) {
        Some("commands") => "commands",
        Some("welcome") => "welcome",
        Some("domains") => "domains",
        Some("audit") => "audit",
        Some("failures") => "failures",
        Some("danger") => "danger",
        _ => "services",
    };
    let (saved, jar) = session::take_saved(jar);

    let settings = state.pool_settings(&guild_id).await;

    let mut domains = Vec::new();
    let mut git_hosts = Vec::new();
    let mut media_hosts = Vec::new();
    let mut disabled_commands = Vec::new();
    let mut channels = Vec::new();
    let mut roles = Vec::new();
    let mut audit_log = Vec::new();
    let mut audit_page = 1usize;
    let mut audit_total_pages = 1usize;
    let mut failures = Vec::new();
    let mut failure_codes = Vec::new();
    let mut failure_code = None;
    let mut failure_page = 1usize;
    let mut failure_total_pages = 1usize;
    match tab {
        "commands" => {
            disabled_commands = db::list_disabled_commands(&state.pool, &guild_id)
                .await
                .unwrap_or_default();
        }
        "domains" => {
            let (d, gh, mh) = tokio::join!(
                db::list_domains(&state.pool, &guild_id),
                db::list_git_hosts(&state.pool),
                db::list_media_hosts(&state.pool),
            );
            domains = d.unwrap_or_default();
            git_hosts = gh.unwrap_or_default();
            media_hosts = mh.unwrap_or_default();
        }
        "welcome" => {
            let (c, r) = tokio::join!(
                discord::guild_channels(&state.http, &state.config, &guild_id),
                discord::guild_roles(&state.http, &state.config, &guild_id),
            );
            channels = c.unwrap_or_default();
            roles = r.unwrap_or_default();
        }
        "audit" => {
            let total = db::count_audit(&state.pool, &guild_id)
                .await
                .unwrap_or(0)
                .max(0) as usize;
            let (p, tp, offset) = paginate(&query, total);
            audit_page = p;
            audit_total_pages = tp;
            audit_log = db::list_audit(&state.pool, &guild_id, PAGE_SIZE as i64, offset as i64)
                .await
                .unwrap_or_default();
        }
        "failures" => {
            failure_codes = db::failure_code_counts(&state.pool, Some(&guild_id))
                .await
                .unwrap_or_default();
            failure_code = query
                .get("type")
                .filter(|c| failure_codes.iter().any(|(k, _)| k == *c))
                .cloned();
            let total = failure_total(&failure_codes, failure_code.as_deref());
            let (p, tp, offset) = paginate(&query, total);
            failure_page = p;
            failure_total_pages = tp;
            failures = db::list_failures(
                &state.pool,
                Some(&guild_id),
                failure_code.as_deref(),
                PAGE_SIZE as i64,
                offset as i64,
            )
            .await
            .unwrap_or_default();
        }
        "services" => {
            channels = discord::guild_channels(&state.http, &state.config, &guild_id)
                .await
                .unwrap_or_default();
        }
        _ => {}
    }

    (
        jar,
        html(views::guild_config(
            &session.user,
            &guild,
            tab,
            &settings,
            &domains,
            &git_hosts,
            &media_hosts,
            &disabled_commands,
            &channels,
            &roles,
            &audit_log,
            audit_page,
            audit_total_pages,
            &failures,
            &failure_codes,
            failure_code.as_deref(),
            failure_page,
            failure_total_pages,
            saved,
        )),
    )
        .into_response()
}

fn ajax_gate(access: &GuildAccess) -> Option<StatusCode> {
    match access {
        GuildAccess::Ok(_) => None,
        GuildAccess::Forbidden => Some(StatusCode::FORBIDDEN),
        GuildAccess::Unavailable => Some(StatusCode::SERVICE_UNAVAILABLE),
    }
}

#[allow(clippy::result_large_err)]
async fn form_guild_gate(
    state: &AppState,
    jar: &PrivateCookieJar,
    guild_id: &str,
    dest: &str,
) -> Result<session::Session, Response> {
    let Some(session) = session::read_session(jar) else {
        return Err(Redirect::to("/login").into_response());
    };
    let access = load_guild(state, &session, guild_id).await;
    if let Some(resp) = deny_mutation(&access, &session, dest) {
        return Err(resp);
    }
    if !state.allow_save(&session.user.id).await {
        return Err(Redirect::to(dest).into_response());
    }
    Ok(session)
}

async fn ajax_guild_gate(
    state: &AppState,
    jar: &PrivateCookieJar,
    guild_id: &str,
) -> Result<session::Session, Response> {
    let Some(session) = session::read_session(jar) else {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    };
    if let Some(code) = ajax_gate(&load_guild(state, &session, guild_id).await) {
        return Err(code.into_response());
    }
    if !state.allow_save(&session.user.id).await {
        return Err(StatusCode::TOO_MANY_REQUESTS.into_response());
    }
    Ok(session)
}

#[allow(clippy::result_large_err)]
async fn ajax_owner_gate(
    state: &AppState,
    jar: &PrivateCookieJar,
) -> Result<session::Session, Response> {
    let Some(session) = session::read_session(jar) else {
        return Err(StatusCode::UNAUTHORIZED.into_response());
    };
    if !state.config.is_owner(&session.user.id) {
        return Err(StatusCode::FORBIDDEN.into_response());
    }
    if !state.allow_save(&session.user.id).await {
        return Err(StatusCode::TOO_MANY_REQUESTS.into_response());
    }
    Ok(session)
}

async fn sync_command_overrides(pool: &db::Pool, scope: &str, form: &HashMap<String, String>) {
    for name in catalog::toggleable_commands() {
        let result = if form.contains_key(name) {
            db::enable_command(pool, scope, name).await
        } else {
            db::disable_command(pool, scope, name).await
        };
        if let Err(e) = result {
            error!("command override failed for {} ({}): {}", name, scope, e);
        }
    }
}

async fn audit(
    state: &AppState,
    guild_id: &str,
    session: &session::Session,
    category: &str,
    action: &str,
) {
    if let Err(e) = db::add_audit(
        &state.pool,
        guild_id,
        &session.user.id,
        &session.user.username,
        category,
        action,
    )
    .await
    {
        error!("audit log write failed: {}", e);
    }
}

fn join_changes(groups: &[(&str, &[&str])]) -> Option<String> {
    let segs: Vec<String> = groups
        .iter()
        .filter(|(_, items)| !items.is_empty())
        .map(|(verb, items)| format!("{} {}", verb, items.join(", ")))
        .collect();
    (!segs.is_empty()).then(|| segs.join("; "))
}

const PAGE_SIZE: usize = 10;

fn failure_total(counts: &[(String, i64)], code: Option<&str>) -> usize {
    let n: i64 = match code {
        Some(code) => counts
            .iter()
            .find(|(c, _)| c == code)
            .map(|(_, n)| *n)
            .unwrap_or(0),
        None => counts.iter().map(|(_, n)| n).sum(),
    };
    n.max(0) as usize
}

fn paginate(query: &HashMap<String, String>, total: usize) -> (usize, usize, usize) {
    let total_pages = total.div_ceil(PAGE_SIZE).max(1);
    let page = query
        .get("page")
        .and_then(|p| p.parse::<usize>().ok())
        .unwrap_or(1)
        .clamp(1, total_pages);
    (page, total_pages, (page - 1) * PAGE_SIZE)
}

async fn save_settings(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let session = match ajax_guild_gate(&state, &jar, &guild_id).await {
        Ok(session) => session,
        Err(resp) => return resp,
    };

    let old = state.pool_settings(&guild_id).await;
    let settings = db::Settings {
        git_diffs: form.contains_key("git_diffs"),
        git_compares: form.contains_key("git_compares"),
        git_links: form.contains_key("git_links"),
        twitter: form.contains_key("twitter"),
        tiktok: form.contains_key("tiktok"),
        instagram: form.contains_key("instagram"),
        reply_cleanup: form.contains_key("reply_cleanup"),
        ..db::Settings::defaults()
    };
    match db::set_settings(&state.pool, &guild_id, &settings).await {
        Ok(()) => {
            let mut on = Vec::new();
            let mut off = Vec::new();
            for (key, label) in db::SERVICES {
                match (form.contains_key(*key), old.enabled(key)) {
                    (true, false) => on.push(*label),
                    (false, true) => off.push(*label),
                    _ => {}
                }
            }
            match (settings.reply_cleanup, old.reply_cleanup) {
                (true, false) => on.push("reply cleanup"),
                (false, true) => off.push("reply cleanup"),
                _ => {}
            }
            if let Some(s) = join_changes(&[("enabled", &on), ("disabled", &off)]) {
                audit(&state, &guild_id, &session, "services", &s).await;
            }
            StatusCode::NO_CONTENT.into_response()
        }
        Err(e) => {
            error!("set_settings failed: {}", e);
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}

#[derive(Deserialize)]
struct WelcomeForm {
    enabled: Option<String>,
    #[serde(default)]
    channel_id: String,
    #[serde(default)]
    message: String,
    #[serde(default)]
    role_id: String,
}

async fn save_welcome(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Form(form): Form<WelcomeForm>,
) -> Response {
    let Some(session) = session::read_session(&jar) else {
        return Redirect::to("/login").into_response();
    };
    let access = load_guild(&state, &session, &guild_id).await;
    if let Some(resp) = deny_mutation(
        &access,
        &session,
        &format!("/dashboard/{}?tab=welcome", guild_id),
    ) {
        return resp;
    }

    let old = state.pool_settings(&guild_id).await;
    let enabled = form.enabled.is_some();
    let channel = snowflake(&form.channel_id);
    let role = snowflake(&form.role_id);
    let message = match form.message.trim() {
        "" => db::DEFAULT_WELCOME,
        m => m,
    };

    let dest = format!("/dashboard/{}?tab=welcome", guild_id);
    match db::set_welcome(&state.pool, &guild_id, enabled, channel, message, role).await {
        Ok(()) => {
            let mut changes: Vec<&str> = Vec::new();
            if old.welcome_enabled != enabled {
                changes.push(if enabled { "turned on" } else { "turned off" });
            }
            if old.welcome_channel_id.as_deref() != channel {
                changes.push("changed channel");
            }
            if old.welcome_role_id.as_deref() != role {
                changes.push("changed role");
            }
            if old.welcome_message.as_str() != message {
                changes.push("changed message");
            }
            if !changes.is_empty() {
                audit(&state, &guild_id, &session, "welcome", &changes.join(", ")).await;
            }
            (
                session::set_saved(jar, state.config.cookie_secure),
                Redirect::to(&dest),
            )
                .into_response()
        }
        Err(e) => {
            error!("set_welcome failed: {}", e);
            Redirect::to(&dest).into_response()
        }
    }
}

#[derive(serde::Deserialize)]
struct ReportChannelForm {
    #[serde(default)]
    channel_id: String,
}

async fn save_report_channel(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Form(form): Form<ReportChannelForm>,
) -> Response {
    let dest = format!("/dashboard/{}?tab=services", guild_id);
    let session = match form_guild_gate(&state, &jar, &guild_id, &dest).await {
        Ok(session) => session,
        Err(resp) => return resp,
    };

    let old = state.pool_settings(&guild_id).await;
    let channel = snowflake(&form.channel_id);
    match db::set_report_channel(&state.pool, &guild_id, channel).await {
        Ok(()) => {
            if old.report_channel_id.as_deref() != channel {
                let action = if channel.is_some() {
                    "changed report channel"
                } else {
                    "disabled failure reports"
                };
                audit(&state, &guild_id, &session, "reports", action).await;
            }
            (
                session::set_saved(jar, state.config.cookie_secure),
                Redirect::to(&dest),
            )
                .into_response()
        }
        Err(e) => {
            error!("set_report_channel failed: {}", e);
            Redirect::to(&dest).into_response()
        }
    }
}

async fn save_domains(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let dest = format!("/dashboard/{}?tab=domains", guild_id);
    let session = match form_guild_gate(&state, &jar, &guild_id, &dest).await {
        Ok(session) => session,
        Err(resp) => return resp,
    };

    let (current, git_hosts, media_hosts) = tokio::join!(
        db::list_domains(&state.pool, &guild_id),
        db::list_git_hosts(&state.pool),
        db::list_media_hosts(&state.pool),
    );
    let current = current.unwrap_or_default();
    let git_hosts = git_hosts.unwrap_or_default();
    let media_hosts = media_hosts.unwrap_or_default();
    let current_set: HashSet<&str> = current.iter().map(String::as_str).collect();

    let candidates: HashSet<&str> = catalog::DOMAIN_GROUPS
        .iter()
        .flat_map(|g| g.domains.iter().copied())
        .chain(catalog::extra_mirrors(&git_hosts, &media_hosts))
        .collect();

    let mut blocked: Vec<&str> = Vec::new();
    let mut unblocked: Vec<&str> = Vec::new();
    for domain in candidates {
        let ticked = form.contains_key(domain);
        let already = current_set.contains(domain);
        match (ticked, already) {
            (true, false) => {
                match db::add_domain(&state.pool, &guild_id, domain, &session.user.id).await {
                    Ok(()) => blocked.push(domain),
                    Err(e) => error!("blocked-domain update failed for {}: {}", domain, e),
                }
            }
            (false, true) => match db::remove_domain(&state.pool, &guild_id, domain).await {
                Ok(()) => unblocked.push(domain),
                Err(e) => error!("blocked-domain update failed for {}: {}", domain, e),
            },
            _ => {}
        }
    }

    if let Some(s) = join_changes(&[("blocked", &blocked), ("unblocked", &unblocked)]) {
        audit(&state, &guild_id, &session, "domains", &s).await;
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to(&dest),
    )
        .into_response()
}

#[derive(serde::Deserialize)]
struct DomainForm {
    #[serde(default)]
    domain: String,
}

async fn add_guild_domain(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Form(f): Form<DomainForm>,
) -> Response {
    guild_domain(state, jar, guild_id, f, true).await
}

async fn remove_guild_domain(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Form(f): Form<DomainForm>,
) -> Response {
    guild_domain(state, jar, guild_id, f, false).await
}

async fn guild_domain(
    state: AppState,
    jar: PrivateCookieJar,
    guild_id: String,
    f: DomainForm,
    block: bool,
) -> Response {
    let dest = format!("/dashboard/{}?tab=domains", guild_id);
    let session = match form_guild_gate(&state, &jar, &guild_id, &dest).await {
        Ok(session) => session,
        Err(resp) => return resp,
    };

    let domain = clean_domain(&f.domain).or_else(|| {
        let raw = f.domain.trim().to_ascii_lowercase();
        (!block && !raw.is_empty()).then_some(raw)
    });

    if let Some(d) = domain {
        let result = if block {
            db::add_domain(&state.pool, &guild_id, &d, &session.user.id).await
        } else {
            db::remove_domain(&state.pool, &guild_id, &d).await
        };
        match result {
            Ok(()) => {
                let verb = if block { "blocked" } else { "unblocked" };
                audit(
                    &state,
                    &guild_id,
                    &session,
                    "domains",
                    &format!("{verb} {d}"),
                )
                .await;
            }
            Err(e) => error!("blocked-domain update failed for {}: {}", d, e),
        }
    }

    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to(&dest),
    )
        .into_response()
}

async fn save_commands(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Path(guild_id): Path<String>,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    let session = match ajax_guild_gate(&state, &jar, &guild_id).await {
        Ok(session) => session,
        Err(resp) => return resp,
    };

    let was_disabled: HashSet<String> = db::list_disabled_commands(&state.pool, &guild_id)
        .await
        .unwrap_or_default()
        .into_iter()
        .collect();
    sync_command_overrides(&state.pool, &guild_id, &form).await;

    let mut on = Vec::new();
    let mut off = Vec::new();
    for name in catalog::toggleable_commands() {
        match (form.contains_key(name), !was_disabled.contains(name)) {
            (true, false) => on.push(name),
            (false, true) => off.push(name),
            _ => {}
        }
    }
    if let Some(s) = join_changes(&[("enabled", &on), ("disabled", &off)]) {
        audit(&state, &guild_id, &session, "commands", &s).await;
    }
    StatusCode::NO_CONTENT.into_response()
}

#[allow(clippy::result_large_err)]
fn owner_guard(state: &AppState, jar: &PrivateCookieJar) -> Result<session::Session, Response> {
    let Some(session) = session::read_session(jar) else {
        return Err(Redirect::to("/login").into_response());
    };
    if !state.config.is_owner(&session.user.id) {
        return Err((
            StatusCode::FORBIDDEN,
            html(views::error_page(
                "you don't have access to this page.",
                Some(&session.user),
            )),
        )
            .into_response());
    }
    Ok(session)
}

fn blank(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty()).then_some(t)
}

fn clean_domain(s: &str) -> Option<String> {
    let d = s
        .trim()
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_start_matches("www.")
        .split('/')
        .next()
        .unwrap_or("")
        .to_ascii_lowercase();
    (d.contains('.')
        && d.len() <= 253
        && d.bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'-'))
    .then_some(d)
}

#[derive(Deserialize)]
struct OwnerIdForm {
    id: String,
    #[serde(default)]
    reason: String,
}

#[derive(Deserialize)]
struct OwnerDomainForm {
    domain: String,
}

#[derive(Deserialize)]
struct OwnerMediaForm {
    service: String,
    domain: String,
}

#[derive(Deserialize)]
struct StatusForm {
    status_type: String,
    #[serde(default)]
    status_text: String,
    online_status: String,
}

async fn owner_page(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Query(query): Query<HashMap<String, String>>,
) -> Response {
    let session = match owner_guard(&state, &jar) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let tab = match query.get("tab").map(String::as_str) {
        Some("servers") => "servers",
        Some("domains") => "domains",
        Some("commands") => "commands",
        Some("status") => "status",
        Some("failures") => "failures",
        _ => "blacklist",
    };
    let (saved, jar) = session::take_saved(jar);

    let mut users = Vec::new();
    let mut servers = Vec::new();
    let mut global_domains = Vec::new();
    let mut git_hosts = Vec::new();
    let mut media_hosts = Vec::new();
    let mut disabled = Vec::new();
    let mut status = None;
    let mut bot_servers: Vec<discord::BotGuild> = Vec::new();
    let mut query_str = String::new();
    let mut page = 1usize;
    let mut total_pages = 1usize;
    let mut total = 0usize;
    let mut failures = Vec::new();
    let mut failure_codes = Vec::new();
    let mut failure_code = None;
    match tab {
        "servers" => {
            let (all, bl) =
                tokio::join!(state.bot_guilds(), db::list_server_blacklist(&state.pool));
            servers = bl.unwrap_or_default();
            let mut all: Vec<discord::BotGuild> = (*all.unwrap_or_default()).clone();

            query_str = query
                .get("q")
                .map(|s| s.trim().to_string())
                .unwrap_or_default();
            if !query_str.is_empty() {
                let needle = query_str.to_lowercase();
                all.retain(|g| g.name.to_lowercase().contains(&needle));
            }
            all.sort_by_cached_key(|g| (std::cmp::Reverse(g.member_count), g.name.to_lowercase()));

            total = all.len();
            let (p, tp, offset) = paginate(&query, total);
            page = p;
            total_pages = tp;
            bot_servers = all.into_iter().skip(offset).take(PAGE_SIZE).collect();
        }
        "domains" => {
            let (g, gh, mh) = tokio::join!(
                db::list_global_domains(&state.pool),
                db::list_git_hosts(&state.pool),
                db::list_media_hosts(&state.pool),
            );
            global_domains = g.unwrap_or_default();
            git_hosts = gh.unwrap_or_default();
            media_hosts = mh.unwrap_or_default();
        }
        "commands" => {
            disabled = db::list_disabled_commands(&state.pool, "global")
                .await
                .unwrap_or_default();
        }
        "status" => {
            status = db::get_bot_status(&state.pool).await.ok().flatten();
        }
        "failures" => {
            failure_codes = db::failure_code_counts(&state.pool, None)
                .await
                .unwrap_or_default();
            failure_code = query
                .get("type")
                .filter(|c| failure_codes.iter().any(|(k, _)| k == *c))
                .cloned();
            total = failure_total(&failure_codes, failure_code.as_deref());
            let (p, tp, offset) = paginate(&query, total);
            page = p;
            total_pages = tp;
            failures = db::list_failures(
                &state.pool,
                None,
                failure_code.as_deref(),
                PAGE_SIZE as i64,
                offset as i64,
            )
            .await
            .unwrap_or_default();
        }
        _ => {
            users = db::list_user_blacklist(&state.pool)
                .await
                .unwrap_or_default();
        }
    }

    (
        jar,
        html(views::owner(
            &session.user,
            tab,
            &users,
            &servers,
            &global_domains,
            &git_hosts,
            &media_hosts,
            &disabled,
            status.as_ref(),
            &bot_servers,
            &query_str,
            page,
            total_pages,
            total,
            &failures,
            &failure_codes,
            failure_code.as_deref(),
            saved,
        )),
    )
        .into_response()
}

async fn owner_user_add(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerIdForm>,
) -> Response {
    let session = match owner_guard(&state, &jar) {
        Ok(s) => s,
        Err(r) => return r,
    };
    if let Some(id) = snowflake(&f.id)
        && let Err(e) =
            db::add_user_blacklist(&state.pool, id, blank(&f.reason), &session.user.id).await
    {
        error!("add_user_blacklist failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=blacklist"),
    )
        .into_response()
}

async fn owner_user_remove(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerIdForm>,
) -> Response {
    if let Err(r) = owner_guard(&state, &jar) {
        return r;
    }
    if let Some(id) = snowflake(&f.id)
        && let Err(e) = db::remove_user_blacklist(&state.pool, id).await
    {
        error!("remove_user_blacklist failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=blacklist"),
    )
        .into_response()
}

async fn owner_server_add(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerIdForm>,
) -> Response {
    let session = match owner_guard(&state, &jar) {
        Ok(s) => s,
        Err(r) => return r,
    };
    if let Some(id) = snowflake(&f.id)
        && let Err(e) =
            db::add_server_blacklist(&state.pool, id, blank(&f.reason), &session.user.id).await
    {
        error!("add_server_blacklist failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=servers"),
    )
        .into_response()
}

async fn owner_server_remove(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerIdForm>,
) -> Response {
    if let Err(r) = owner_guard(&state, &jar) {
        return r;
    }
    if let Some(id) = snowflake(&f.id)
        && let Err(e) = db::remove_server_blacklist(&state.pool, id).await
    {
        error!("remove_server_blacklist failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=servers"),
    )
        .into_response()
}

async fn owner_domain_add(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerDomainForm>,
) -> Response {
    let session = match owner_guard(&state, &jar) {
        Ok(s) => s,
        Err(r) => return r,
    };
    if let Some(d) = clean_domain(&f.domain)
        && let Err(e) = db::add_global_domain(&state.pool, &d, &session.user.id).await
    {
        error!("add_global_domain failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=domains"),
    )
        .into_response()
}

async fn owner_domain_remove(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerDomainForm>,
) -> Response {
    if let Err(r) = owner_guard(&state, &jar) {
        return r;
    }
    if let Err(e) = db::remove_global_domain(&state.pool, f.domain.trim()).await {
        error!("remove_global_domain failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=domains"),
    )
        .into_response()
}

async fn owner_git_add(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerDomainForm>,
) -> Response {
    let session = match owner_guard(&state, &jar) {
        Ok(s) => s,
        Err(r) => return r,
    };
    if let Some(d) = clean_domain(&f.domain)
        && let Err(e) = db::add_git_host(&state.pool, &d, &session.user.id).await
    {
        error!("add_git_host failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=domains"),
    )
        .into_response()
}

async fn owner_git_remove(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerDomainForm>,
) -> Response {
    if let Err(r) = owner_guard(&state, &jar) {
        return r;
    }
    if let Err(e) = db::remove_git_host(&state.pool, f.domain.trim()).await {
        error!("remove_git_host failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=domains"),
    )
        .into_response()
}

async fn owner_media_add(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerMediaForm>,
) -> Response {
    let session = match owner_guard(&state, &jar) {
        Ok(s) => s,
        Err(r) => return r,
    };
    let valid_service = catalog::media_services().any(|g| g.key == f.service);
    if valid_service
        && let Some(d) = clean_domain(&f.domain)
        && let Err(e) = db::add_media_host(&state.pool, &f.service, &d, &session.user.id).await
    {
        error!("add_media_host failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=domains"),
    )
        .into_response()
}

async fn owner_media_remove(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<OwnerMediaForm>,
) -> Response {
    if let Err(r) = owner_guard(&state, &jar) {
        return r;
    }
    if let Err(e) = db::remove_media_host(&state.pool, f.service.trim(), f.domain.trim()).await {
        error!("remove_media_host failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=domains"),
    )
        .into_response()
}

async fn owner_commands_save(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(form): Form<HashMap<String, String>>,
) -> Response {
    if let Err(resp) = ajax_owner_gate(&state, &jar).await {
        return resp;
    }
    sync_command_overrides(&state.pool, "global", &form).await;
    StatusCode::NO_CONTENT.into_response()
}

async fn owner_status_save(
    State(state): State<AppState>,
    jar: PrivateCookieJar,
    Form(f): Form<StatusForm>,
) -> Response {
    if let Err(r) = owner_guard(&state, &jar) {
        return r;
    }
    let type_ok = catalog::ACTIVITY_TYPES.contains(&f.status_type.as_str());
    let online_ok = catalog::ONLINE_STATES.contains(&f.online_status.as_str());
    if type_ok
        && online_ok
        && let Err(e) = db::set_bot_status(
            &state.pool,
            &f.status_type,
            f.status_text.trim(),
            &f.online_status,
        )
        .await
    {
        error!("set_bot_status failed: {}", e);
    }
    (
        session::set_saved(jar, state.config.cookie_secure),
        Redirect::to("/owner?tab=status"),
    )
        .into_response()
}

fn forbidden(session: &session::Session) -> Response {
    (
        StatusCode::FORBIDDEN,
        html(views::error_page(
            "you can't manage that server, or the bot isn't in it.",
            Some(&session.user),
        )),
    )
        .into_response()
}

fn unavailable(user: &discord::User) -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        html(views::error_page(
            "couldn't load that server right now. try again in a moment.",
            Some(user),
        )),
    )
        .into_response()
}

fn deny_mutation(
    access: &GuildAccess,
    session: &session::Session,
    retry: &str,
) -> Option<Response> {
    match access {
        GuildAccess::Ok(_) => None,
        GuildAccess::Forbidden => Some(forbidden(session)),
        GuildAccess::Unavailable => Some(Redirect::to(retry).into_response()),
    }
}

fn snowflake(s: &str) -> Option<&str> {
    let t = s.trim();
    (!t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())).then_some(t)
}

impl AppState {
    async fn allow_save(&self, user_id: &str) -> bool {
        const WINDOW: Duration = Duration::from_secs(10);
        const MAX: usize = 30;
        let now = Instant::now();
        let mut hits = self.save_hits.lock().await;
        if hits.len() > 1000 {
            hits.retain(|_, v| v.iter().any(|t| now.duration_since(*t) < WINDOW));
        }
        let v = hits.entry(user_id.to_string()).or_default();
        v.retain(|t| now.duration_since(*t) < WINDOW);
        if v.len() >= MAX {
            return false;
        }
        v.push(now);
        true
    }

    async fn bot_guilds(&self) -> Result<Arc<Vec<discord::BotGuild>>, reqwest::Error> {
        const TTL: Duration = Duration::from_secs(300);

        let cached = {
            let guard = self.bot_guilds_detailed.lock().await;
            guard
                .as_ref()
                .filter(|(fetched, _)| fetched.elapsed() < TTL)
                .map(|(_, guilds)| Arc::clone(guilds))
        };
        if let Some(guilds) = cached {
            return Ok(guilds);
        }

        let guilds = Arc::new(discord::bot_guilds_detailed(&self.http, &self.config).await?);
        *self.bot_guilds_detailed.lock().await = Some((Instant::now(), Arc::clone(&guilds)));
        Ok(guilds)
    }

    async fn bot_guild_ids(&self) -> Result<Arc<HashSet<String>>, reqwest::Error> {
        const TTL: Duration = Duration::from_secs(300);

        let cached = {
            let guard = self.bot_guilds.lock().await;
            guard
                .as_ref()
                .map(|(fetched, ids)| (fetched.elapsed(), Arc::clone(ids)))
        };

        if let Some((age, ids)) = cached {
            if age >= TTL {
                self.spawn_bot_refresh();
            }
            return Ok(ids);
        }

        let ids = Arc::new(discord::bot_guild_ids(&self.http, &self.config).await?);
        *self.bot_guilds.lock().await = Some((Instant::now(), Arc::clone(&ids)));
        Ok(ids)
    }

    async fn app_stats(&self) -> Option<discord::AppStats> {
        const TTL: Duration = Duration::from_secs(300);

        let (stats, stale) = {
            let guard = self.app_stats.lock().await;
            match guard.as_ref() {
                Some((fetched, s)) => (Some(*s), fetched.elapsed() >= TTL),
                None => (None, true),
            }
        };

        if stale {
            self.spawn_app_stats_refresh();
        }
        stats
    }

    async fn analytics(&self) -> Option<db::Analytics> {
        const TTL: Duration = Duration::from_secs(60);
        let (value, stale) = {
            let guard = self.analytics.lock().await;
            match guard.as_ref() {
                Some((fetched, a)) => (Some(*a), fetched.elapsed() >= TTL),
                None => (None, true),
            }
        };
        if stale {
            self.spawn_analytics_refresh();
        }
        value
    }

    fn spawn_analytics_refresh(&self) {
        if self.analytics_refreshing.swap(true, Ordering::AcqRel) {
            return;
        }
        let pool = self.pool.clone();
        let cache = Arc::clone(&self.analytics);
        let flag = Arc::clone(&self.analytics_refreshing);
        tokio::spawn(async move {
            match db::analytics(&pool).await {
                Ok(a) => *cache.lock().await = Some((Instant::now(), a)),
                Err(e) => error!("analytics refresh failed: {}", e),
            }
            flag.store(false, Ordering::Release);
        });
    }

    fn spawn_app_stats_refresh(&self) {
        if self.app_stats_refreshing.swap(true, Ordering::AcqRel) {
            return;
        }
        let http = self.http.clone();
        let config = Arc::clone(&self.config);
        let cache = Arc::clone(&self.app_stats);
        let flag = Arc::clone(&self.app_stats_refreshing);
        tokio::spawn(async move {
            match discord::app_stats(&http, &config).await {
                Ok(s) => *cache.lock().await = Some((Instant::now(), s)),
                Err(e) => error!("app stats refresh failed: {}", e),
            }
            flag.store(false, Ordering::Release);
        });
    }

    async fn bot_guild_count(&self) -> Option<usize> {
        self.bot_guilds_detailed
            .lock()
            .await
            .as_ref()
            .map(|(_, guilds)| guilds.len())
    }

    async fn bot_user_count(&self) -> Option<u64> {
        self.bot_guilds_detailed
            .lock()
            .await
            .as_ref()
            .map(|(_, guilds)| guilds.iter().map(|g| g.member_count).sum())
    }

    async fn ensure_bot_guilds_fresh(&self, min_age: Duration) {
        let stale = {
            let guard = self.bot_guilds_detailed.lock().await;
            guard.as_ref().is_none_or(|(t, _)| t.elapsed() >= min_age)
        };
        if stale {
            self.spawn_bot_guilds_refresh();
        }
    }

    fn spawn_bot_guilds_refresh(&self) {
        if self
            .bot_guilds_detailed_refreshing
            .swap(true, Ordering::AcqRel)
        {
            return;
        }
        let http = self.http.clone();
        let config = Arc::clone(&self.config);
        let cache = Arc::clone(&self.bot_guilds_detailed);
        let flag = Arc::clone(&self.bot_guilds_detailed_refreshing);
        tokio::spawn(async move {
            match discord::bot_guilds_detailed(&http, &config).await {
                Ok(guilds) => *cache.lock().await = Some((Instant::now(), Arc::new(guilds))),
                Err(e) => error!("bot guilds refresh failed: {}", e),
            }
            flag.store(false, Ordering::Release);
        });
    }

    async fn refresh_bot_guilds_if_stale(&self, min_age: Duration) {
        let stale = {
            let guard = self.bot_guilds.lock().await;
            guard.as_ref().is_none_or(|(t, _)| t.elapsed() >= min_age)
        };
        if stale {
            self.spawn_bot_refresh();
        }
    }

    fn spawn_bot_refresh(&self) {
        if self.bot_refreshing.swap(true, Ordering::AcqRel) {
            return;
        }
        let http = self.http.clone();
        let config = Arc::clone(&self.config);
        let cache = Arc::clone(&self.bot_guilds);
        let flag = Arc::clone(&self.bot_refreshing);
        tokio::spawn(async move {
            match discord::bot_guild_ids(&http, &config).await {
                Ok(ids) => *cache.lock().await = Some((Instant::now(), Arc::new(ids))),
                Err(e) => error!("bot guild refresh failed: {}", e),
            }
            flag.store(false, Ordering::Release);
        });
    }

    async fn dashboard_guilds(
        &self,
        user_id: &str,
        access_token: &str,
    ) -> Result<Arc<Vec<discord::DashGuild>>, reqwest::Error> {
        const TTL: Duration = Duration::from_secs(30);

        if let Some((fetched, guilds)) = self.user_guilds.lock().await.get(user_id)
            && fetched.elapsed() < TTL
        {
            return Ok(Arc::clone(guilds));
        }

        let bot_ids = self.bot_guild_ids().await?;
        let guilds = Arc::new(discord::dashboard_guilds(&self.http, access_token, &bot_ids).await?);

        let mut cache = self.user_guilds.lock().await;
        if cache.len() > 500 {
            cache.retain(|_, (t, _)| t.elapsed() < TTL);
        }
        cache.insert(user_id.to_string(), (Instant::now(), Arc::clone(&guilds)));
        Ok(guilds)
    }

    async fn pool_settings(&self, guild_id: &str) -> db::Settings {
        db::get_settings(&self.pool, guild_id)
            .await
            .unwrap_or_else(|_| db::Settings::defaults())
    }
}
