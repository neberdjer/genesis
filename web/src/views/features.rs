use super::layout::{ico, layout};
use crate::catalog::FEATURE_GROUPS;
use crate::discord::User;
use maud::{Markup, html};

pub fn features(user: Option<&User>) -> Markup {
    let body = html! {
        div.page-intro {
            h1 { "features" }
            p.muted { "paste a supported link and genesis replaces the broken preview with one that actually works." }
        }
        @for group in FEATURE_GROUPS {
            section.group {
                h2.group-title { (group.title) }
                div.feature-list {
                    @for f in group.features {
                        div.feature {
                            span.feature-ico { (ico(f.icon)) }
                            div.feature-text {
                                h3 { (f.title) }
                                p { (f.desc) }
                            }
                        }
                    }
                }
            }
        }
    };
    layout("features", "features", user, body)
}
