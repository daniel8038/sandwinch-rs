use std::{collections::HashMap, sync::Arc};

use anyhow::Result;
use ethers::types::{H256, U256};
use ethers_providers::{Provider, Ws};

use crate::common::{
    evm::VictimTx,
    utils::{is_weth, MainCurrency},
};

use super::{
    simulation::{BatchSandwich, PendingTxInfo, Sandwich, SwapDirection},
    streams::NewBlock,
};
// 三明治套利的"预备"阶段
pub async fn appetizer(
    provider: &Arc<Provider<Ws>>,
    new_block: &NewBlock,
    tx_hash: H256,
    victim_gas_price: U256,
    pending_txs: &HashMap<H256, PendingTxInfo>,
    promising_sandwiches: &mut HashMap<H256, Vec<Sandwich>>,
) {
    // pending_txs是被模拟执行过之后 取出来的swap信息
    // 从pending_txs中获取这次tx_hash交易的信息，并创建 victim_tx（目标交易）结构
    // pub struct PendingTxInfo {pub pending_tx: NewPendingTx,pub touched_pairs: Vec<SwapInfo>,}
    let pending_tx_info = pending_txs.get(&tx_hash).unwrap();
    let pending_tx = &pending_tx_info.pending_tx;
    //make sandwiches and simulate
    let victim_tx = VictimTx {
        tx_hash,
        from: pending_tx.tx.from,
        to: pending_tx.tx.to.unwrap_or_default(),
        data: pending_tx.tx.input.0.clone().into(),
        value: pending_tx.tx.value,
        gas_price: victim_gas_price,
        gas_limit: Some(pending_tx.tx.gas.as_u64()),
    };
    let swap_info = &pending_tx_info.touched_pairs;
    /*
    For now, we focus on the buys:
    1. Frontrun: Buy
    2. Victim: Buy
    3. Backrun: Sell
    */
    // 处理这次pending_tx涉及到的所有的swap操做 只处理买操做
    for info in swap_info {
        match info.direction {
            SwapDirection::Sell => continue,
            _ => {}
        }
        let main_currecy = info.main_currency;
        let mc = MainCurrency::new(main_currecy);
        let decimals = mc.decimals();
        let small_amount_in = if is_weth(main_currecy) {
            // 0.01ETH
            U256::from(10).pow(U256::from(decimals - 2))
        } else {
            U256::from(10) * U256::from(10).pow(U256::from(decimals)) // 10 USDT, 10 USDC
        };
        let base_fee = new_block.next_base_fee;
        let max_fee = base_fee;
        // 构建一个三明治信息结构
        let mut sandwich = Sandwich {
            amount_in: small_amount_in,
            swap_info: info.clone(),
            victim_tx: victim_tx.clone(),
            optimized_sandwich: None,
        };
        // 三明治 批 id：0x12345678-0x98765432-xxxxx
        let batch_sandwich = BatchSandwich {
            sandwiches: vec![sandwich.clone()],
        };
        // 三明治机会模拟执行
        let simulated_sandwich = batch_sandwich.
    }
}
