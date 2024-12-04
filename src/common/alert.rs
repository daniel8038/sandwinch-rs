use crate::common::constants::Env;
use anyhow::Result;
use ethers::types::{H256, U64};
use teloxide::prelude::*;
use teloxide::types::ChatId;
pub struct Alert {
    pub bot: Option<Bot>,        // Telegram机器人实例
    pub chat_id: Option<ChatId>, // Telegram聊天ID
}
// telegram 警告
impl Alert {
    pub fn new() -> Self {
        let env = Env::new();
        // 如果启用了警报功能
        if env.use_alert {
            let bot = Bot::from_env(); // 从环境变量创建机器人
            let chat_id = ChatId(env.telegram_chat_id.parse::<i64>().unwrap());
            Self {
                bot: Some(bot),
                chat_id: Some(chat_id),
            }
        } else {
            // 未启用警报时返回空实例
            Self {
                bot: None,
                chat_id: None,
            }
        }
    }
    // 发送消息
    pub async fn send(&self, message: &str) -> Result<()> {
        match &self.bot {
            Some(bot) => {
                bot.send_message(self.chat_id.unwrap(), message).await?;
            }
            _ => {}
        }
        Ok(())
    }
    // 发送bundle交易 信息警告 区块 交易hash gambit_hash
    pub async fn send_bundle_sent(
        &self,
        block_number: U64,
        tx_hash: H256,
        gambit_hash: H256,
    ) -> Result<()> {
        // 拼接message 发送给Tg
        // Eigenphi 是一个 MEV (矿工可提取价值) 分析平台
        // 可以查看交易的 MEV 相关信息
        // 包括套利、清算等详细分析
        let eigenphi_url = format!("https://eigenphi.io/mev/eigentx/{:?}", tx_hash);
        // Gambit 是一个 MEV 拍卖平台
        // 可以查看你的 bundle 交易信息
        // 包括 bundle 状态、执行情况等
        let gambit_url = format!("https://gmbit-co.vercel.app/auction?txHash={:?}", tx_hash);
        let mut message = format!("[Block #{:?}] Bundle sent: {:?}", block_number, tx_hash);
        message = format!("{}\n-Eigenphi: {}", message, eigenphi_url);
        message = format!("{}\n-Gambit: {}", message, gambit_url);
        message = format!("{}\n-Gambit bundle hash: {:?}", message, gambit_hash);
        self.send(&message).await?;
        Ok(())
    }
}
