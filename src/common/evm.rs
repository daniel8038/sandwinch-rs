use ethers::types::*;
/// 记录目标交易(被夹在三明治交易中间的交易)的详细信息
/// 受害者hash
#[derive(Debug, Clone, Default)]
pub struct VictimTx {
    /// 交易的哈希值
    pub tx_hash: H256,
    /// 交易发送者的地址
    pub from: H160,
    /// 交易接收者的地址(通常是 DEX 合约)
    pub to: H160,
    /// 交易的调用数据(包含函数选择器和参数)
    pub data: Bytes,
    /// 交易附带的 ETH 数量
    pub value: U256,
    /// 交易的 gas 价格
    pub gas_price: U256,
    /// 交易的 gas 上限,某些情况下可能没有设置
    pub gas_limit: Option<u64>,
}
