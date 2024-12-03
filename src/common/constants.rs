#[derive(Debug, Clone)]
pub struct Env {
    pub https_url: String,
    pub wss_url: String,
    pub bot_address: String,
    pub private_key: String,
    pub identity_key: String,
    pub telegram_token: String,
    pub telegram_chat_id: String,
    pub use_alert: bool,
    pub debug: bool,
}
fn get_env(key: &str) -> String {
    std::env::var(key).unwrap()
}
impl Env {
    pub fn new() -> Self {
        Env {
            https_url: get_env("HTTPS_URL"),
            wss_url: get_env("WSS_URL"),
            bot_address: get_env("BOT_ADDRESS"),
            private_key: get_env("PRIVATE_KEY"),
            identity_key: get_env("IDENTITY_KEY"),
            telegram_token: get_env("TELEGRAM_TOKEN"),
            telegram_chat_id: get_env("TELEGRAM_CHAT_ID"),
            use_alert: get_env("USE_ALERT").parse::<bool>().unwrap(),
            debug: get_env("DEBUG").parse::<bool>().unwrap(),
        }
    }
}
