use std::sync::Arc;

use anyhow::Result;
use dotenv::dotenv;
use ethers_providers::{Provider, Ws};
use log::info;
use sandwinch_rs::{
    common::{constants::Env, utils::setup_logger},
    sandwich::streams::{stream_new_block, stream_pending_transactions, Event},
};
use tokio::sync::broadcast::Sender;
use tokio::{sync::broadcast, task::JoinSet};
#[tokio::main]
async fn main() -> Result<()> {
    dotenv().ok();
    setup_logger()?;
    let env = Env::new();
    let ws = Ws::connect(env.wss_url.clone()).await?;
    let ws_provider = Arc::new(Provider::new(ws));
    // 多线程跑 pendingtx & new block
    let (event_sender, _): (Sender<Event>, _) = broadcast::channel(512);
    let mut set = JoinSet::new();
    set.spawn(stream_new_block(ws_provider.clone(), event_sender.clone()));
    set.spawn(stream_pending_transactions(
        ws_provider.clone(),
        event_sender.clone(),
    ));
    while let Some(res) = set.join_next().await {
        info!("{:?}", res);
    }
    Ok(())
}
