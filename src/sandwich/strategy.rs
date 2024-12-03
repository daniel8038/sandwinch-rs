use std::sync::Arc;

use crate::common::constants::Env;
use ethers_providers::{Provider, Ws};
use tokio::sync::broadcast::Sender;

use super::streams::Event;

pub async fn run_sandwich_strategy(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let env = Env::new();
    let mut event_receiver = event_sender.subscribe();
    // 获取所有的池子
}
