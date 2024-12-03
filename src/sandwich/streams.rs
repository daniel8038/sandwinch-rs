use std::sync::Arc;

use crate::common::utils::calculate_next_block_base_fee;
use ethers::types::*;
use ethers_providers::{Middleware, Provider, Ws};
use tokio::sync::broadcast::Sender;
use tokio_stream::StreamExt;
#[derive(Default, Debug, Clone)]
pub struct NewBlock {
    pub block_number: U64,
    pub base_fee: U256,
    pub next_base_fee: U256,
}
#[derive(Default, Debug, Clone)]
pub struct NewPendingTx {
    pub added_block: Option<U64>,
    pub tx: Transaction,
}

#[derive(Debug, Clone)]
pub enum Event {
    Block(NewBlock),
    PendingTx(NewPendingTx),
}
// 新区快
pub async fn stream_new_block(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let stream = provider.subscribe_blocks().await.unwrap();
    // 格式化
    let mut stream = stream.filter_map(|block| match block.number {
        Some(number) => Some(NewBlock {
            block_number: number,
            base_fee: block.base_fee_per_gas.unwrap(),
            next_base_fee: calculate_next_block_base_fee(
                block.gas_used,
                block.gas_limit,
                block.base_fee_per_gas.unwrap_or_default(),
            ),
        }),
        None => None,
    });
    // 发送事件
    while let Some(block) = stream.next().await {
        match event_sender.send(Event::Block(block)) {
            Ok(_) => {}
            Err(_) => {}
        }
    }
}
// pending交易
pub async fn stream_pending_transactions(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    let stream = provider.subscribe_pending_txs().await.unwrap();
    // transactions_unordered(256)
    // 将交易哈希流转换为实际的交易数据流
    // 256 是并发限制，意味着最多同时处理 256 个交易
    // "unordered" 表示交易的处理结果可能不按原始顺序返回
    // 这是性能优化的一种方式，允许并行处理多个交易
    // fuse()
    // 将流转换为 "熔断" 状态
    // 一旦流返回 None，后续所有的 poll 操作都会返回 None
    // 这防止了流在结束后被重新激活
    let mut stream = stream.transactions_unordered(256).fuse();
    while let Some(result) = stream.next().await {
        match result {
            Ok(tx) => match event_sender.send(Event::PendingTx(NewPendingTx {
                added_block: None,
                tx,
            })) {
                Ok(_) => {}
                Err(_) => {}
            },
            Err(_) => {}
        }
    }
}
