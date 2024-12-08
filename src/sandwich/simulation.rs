use anyhow::Result;
use ethers::{
    abi::{Bytes, ParamType},
    types::{
        transaction::eip2930::AccessList, CallConfig, CallFrame, CallLogFrame,
        GethDebugBuiltInTracerConfig, GethDebugBuiltInTracerType, GethDebugTracerConfig,
        GethDebugTracerType, GethDebugTracingCallOptions, GethTrace, GethTraceFrame, H160, H256,
        U256, U64,
    },
};
use ethers_providers::{Middleware, Provider, Ws};
use std::{collections::HashMap, str::FromStr, sync::Arc};

use crate::common::{evm::VictimTx, pools::Pool, utils::return_main_and_target_currency};

use super::streams::{NewBlock, NewPendingTx};
/// DEX 交易信息
#[derive(Debug, Clone)]
pub struct SwapInfo {
    /// 交易的哈希值
    pub tx_hash: H256,
    /// 交易对合约的地址
    pub target_pair: H160,
    /// 主要交易货币的地址(通常是 WETH)
    pub main_currency: H160,
    /// 目标代币的地址
    pub target_token: H160,
    /// DEX 版本(如 UniswapV2=2, V3=3)
    pub version: u8,
    /// token0 是否是主要货币
    pub token0_is_main: bool,
    /// 交易方向(买入/卖出)
    pub direction: SwapDirection,
}
/// 交易方向枚举
#[derive(Debug, Clone)]
pub enum SwapDirection {
    Buy,
    Sell,
}
/// 待处理交易的详细信息
#[derive(Debug, Clone, Default)]
pub struct PendingTxInfo {
    /// 待处理交易的基本信息
    pub pending_tx: NewPendingTx,
    /// 该交易涉及的所有交易对信息
    pub touched_pairs: Vec<SwapInfo>,
}
/// 三明治交易机会的信息
#[derive(Debug, Clone)]
pub struct Sandwich {
    /// 三明治交易需要投入的金额
    pub amount_in: U256,
    /// 目标交易的交易对信息
    pub swap_info: SwapInfo,
    /// 受影响交易(夹在中间的交易)的信息
    pub victim_tx: VictimTx,
    /// 优化后的三明治交易参数,如果已优化则存在
    pub optimized_sandwich: Option<OptimizedSandwich>,
}
/// 优化后的三明治交易详情
#[derive(Debug, Default, Clone)]
pub struct OptimizedSandwich {
    /// 优化后的投入金额
    pub amount_in: U256,
    /// 预期最大收益
    pub max_revenue: U256,
    /// 前置交易预估 gas 用量
    pub front_gas_used: u64,
    /// 后置交易预估 gas 用量
    pub back_gas_used: u64,
    /// 前置交易的访问列表(EIP-2930)
    pub front_access_list: AccessList,
    /// 后置交易的访问列表(EIP-2930)
    pub back_access_list: AccessList,
    /// 前置交易的调用数据
    pub front_calldata: Bytes,
    /// 后置交易的调用数据
    pub back_calldata: Bytes,
}
pub static V2_SWAP_EVENT_ID: &str = "0xd78ad95f";
/// 用于追踪和分析以太坊交易执行过程
pub async fn debug_trace_call(
    provider: &Arc<Provider<Ws>>,
    new_block: &NewBlock,
    pending_tx: &NewPendingTx,
) -> Result<Option<CallFrame>> {
    //////////////////////////////////////////////////////////////
    // 这些配置是告诉 provider(远程节点)你要如何追踪交易//
    // 代码 ---(带着这些配置)---> Provider ---> 远程 Geth 节点
    // 节点执行追踪 <--- 节点按照配置收集信息 <----- 节点收到请求
    // 配置会通过 provider 发送给真正执行追踪的远程节点
    //////////////////////////////////////////////////////////////
    // 配置 Geth 调试追踪功能
    // 这是 Geth 节点的调试追踪选项，用于配置如何追踪交易执行。
    // 它包含了：追踪器类型 追踪深度 是否收集特定信息 执行限制等
    let mut opts = GethDebugTracingCallOptions::default();
    // 这是 CallTracer 的具体配置：
    let mut call_config = CallConfig::default();
    // with_log = true：启用日志收集 会记录合约执行过程中发出的所有事件日志 对于分析 DEX 交易至关重要，因为 Swap 事件都在日志中
    call_config.with_log = Some(true);
    // 设置追踪器
    // Geth 提供多种内置追踪器，这里选择 CallTracer：
    // CallTracer：专注于追踪合约调用
    // 记录调用堆栈、输入数据、返回值等
    // 适合分析复杂的合约交互
    opts.tracing_options.tracer = Some(GethDebugTracerType::BuiltInTracer(
        GethDebugBuiltInTracerType::CallTracer,
    ));
    // 配置追踪器
    // 将之前的 CallConfig 应用到追踪器：
    // 确保追踪器按照配置工作
    // 启用所需的功能（如日志收集）
    opts.tracing_options.tracer_config = Some(GethDebugTracerConfig::BuiltInTracer(
        GethDebugBuiltInTracerConfig::CallTracer(call_config),
    ));
    let block_number = new_block.block_number;
    let mut tx = pending_tx.tx.clone();
    // 获取 nonce 是为了确保在模拟交易时使用正确的 nonce
    // 确保模拟交易时的状态是准确的
    // 避免因 nonce 不匹配导致的模拟失败
    // 让追踪结果更接近实际执行情况
    let nonce = provider
        .get_transaction_count(tx.from, Some(block_number.into()))
        .await
        .unwrap_or_default();
    tx.nonce = nonce;
    // 调用节点的 debug_trace_call 接口模拟执行交易
    // &tx: 要模拟的交易
    // Some(block_number.into()): 在指定区块的状态下模拟
    // opts: 之前配置的追踪选项
    let trace = provider
        .debug_trace_call(&tx, Some(block_number.into()), opts)
        .await;
    // 处理追踪结果
    // 获取调用数据
    match trace {
        Ok(trace) => match trace {
            // typ from value gas gas_used input output error callslogs
            GethTrace::Known(call_tracer) => match call_tracer {
                GethTraceFrame::CallTracer(frame) => Ok(Some(frame)),
                _ => Ok(None),
            },
            _ => Ok(None),
        },
        _ => Ok(None),
    }
}
/// 分析待处理交易中的 swap 操作
/// 提取相关的交易对信息
/// 确定交易的方向(买入/卖出)
pub async fn extract_swap_info(
    provider: &Arc<Provider<Ws>>,
    new_block: &NewBlock,
    pending_tx: &NewPendingTx,
    pools_map: &HashMap<H160, Pool>,
) -> Result<Vec<SwapInfo>> {
    let tx_hash = pending_tx.tx.hash;
    let mut swap_info_vec = Vec::new();
    // 把 pending_tx 在当前区块状态下模拟执行 获取执行结果
    let frame = debug_trace_call(provider, new_block, pending_tx).await?;
    // 没有获取到交易信息 直接返回
    if frame.is_none() {
        return Ok(swap_info_vec);
    }
    let frame: CallFrame = frame.unwrap();
    let mut logs = Vec::new();
    extract_logs(&frame, &mut logs);
    // 识别 Uniswap V2 的 swap 事件
    // 提取相关的交易信息
    // 确定交易方向
    // 收集套利所需的关键信息
    // 主要收集swap info
    for log in &logs {
        match &log.topics {
            Some(topics) => {
                if topics.len() > 1 {
                    // 检查事件签名是否是 Uniswap V2 的 swap 事件
                    let selector = &format!("{:?}", topics[0])[0..10];
                    let is_v2_swap = selector == V2_SWAP_EVENT_ID;
                    if is_v2_swap {
                        let pair_address = log.address.unwrap();
                        // 检查交易对地址是否在我们跟踪的池子列表中
                        let pool = pools_map.get(&pair_address);
                        if pool.is_none() {
                            continue;
                        }
                        let pool = pool.unwrap();
                        let token0 = pool.token0;
                        let token1 = pool.token1;
                        // 获取主要货币 与 目标货币  主要货币是否为token0
                        let (main_currency, target_token, token0_is_main) =
                            match return_main_and_target_currency(token0, token1) {
                                Some(out) => (out.0, out.1, out.0 == token0),
                                None => continue,
                            };
                        let (in0, _, _, out1) = match ethers::abi::decode(
                            &[
                                ParamType::Uint(256),
                                ParamType::Uint(256),
                                ParamType::Uint(256),
                                ParamType::Uint(256),
                            ],
                            // 从 Option 中获取值，如果是 None 则 panic
                            log.data.as_ref().unwrap(),
                        ) {
                            Ok(input) => {
                                let uints: Vec<U256> = input
                                    .into_iter()
                                    // 对迭代器中的每个元素进行转换：
                                    // - to_owned(): 创建一个拥有所有权的副本
                                    // - into_uint(): 转换为无符号整数
                                    // - unwrap(): 获取转换结果
                                    .map(|i| i.to_owned().into_int().unwrap())
                                    .collect();
                                (uints[0], uints[1], uints[2], uints[3])
                            }
                            _ => {
                                let zero = U256::zero();
                                (zero, zero, zero, zero)
                            }
                        };
                        // 判断交易方向 判断用户是在买入还是卖出主要代币
                        let zero_for_one = (in0 > U256::zero()) && (out1 > U256::zero());
                        // token0
                        let direction = if token0_is_main {
                            if zero_for_one {
                                SwapDirection::Buy
                            } else {
                                SwapDirection::Sell
                            }
                        } else {
                            if zero_for_one {
                                SwapDirection::Sell
                            } else {
                                SwapDirection::Buy
                            }
                        };
                        let swap_info = SwapInfo {
                            tx_hash,
                            target_pair: pair_address,
                            main_currency,
                            target_token,
                            version: 2,
                            token0_is_main,
                            direction,
                        };
                        swap_info_vec.push(swap_info);
                    }
                }
            }
            _ => {}
        }
    }
    Ok(swap_info_vec)
}
pub fn extract_logs(call_frame: &CallFrame, logs: &mut Vec<CallLogFrame>) {
    // 如果调用帧中有日志
    if let Some(ref logs_vec) = call_frame.logs {
        // clone() 直接克隆对象
        // cloned() 是迭代器的适配器，用于克隆迭代器中的引用所指向的值
        // 在这里使用 cloned() 是因为 iter() 产生的是引用的迭代器
        // // 将这一层的日志添加到结果中
        logs.extend(logs_vec.iter().cloned());
    }
    // 如果有子调用
    if let Some(ref call_vec) = call_frame.calls {
        for call in call_vec {
            // 递归处理每个子调用
            extract_logs(call, logs);
        }
    }
}
///所有三明治机会Vec
#[derive(Debug, Default, Clone)]
pub struct BatchSandwich {
    pub sandwiches: Vec<Sandwich>,
}
// 模拟整个三明治套利过程
// 1. 记录初始状态（余额、流动性等）
// 2. 执行前置交易（frontrun）
// 3. 执行目标交易（victim tx）
// 4. 执行后置交易（backrun）
// 5. 计算收益和成本
impl BatchSandwich {
    /// 生成唯一标识 "0x12345678-0x98765432-xxxxx"
    pub fn bundle_id(&self) -> String {
        let mut tx_hashes = Vec::new();
        for sandwich in &self.sandwiches {
            let tx_hash = sandwich.victim_tx.tx_hash;
            let tx_hash_4_bytes = &format!("{:?}", tx_hash)[0..10];
            tx_hashes.push(String::from_str(tx_hash_4_bytes).unwrap());
        }
        tx_hashes.sort();
        tx_hashes.dedup();
        tx_hashes.join("-")
    }
    // 获取目标交易哈希
    pub fn victim_tx_hashes(&self) {}
    // 获取目标代币
    pub fn target_tokens(&self) {}
    // 获取目标交易对
    pub fn target_v2_pairs(&self) {}
    // 编码前置交易（买入）
    pub fn encode_frontrun_tx() {}
    // 编码后置交易（卖出）
    pub fn encode_backrun_tx() {}
    // 模拟执行
    pub async fn simulate(
        &self,
        provider: Arc<Provider<Ws>>,
        owner: Option<H160>,
        block_number: U64,
        base_fee: U256,
        max_fee: U256,
        front_access_list: Option<AccessList>,
        back_access_list: Option<AccessList>,
        bot_address: Option<H160>,
    ) {
        let mut simulator = 
    }
}
