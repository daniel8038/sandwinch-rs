use std::{collections::HashMap, str::FromStr, sync::Arc};

use crate::{
    common::{
        alert::Alert,
        constants::Env,
        execution::Executor,
        pools::{load_all_pools, Pool},
        tokens::load_all_tokens,
        utils::calculate_next_block_base_fee,
    },
    sandwich::{
        simulation::{PendingTxInfo, Sandwich},
        streams::NewBlock,
    },
};
use bounded_vec_deque::BoundedVecDeque;
use ethers::{
    signers::{LocalWallet, Signer},
    types::{BlockNumber, H160, H256, U256, U64},
};
use ethers_core::k256::elliptic_curve::rand_core::block;
use ethers_providers::{Middleware, Provider, Ws};
use futures::executor;
use log::info;
use tokio::sync::broadcast::Sender;

use super::streams::Event;
// 获取所有的池子->
pub async fn run_sandwich_strategy(provider: Arc<Provider<Ws>>, event_sender: Sender<Event>) {
    /////////////////////////
    ////////基础配置/////////
    ////////////////////////
    let env = Env::new();

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
    // 创建执行器实例flash bot
    let executor = Executor::new(provider);
    // 三明治机器人合约地址
    let bot_address = H160::from_str(&env.bot_address).unwrap();
    //    钱包signer
    let wallet = env
        .private_key
        .parse::<LocalWallet>()
        .unwrap()
        .with_chain_id(1 as u64);
    // owner地址
    let owner = wallet.address();
    /////////////////////////
    ///////////交易//////////
    ////////////////////////
    let mut event_receiver = event_sender.subscribe();
    // 为什么需要这个 HashMap:
    // 三明治交易需要追踪和分析待处理的交易来寻找套利机会
    // 需要快速查找和更新交易状态(HashMap 提供 O(1) 的查找效率)
    // 用于避免重复处理相同的交易(通过 already_received 检查)
    // 要注意Hashmap的清理  避免太占用资源
    let mut pending_txs: HashMap<H256, PendingTxInfo> = HashMap::new();
    // 创建一个新的 HashMap 用于存储潜在的三明治交易机会 用于记录和跟踪可能的套利机会
    let mut promising_sandwiches: HashMap<H256, Vec<Sandwich>> = HashMap::new();
    let mut simulated_bundle_ids = BoundedVecDeque::new(30);
    //    接受线程消息 执行策略
    loop {
        match event_receiver.recv().await {
            Ok(event) => match event {
                // 接受新的区块生成打包消息 如果区块中的交易txs 在pending_txs中存在 那么代表交易已经完成  直接清除
                Event::Block(block) => {
                    // 更新最新区块信息
                    new_block = block;
                    let block_with_txs = provider
                        .get_block_with_txs(new_block.block_number)
                        .await
                        .unwrap()
                        .unwrap();
                    let txs = block_with_txs
                        .transactions
                        .into_iter()
                        .map(|tx| tx.hash)
                        .collect();
                    // 检查pendingtx
                    for tx_hash in &txs {
                        if pending_txs.contains_key(tx_hash) {
                            let removed = pending_txs.remove(tx_hash).unwrap();
                            promising_sandwiches.remove(tx_hash);
                            // info!(
                            //     "⚪️ V{:?} TX REMOVED: {:?} / Pending txs: {:?}",
                            //     removed.touched_pairs.get(0).unwrap().version,
                            //     tx_hash,
                            //     pending_txs.len()
                            // );
                        }
                    }
                    //    只保留3个区块的pending tx
                    pending_txs.retain(|_, v| {
                        new_block.block_number - v.pending_tx.added_block.unwrap() < U64::from(3)
                    });
                    promising_sandwiches.retain(|h, _| pending_txs.contains_key(h));
                }
                Event::PendingTx(mut pending_tx) => {
                    let tx_hash = pending_tx.tx.hash;
                    // 检查是否已经处理过这笔交易
                    let already_received = pending_txs.contains_key(&tx_hash);
                    let mut should_add = false;
                    // 如果是已经接受的pending_tx 检查是否有交易回执 如果有交易回执 证明交易已经被处理
                    if !already_received {
                        let tx_receipt = provider.get_transaction_receipt(tx_hash).await;
                        match tx_receipt {
                            Ok(receipt) => match receipt {
                                Some(_) => {
                                    pending_txs.remove(&tx_hash);
                                }
                                None => {
                                    should_add = true;
                                }
                            },
                            _ => {}
                        }
                    }
                    /////////////////////////////////////////
                    /////////未被处理的交易///////
                    /////////////////////////////////////////
                    // 用于记录受害者的gas费用
                    let mut victim_gas_price = U256::zero();
                    // 思路：gas判断交易思路.md
                    // 这里比较重要： 主要作用就是 判断交易类型 根据gas 确定用户交易 能不能在这个区块内打包成功
                    match pending_tx.tx.transaction_type {
                        Some(tx_type) => {
                            if tx_type == U64::zero() {
                                victim_gas_price = pending_tx.tx.gas_price.unwrap_or_default();
                                should_add = victim_gas_price >= new_block.base_fee;
                            } else if tx_type == U64::from(2) {
                                victim_gas_price =
                                    pending_tx.tx.max_fee_per_gas.unwrap_or_default();
                                should_add = victim_gas_price >= new_block.base_fee;
                            }
                        }
                        _ => {}
                    }
                    // 如果应该添加进三明治
                }
            },
            _ => {}
        }
    }
}
