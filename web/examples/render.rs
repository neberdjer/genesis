use genesis_web::config::Config;
fn main() {
    for v in [
        "",
        "abc123",
        "discord.gg/abc123",
        "discord.com/invite/abc123",
        "https://discord.gg/abc123",
        "/abc123",
    ] {
        unsafe {
            std::env::set_var("SUPPORT_INVITE", v);
        }
        let c = Config::from_env();
        println!("{:<32} -> {:?}", format!("{:?}", v), c.support_invite_url());
    }
}
