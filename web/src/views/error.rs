use super::layout::layout;
use crate::discord::User;
use maud::{Markup, html};

pub fn error_page(message: &str, user: Option<&User>) -> Markup {
    let body = html! {
        h1 { "something went wrong" }
        p.muted { (message) }
        p { a.btn href="/" { "back home" } }
    };
    layout("error", "", user, body)
}
