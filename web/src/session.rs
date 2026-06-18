use crate::discord::User;
use axum_extra::extract::cookie::{Cookie, PrivateCookieJar, SameSite};
use serde::{Deserialize, Serialize};

const SESSION_COOKIE: &str = "genesis_session";
const STATE_COOKIE: &str = "genesis_oauth_state";
const FLASH_COOKIE: &str = "genesis_saved";

#[derive(Serialize, Deserialize, Clone)]
pub struct Session {
    pub user: User,
    pub access_token: String,
}

pub fn read_session(jar: &PrivateCookieJar) -> Option<Session> {
    let raw = jar.get(SESSION_COOKIE)?;
    serde_json::from_str(raw.value()).ok()
}

pub fn write_session(jar: PrivateCookieJar, session: &Session, secure: bool) -> PrivateCookieJar {
    let value = serde_json::to_string(session).unwrap_or_default();
    let mut cookie = Cookie::new(SESSION_COOKIE, value);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(secure);
    cookie.set_path("/");
    jar.add(cookie)
}

pub fn clear_session(jar: PrivateCookieJar) -> PrivateCookieJar {
    jar.remove(Cookie::from(SESSION_COOKIE))
}

pub fn write_state(jar: PrivateCookieJar, state: &str, secure: bool) -> PrivateCookieJar {
    let mut cookie = Cookie::new(STATE_COOKIE, state.to_string());
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(secure);
    cookie.set_path("/");
    jar.add(cookie)
}

pub fn read_state(jar: &PrivateCookieJar) -> Option<String> {
    jar.get(STATE_COOKIE).map(|c| c.value().to_string())
}

pub fn clear_state(jar: PrivateCookieJar) -> PrivateCookieJar {
    jar.remove(Cookie::from(STATE_COOKIE))
}

pub fn set_saved(jar: PrivateCookieJar, secure: bool) -> PrivateCookieJar {
    let mut cookie = Cookie::new(FLASH_COOKIE, "1");
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Lax);
    cookie.set_secure(secure);
    cookie.set_path("/");
    jar.add(cookie)
}

pub fn take_saved(jar: PrivateCookieJar) -> (bool, PrivateCookieJar) {
    if jar.get(FLASH_COOKIE).is_some() {
        let mut removal = Cookie::from(FLASH_COOKIE);
        removal.set_path("/");
        (true, jar.remove(removal))
    } else {
        (false, jar)
    }
}
