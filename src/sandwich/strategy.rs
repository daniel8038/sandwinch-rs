use std::{collections::HashMap, sync::Arc};

use crate::{
    common::{
        alert::Alert,
        constants::Env,
        pools::{load_all_pools, Pool},
        tokens::load_all_tokens,
        utils::calculate_next_block_base_fee,
    },
    sandwich::streams::NewBlock,
};
use ethers::types::{BlockNumber, H160};
use ethers_providers::{Middleware, Provider, Ws};
use log::info;
use tokio::sync::broadcast::Sender;

use super::streams::Event;
// 获取所有的池子->
pub async fn run_sandwich_strategy(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let env = Env::new();
    let mut event_receiver = event_sender.subscribe();
    // 获取所有的池子
    let (pools, prev_pool_id) = load_all_pools(env.wss_url.clone(), 10000000, 50000)
        .await
        .unwrap();
    // 根据最新区块获取池子所有token信息 并进行表绑定
    let block_number = provider.get_block_number().await.unwrap();
    let tokens_map = load_all_tokens(&provider, block_number, &pools, prev_pool_id)
        .await
        .unwrap();
    info!("Tokens map count: {:?}", tokens_map.len());
    // 过滤掉没有存储token信息的池子
    let pools_vec: Vec<Pool> = pools
        .into_iter()
        .filter(|p| {
            let token0_exists = tokens_map.contains_key(&p.token0);
            let token1_exists = tokens_map.contains_key(&p.token1);
            token0_exists && token1_exists
        })
        .collect();
    info!("Filtered pools by tokens count: {:?}", pools_vec.len());
    // 创建pools_map
    let pools_map: HashMap<H160, Pool> = pools_vec
        .clone()
        .into_iter()
        .map(|p| (p.address, p))
        .collect();
    // 获取最新的区块信息
    let block = provider
        .get_block(BlockNumber::Latest)
        .await
        .unwrap()
        .unwrap();
    let mut new_block = NewBlock {
        block_number: block.number.unwrap(),
        base_fee: block.base_fee_per_gas.unwrap(),
        next_base_fee: calculate_next_block_base_fee(
            block.gas_used,
            block.gas_limit,
            block.base_fee_per_gas.unwrap(),
        ),
    };
    // 创建Tg Alert
    let alert = Alert::new();

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
