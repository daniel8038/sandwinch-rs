////////////////////////////////////////
////区块与pendingTx事件监听///
////////////////////////////////////////

use crate::common::utils::calculate_next_block_base_fee;
use ethers::types::*;
use ethers_providers::{Middleware, Provider, Ws};
use std::sync::Arc;
use tokio::sync::broadcast::Sender;
use tokio_stream::StreamExt;
/// 新区块的信息
#[derive(Default, Debug, Clone)]
pub struct NewBlock {
    /// 区块号
    pub block_number: U64,
    /// 当前区块的基础 gas 费用
    pub base_fee: U256,
    /// 预估的下一个区块基础 gas 费用
    pub next_base_fee: U256,
}
/// 待处理交易的信息
#[derive(Default, Debug, Clone)]
pub struct NewPendingTx {
    /// 交易被添加时的区块号。None 表示刚收到还未分配区块号
    pub added_block: Option<U64>,
    /// 交易的完整信息,包含 gas、nonce、数据等
    pub tx: Transaction,
}
/// 事件枚举,用于在不同组件间传递区块和交易信息
#[derive(Debug, Clone)]
pub enum Event {
    /// 新区块事件,包含区块信息
    Block(NewBlock),
    /// 新的待处理交易事件
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
