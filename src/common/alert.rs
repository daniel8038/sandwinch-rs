use crate::common::constants::Env;
use anyhow::Result;
use ethers::types::{H256, U64};
use teloxide::prelude::*;
use teloxide::types::ChatId;
pub struct Alert {
    pub bot: Option<Bot>,
    pub chat_id: Option<ChatId>,
}
impl Alert {
    pub fn new() -> Self {
        let env = Env::new();
        if env.use_alert {
            let bot = Bot::from_env();
            let chat_id = ChatId(env.telegram_chat_id.parse::<i64>().unwrap());
            Self {
                bot: Some(bot),
                chat_id: Some(chat_id),
            }
        } else {
            Self {
                bot: None,
                chat_id: None,
            }
        }
    }
}
