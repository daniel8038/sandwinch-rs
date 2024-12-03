use anyhow::Result;
use csv::StringRecord;
use ethers::{
    abi::{parse_abi, parse_abi_str, ParamType},
    middleware::gas_oracle::cache,
    types::{Filter, H160, H256, U256, U64},
};
use ethers_providers::{Middleware, Provider, Ws};
use indicatif::{ProgressBar, ProgressStyle};
use log::info;
use serde::{Deserialize, Serialize};
use std::{
    collections::HashMap,
    fs::{create_dir_all, OpenOptions},
    path::Path,
    str::FromStr,
    sync::Arc,
};
//Serialize 允许将数据结构转换为特定格式（如 JSON、YAML 等）
//PartialEq 允许使用 == 和 != 运算符比较两个实例
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub enum DexVariant {
    UniswapV2,
}
impl DexVariant {
    pub fn version_num(&self) -> u8 {
        match self {
            DexVariant::UniswapV2 => 2,
        }
    }
}
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
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
impl Pool {
    pub fn cache_row(&self) -> (i64, String, i32, String, String, u32, u64, u64) {
        (
            self.id,
            format!("{:?}", self.address),
            self.version.version_num() as i32,
            format!("{:?}", self.token0),
            format!("{:?}", self.token1),
            self.fee,
            self.block_number,
            self.timestamp,
        )
    }
}
// 接受csv 字符 转换为Pool实例
impl From<StringRecord> for Pool {
    fn from(record: StringRecord) -> Self {
        // 获取版本 现只做V2
        let version = match record.get(2).unwrap().parse().unwrap() {
            2 => DexVariant::UniswapV2,
            _ => DexVariant::UniswapV2,
        };
        Self {
            id: record.get(0).unwrap().parse().unwrap(),
            address: H160::from_str(record.get(1).unwrap()).unwrap(),
            version,
            token0: H160::from_str(record.get(3).unwrap()).unwrap(),
            token1: H160::from_str(record.get(4).unwrap()).unwrap(),
            fee: record.get(5).unwrap().parse().unwrap(),
            block_number: record.get(6).unwrap().parse().unwrap(),
            timestamp: record.get(7).unwrap().parse().unwrap(),
        }
    }
}
// 加载所有的pool
pub async fn load_add_pools(
    wss_url: String,
    from_block: u64,
    chunk: u64,
) -> Result<(Vec<Pool>, i64)> {
    // 创建池子缓存信息目录
    match create_dir_all("cache") {
        _ => {}
    }
    // 文件路径
    let cache_file = "cache/.cached-pools.csv";
    let file_path = Path::new(cache_file);
    // 判断路径是否存在
    let file_exists = file_path.exists();
    let file_handle = OpenOptions::new()
        .write(true)
        .read(true)
        .create(true)
        .open(file_path)
        .unwrap();
    // 接收你的数据
    // 把数据转换成 CSV 格式
    // 然后通过底层的文件句柄（就是之前我们讨论的 file）写入到文件中
    let mut writer = csv::Writer::from_writer(file_handle);
    let mut pools: Vec<Pool> = Vec::new();
    let mut v2_pool_cnt = 0;
    // 如果文件存在直接从csv 读取并格式化
    if file_exists {
        let mut reader = csv::Reader::from_path(file_path)?;
        for row in reader.records() {
            let row = row.unwrap();
            let pool = Pool::from(row);
            match pool.version {
                DexVariant::UniswapV2 => v2_pool_cnt += 1,
            }
            pools.push(pool);
        }
    } else {
        // 写入列
        writer.write_record(&[
            "id",
            "address",
            "version",
            "token0",
            "token1",
            "fee",
            "block_number",
            "timestamp",
        ])?;
    }
    info!("Pools loaded: {:?}", pools.len());
    info!("V2 pools: {:?}", v2_pool_cnt);
    // 如果不存在 wss rpc 获取 并写入csv
    // provider
    let ws = Ws::connect(wss_url).await?;
    let ws_provider = Arc::new(Provider::new(ws));
    // UniswapV2
    // 事件
    let pair_create_event = "PairCreated(address,address,address,unit256)";
    // 事件转abi格式 .event() 是一个方法，它在 ABI 中查找名为 "PairCreated" 的事件
    let abi = parse_abi(&[&format!("event {}", pair_create_event)]).unwrap();
    // 事件签名唯一标识 或者说 topic[0]
    let pair_created_signature = abi.event("PairCreated").unwrap().signature();
    // 从文件存储的最新区块 查找到 rpc 获取的最新区块
    let mut id = if pools.len() > 0 {
        // as_ref() 借用 Option 内部值的引用
        pools.last().as_ref().unwrap().id as i64
    } else {
        -1
    };
    let last_id = id as i64;
    let from_block = if id != -1 {
        pools.last().as_ref().unwrap().block_number
    } else {
        from_block
    };
    let to_block = ws_provider.get_block_number().await.unwrap().as_u64();
    // 进度显示
    let mut blocks_processed = 0;
    let mut block_range = Vec::new();
    loop {
        let start_idx = from_block + blocks_processed;
        let mut end_idx = start_idx + chunk - 1;
        if end_idx > to_block {
            end_idx = to_block;
            block_range.push((start_idx, end_idx));
            break;
        }
        block_range.push((start_idx, end_idx));
    }
    info!("Block range: {:?}", block_range);
    let pb = ProgressBar::new(block_range.len() as u64);
    pb.set_style(
        ProgressStyle::with_template(
            "[{elapsed_precise}] {bar:40.cyan/blue} {pos:>7}/{len:7} {msg}",
        )
        .unwrap()
        .progress_chars("##-"),
    );
    // 开始获取数据 添加进集合
    for range in block_range {
        let mut requests = Vec::new();
        requests.push(tokio::task::spawn(load_uniswap_v2_pool(
            ws_provider.clone(),
            range.0,
            range.1,
            pair_create_event,
            pair_created_signature,
        )));
        let results = futures::future::join_all(requests).await;
        for result in results {
            match result {
                Ok(response) => match response {
                    Ok(pools_response) => {
                        pools.extend(pools_response);
                    }
                    _ => {}
                },
                _ => {}
            }
        }

        pb.inc(1);
    }
    //    遍历pool集合 递增id
    let mut added = 0;
    pools.sort_by_key(|p| p.block_number);
    for pool in pools.iter_mut() {
        if pool.id == -1 {
            id += 1;
            pool.id = id;
        }
        if (pool.id as i64) > last_id {
            // 当你调用 writer.serialize() 时，数据通常会先存在内存缓冲区中
            // // 数据可能还在缓冲区
            writer.serialize(pool.cache_row())?;
            added += 1;
        }
    }
    // writer.flush()? 的作用是强制将缓冲区中的数据写入到文件中
    // 强制写入文件
    writer.flush()?;
    info!("Added {:?} new pools", added);

    Ok((pools, last_id))
}
pub async fn load_uniswap_v2_pool(
    provider: Arc<Provider<Ws>>,
    from_block: u64,
    to_block: u64,
    event: &str,
    signature: H256,
) -> Result<Vec<Pool>> {
    let mut pools = Vec::new();
    let mut timestamp_map = HashMap::new();
    // 创建事件过滤器
    let event_filter = Filter::new()
        .from_block(U64::from(from_block))
        .to_block(U64::from(to_block))
        .event(event);
    // 获取所有的logs
    let logs = provider.get_logs(&event_filter).await?;
    // 解析log
    for log in logs {
        let topic = log.topics[0];
        let block_number = log.block_number.unwrap_or_default();
        if topic != signature {
            continue;
        }
        let timestamp = if !timestamp_map.contains_key(&block_number) {
            let block = provider.get_block(block_number).await.unwrap().unwrap();
            let timestamp = block.timestamp.as_u64();
            timestamp_map.insert(block_number, timestamp);
            timestamp
        } else {
            let timestamp = *timestamp_map.get(&block_number).unwrap();
            timestamp
        };
        let token0 = H160::from(log.topics[1]);
        let token1 = H160::from(log.topics[2]);
        // event PairCreated(address indexed token0, address indexed token1,address pair,uint256 allPairsLength);
        if let Ok(input) =
            // log.data 是以太坊事件日志中的数据字段，包含了事件中所有非索引(non-indexed)参数的编码数据。
            ethers::abi::decode(&[ParamType::Address, ParamType::Uint(256)], &log.data)
        {
            let pair = input[0].to_owned().into_address().unwrap();
            // -1 是一个标记值，表示"这是一个新的、还未持久化到数据库的记录"
            // 当这个 Pool 对象被存储到数据库后，数据库会给它分配一个真实的、正整数的 ID
            let pool_data = Pool {
                id: -1,
                address: pair,
                version: DexVariant::UniswapV2,
                token0,
                token1,
                fee: 300,
                block_number: block_number.as_u64(),
                timestamp,
            };
            pools.push(pool_data);
        };
    }

    Ok(pools)
}
