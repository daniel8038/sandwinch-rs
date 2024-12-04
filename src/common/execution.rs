use super::abi::Abi;
use super::constants::Env;
use ethers::prelude::*;
use ethers::{middleware::SignerMiddleware, signers::LocalWallet, types::H160};
use ethers_flashbots::*;
use ethers_providers::{Provider, Ws};
use std::str::FromStr;
use std::{collections::HashMap, sync::Arc};
use url::Url;

pub struct Executor {
    pub provider: Arc<Provider<Ws>>,
    pub abi: Abi,
    pub owner: LocalWallet,
    pub identity: LocalWallet,
    pub bot_address: H160,
    pub builder_urls: HashMap<String, Url>,
    pub client: SignerMiddleware<FlashbotsMiddleware<Arc<Provider<Ws>>, LocalWallet>, LocalWallet>,
}
impl Executor {
    pub fn new(provider: Arc<Provider<Ws>>) -> Self {
        let env = Env::new();
        let abi = Abi::new();
        // sandwich机器人合约地址
        let bot_address = H160::from_str(&env.bot_address).unwrap();
        let owner = env
            .private_key
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(1 as u64);
        // identity：
        // Flashbots 的身份钱包
        // 用于与 MEV-Boost 构建者进行身份验证
        // 不用于签名实际交易
        // 建立声誉系统的标识
        let identity = env
            .identity_key
            .parse::<LocalWallet>()
            .unwrap()
            .with_chain_id(1 as u64);
        // relay_url：
        // Flashbots 中继节点的 URL
        // 用于发送 bundle 交易
        // 是与 MEV 相关服务交互的入口点
        let relay_url = Url::parse("https://relay.flashbots.net").unwrap();
        // client：
        // 组合了多个功能的客户端
        // 包含了 Flashbots 功能和交易签名能力
        // 用于发送和管理交易
        let client = SignerMiddleware::new(
            FlashbotsMiddleware::new(provider.clone(), relay_url.clone(), identity.clone()),
            owner.clone(),
        );
        // builder_urls：
        // - flashbots
        // - beaverbuild
        // - rsync
        // - titanbuilder
        // - builder0x69
        // - f1b
        // - lokibuilder
        // - eden
        // - penguinbuild
        // - gambit
        // - idcmev
        let mut builder_urls = HashMap::new();
        builder_urls.insert(
            "flashbots".to_string(),
            Url::parse("https://relay.flashbots.net").unwrap(),
        );
        builder_urls.insert(
            "beaverbuild".to_string(),
            Url::parse("https://rpc.beaverbuild.org").unwrap(),
        );
        builder_urls.insert(
            "rsync".to_string(),
            Url::parse("https://rsync-builder.xyz").unwrap(),
        );
        builder_urls.insert(
            "titanbuilder".to_string(),
            Url::parse("https://rpc.titanbuilder.xyz").unwrap(),
        );
        builder_urls.insert(
            "builder0x69".to_string(),
            Url::parse("https://builder0x69.io").unwrap(),
        );
        builder_urls.insert("f1b".to_string(), Url::parse("https://rpc.f1b.io").unwrap());
        builder_urls.insert(
            "lokibuilder".to_string(),
            Url::parse("https://rpc.lokibuilder.xyz").unwrap(),
        );
        builder_urls.insert(
            "eden".to_string(),
            Url::parse("https://api.edennetwork.io/v1/rpc").unwrap(),
        );
        builder_urls.insert(
            "penguinbuild".to_string(),
            Url::parse("https://rpc.penguinbuild.org").unwrap(),
        );
        builder_urls.insert(
            "gambit".to_string(),
            Url::parse("https://builder.gmbit.co/rpc").unwrap(),
        );
        builder_urls.insert(
            "idcmev".to_string(),
            Url::parse("https://rpc.idcmev.xyz").unwrap(),
        );

        Self {
            provider,
            abi,
            owner,
            identity,
            bot_address,
            builder_urls,
            client,
        }
    }
}
