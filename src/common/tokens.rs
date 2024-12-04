use std::{collections::HashMap, fs::OpenOptions, path::Path, str::FromStr, sync::Arc};

use super::{pools::Pool, utils::create_new_wallet};
use crate::common::bytecoode::REQUEST_BYTECODE;
use anyhow::Result;
use csv::StringRecord;
use ethers::{
    abi::parse_abi,
    types::{BlockNumber, TransactionRequest, H160, U256, U64},
};
use ethers_contract::BaseContract;
use ethers_providers::RawCall;
use ethers_providers::{spoof, Provider, Ws};
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
#[derive(Debug, Clone)]
pub struct Token {
    pub id: i64,
    pub address: H160,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
    pub pool_ids: Vec<i64>,
}
impl From<StringRecord> for Token {
    fn from(record: StringRecord) -> Self {
        Self {
            id: record.get(0).unwrap().parse().unwrap(),
            address: H160::from_str(record.get(1).unwrap()).unwrap(),
            name: String::from(record.get(2).unwrap()),
            symbol: String::from(record.get(3).unwrap()),
            decimals: record.get(4).unwrap().parse().unwrap(),
            pool_ids: Vec::new(),
        }
    }
}
impl Token {
    pub fn cache_row(&self) -> (i64, String, String, String, u8) {
        (
            self.id,
            format!("{:?}", self.address),
            self.name.clone(),
            self.symbol.clone(),
            self.decimals,
        )
    }
}
#[derive(Debug, Clone)]
pub struct TokenInfo {
    pub address: H160,
    pub name: String,
    pub symbol: String,
    pub decimals: u8,
}

