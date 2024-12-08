# 获取新区快事件

区块信息格式：

```rs
pub struct NewBlock {
    /// 区块号
    pub block_number: U64,
    /// 当前区块的基础 gas 费用
    pub base_fee: U256,
    /// 预估的下一个区块基础 gas 费用
    pub next_base_fee: U256,
}
```

# 获取 pending tx

pending_tx 信息格式：

```rs
pub struct NewPendingTx {
    /// 交易被添加时的区块号。None 表示刚收到还未分配区块号
    pub added_block: Option<U64>,
    /// 交易的完整信息,包含 gas、nonce、数据等
    pub tx: Transaction,
}
```

# 加载所有 pool

根据 chunk 分辟处理，写入 csv 格式文件

```rs
// UniswapV2
    // 事件
    let pair_create_event = "PairCreated(address,address,address,unit256)";
    // 事件转abi格式 .event() 是一个方法，它在 ABI 中查找名为 "PairCreated" 的事件
    let abi = parse_abi(&[&format!("event {}", pair_create_event)]).unwrap();
    // 事件签名唯一标识 或者说 topic[0]
    let pair_created_signature = abi.event("PairCreated").unwrap().signature();
```

**get_logs**

```rs
// 创建事件过滤器
    let event_filter = Filter::new()
        .from_block(U64::from(from_block))
        .to_block(U64::from(to_block))
        .event(event);
    // 获取所有的logs
    let logs = provider.get_logs(&event_filter).await?;
```

根据 log topic 和 事件 signature
解析出 Pool 数据

```rs
pub struct Pool {
    // 地址 版本类型 token0 token1 fee 区块号 时间戳
    pub id: i64,
    pub address: H160,
    pub version: DexVariant,
    pub token0: H160,
    pub token1: H160,
    pub fee: u32,
    pub block_number: u64,
    pub timestamp: u64,
}
```

# 加载所有 pool 相关 token

从解析出的所有的 pool 池子的 相关数据中挨个处理每个 token 避免重复处理 token`!tokens_map.contains_key(&token)`

get_token_info 使用 spoof state 模拟区块链状态 本地模拟调用合约 本质是使用底层的 eth_call 如果有自己的本地节点 会很快

# 区块事件处理

获取区块内所有的 txs
如果 txs 存在 pendingTx 则此 tx 已经执行完毕 也代表不存在三明治机会
pendingTxs 集合 只保存 3 个个最近区块 旧区块无意义
确保 promising_sandwiches 中的交易都存在于 pending_txs 中 每次新区区块都要判定一次

# pendingTx 处理

拿到 tx_hash

1. 如果 pending_txs Hashmap 已经存在这个 key 达标已经接收，判断是否有交易回执 如果有代表链上已经执行成功 就没有必要处理这个交易 不是三明治机会 直接删除
2. 判断受害者的 gas，根据交易类型 不同类型不同的 gas 处理方式 最后判断时候会被区块打包

3. 如果判定是一个三明治机会，解析出 swap 信息

   debug_trace_call 本地模拟 pending_tx 拿到执行后的信息

   ```rs
   let trace = provider
           .debug_trace_call(&tx, Some(block_number.into()), opts)
           .await;
   ```

   如果模拟执行成功 会得到 CallFrame 整个的交易信息

   ```rs
   pub struct CallFrame {
       pub typ: String,
       pub from: Address,
       pub to: Option<NameOrAddress>,
       pub value: Option<U256>,
       pub gas: U256,
       pub gas_used: U256,
       pub input: Bytes,
       pub output: Option<Bytes>,
       pub error: Option<String>,
       pub calls: Option<Vec<CallFrame>>,
       pub logs: Option<Vec<CallLogFrame>>,
   }
   ```

   解析出所有的调用 log 根据 topics 进行分离出 swap 信息 不处理无关的事件 只处理 swap 相关的

   ```rs
   // 检查事件签名是否是 Uniswap V2 的 swap 事件
   let selector = &format!("{:?}", topics[0])[0..10];
   let is_v2_swap = selector == V2_SWAP_EVENT_ID;
   ```

   根据 log 中的 swap 事件的数据判断交易方向

   得到 SwapInfo

   ```rs
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
   ```

4. 记录下所涉及到的池子
   ```rs
   pending_tx.added_block = Some(new_block.block_number);
   let pending_tx_info = PendingTxInfo {
        pending_tx: pending_tx.clone(),
        touched_pairs: swap_info.clone(),
   };
    pending_txs.insert(tx_hash, pending_tx_info.clone());
   ```
