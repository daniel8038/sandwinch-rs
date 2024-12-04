use std::sync::Arc;

use crate::common::{constants::Env, pools::load_all_pools};
use ethers_providers::{Middleware, Provider, Ws};
use log::info;
use tokio::sync::broadcast::Sender;

use super::streams::Event;
// 获取所有的池子->
pub async fn run_sandwich_strategy(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let env = Env::new();
    let mut event_receiver = event_sender.subscribe();
    // 获取所有的池子
    let (pools_vec, prev_pool_id) = load_all_pools(env.wss_url.clone(), 10000000, 50000)
        .await
        .unwrap();
    // 获取所有token
    let block_number = provider.get_block_number().await.unwrap();
    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                Event::Block(block) => {
                    info!("{:?}", block);
                }
                Event::PendingTx(mut pending_tx) => {
                    info!("{:?}", pending_tx);
                }
            },
            _ => {}
        }
    }
}