pub async fn load_all_tokens(
    provider: &Arc<Provider<Ws>>,
    block_number: U64,
    pools: &Vec<Pool>,
    prev_pool_id: i64,
) -> Result<HashMap<H160, Token>> {
    // 与加载所有的池子 同样的步骤
    // 设置缓存文件路径
    let cache_file = "cache/.cached-tokens.csv";
    let file_path = Path::new(cache_file);
    let file_exists = file_path.exists();
    // 打开文件，设置写入和追加权限
    let file = OpenOptions::new()
        .write(true)
        .append(true)
        .create(true)
        .open(file_path)
        .unwrap();
    // 创建 CSV writer 用于写入数据
    let mut writer = csv::Writer::from_writer(file);
    // 创建代币地址到代币信息的映射
    let mut tokens_map: HashMap<H160, Token> = HashMap::new();
    let mut token_id = 0;
    // 如果缓存文件存在，读取已有的代币数据
    if file_exists {
        let mut reader = csv::Reader::from_path(file_path)?;

        for row in reader.records() {
            let row = row.unwrap();
            let token = Token::from(row);
            tokens_map.insert(token.address, token);
            token_id += 1;
        }
    } else {
        // 如果是新文件，写入CSV表头
        writer.write_record(&["id", "address", "name", "symbol", "decimals"])?;
    }
    // 创建进度条
    let pb = ProgressBar::new(pools.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );
    // 记录当前token_id，用于后续判断新增代币
    let new_token_id = token_id;
    // 遍历所有池子，获取代币信息
    for pool in pools {
        let pool_id = pool.id;
        // 跳过太老的池子（优化性能）
        if pool_id < prev_pool_id - 50 {
            continue;
        }

        let token0 = pool.token0;
        let token1 = pool.token1;
        // 处理池子中的两个代币
        for token in vec![token0, token1] {
            // 如果代币还未收录，获取其信息
            if !tokens_map.contains_key(&token) {
                match get_token_info(provider, block_number.into(), token).await {
                    Ok(token_info) => {
                        tokens_map.insert(
                            token,
                            Token {
                                id: token_id,
                                address: token,
                                name: token_info.name,
                                symbol: token_info.symbol,
                                decimals: token_info.decimals,
                                pool_ids: Vec::new(),
                            },
                        );
                        token_id += 1;
                    }
                    Err(_) => {}
                }
            }
        }
        // 更新进度条
        pb.inc(1);
    }
    // 更新代币与池子的关联关系
    for pool in pools {
        let pool_id = pool.id;

        let token0 = pool.token0;
        let token1 = pool.token1;
        for token in vec![token0, token1] {
            if let Some(token_map) = tokens_map.get_mut(&token) {
                token_map.pool_ids.push(pool_id);
            }
        }
    }
    info!("Token count: {:?}", tokens_map.len());
    // 保存新增的代币信息到缓存文件
    let mut added = 0;
    let mut tokens_vec: Vec<&Token> = tokens_map.values().collect();
    tokens_vec.sort_by_key(|t| t.id);
    for token in tokens_vec.iter() {
        if token.id >= new_token_id {
            writer.serialize(token.cache_row())?;
            added += 1;
        }
    }
    // 确保数据写入磁盘
    writer.flush()?;
    info!("Added {:?} new tokens", added);

    Ok(tokens_map)
}
pub async fn get_token_info(
    provider: &Arc<Provider<Ws>>,
    block_number: BlockNumber,
    token_address: H160,
) -> Result<TokenInfo> {
    // 区块链模拟
    // 创建一个新钱包作为交易发起者
    let owner = create_new_wallet().1;
    // 创建模拟的区块链状态
    // state 的作用范围和特点:
    // state 创建的是一个独立的虚拟环境
    // 可以读取真实区块链的状态
    // 可以覆盖部分状态进行模拟
    // 执行结果不会影响实际链上状态
    let mut state = spoof::state();
    // owner账号 用于调用
    state.account(owner).balance(U256::MAX).nonce(0.into());
    let request_address = create_new_wallet().1;
    // 等于是合约地址 通过模拟state 不需要真正部署合约再去调用合约  直接可以通过字节码模拟一个合约账户 再使用模拟状态进行调用
    state
        .account(request_address)
        .code((*REQUEST_BYTECODE).clone());
    //BaseContract 是 ethers-rs 中的一个核心类型，用于处理与智能合约交互的底层操作
    // BaseContract 更像是一个工具，用来处理 ABI 编解码
    // 创建 BaseContract，只包含 ABI 定义
    // 主要用来：
    // 解码返回数据（将返回的字节码转换为我们需要的类型）
    // 编码函数调用数据（将函数名和参数转换为字节码）
    let request_abi = BaseContract::from(parse_abi(&[
        "function getTokenInfo(address) external returns (string,string,uint8,uint256)",
    ])?);
    // 编码调用数据calldata
    let calldata = request_abi.encode("getTokenInfo", token_address)?;
    // wei
    let gas_price = U256::from(1000)
        .checked_mul(U256::from(10).pow(U256::from(9)))
        .unwrap();
    // 创建交易请求参数
    let tx = TransactionRequest::default()
        .from(owner)
        .to(request_address)
        .value(U256::zero())
        .data(calldata.0)
        .nonce(U256::zero())
        .gas(5000000)
        .gas_price(gas_price)
        .chain_id(1)
        .into();
    // eth_call底层调用也是 TransactionRequest格式 只不过不是一笔真正的交易不会改变state
    // 真正的交易是：eth_sendTransaction 这个调用指令
    let result = provider
        .call_raw(&tx)
        .state(&state) // 在模拟状态中执行
        //指定在哪个区块执行调用
        // 可以查询历史状态
        // 如果不指定，默认是最新区块
        .block(block_number.into())
        .await?;
    let out: (String, String, u8, U256) = request_abi.decode_output("getTokenInfo", result)?;
    let token_info = TokenInfo {
        address: token_address,
        name: out.0,
        symbol: out.1,
        decimals: out.2,
    };
    Ok(token_info)
}
/*
spoof:
   Provides types and methods for constructing an eth_call state override set
## 什么是状态覆盖？
    状态覆盖允许我们在不实际修改区块链的情况下，临时修改以下内容：

    账户余额
    合约代码
    存储内容
    Nonce值
    状态变量

    这就像是创建了一个"假的"或"模拟的"区块链环境。
## 使用spoof的主要好处是：

    不需要实际交易
    可以快速测试各种场景
    完全可控的测试环境
    可重复的测试结果

    但需要注意：

    spoof只是模拟，不会改变实际链上状态
    某些复杂的交互可能难以完全模拟
    需要了解合约的存储布局才能正确设置状态
*/
