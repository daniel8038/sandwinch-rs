use std::str::FromStr;

use crate::common::constants::*;
use anyhow::{Ok, Result};
use ethers::{
    signers::{LocalWallet, Signer},
    types::{H160, U256},
};
use fern::colors::{Color, ColoredLevelConfig};
use rand::{thread_rng, Rng};
pub fn setup_logger() -> Result<()> {
    let colors = ColoredLevelConfig {
        trace: Color::Cyan,
        debug: Color::Magenta,
        info: Color::Green,
        warn: Color::Red,
        error: Color::BrightRed,
        ..ColoredLevelConfig::new()
    };
    fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{} {}] {}",
                chrono::Local::now(),
                colors.color(record.level()),
                message
            ))
        })
        .chain(std::io::stdout())
        .level(log::LevelFilter::Error)
        .level_for("My_sandwich", log::LevelFilter::Info)
        .apply()?;
    Ok(())
}
pub fn calculate_next_block_base_fee(
    gas_used: U256,
    gas_limit: U256,
    base_fee_per_gas: U256,
) -> U256 {
    let gas_used = gas_used;

    let mut target_gas_used = gas_limit / 2;
    target_gas_used = if target_gas_used == U256::zero() {
        U256::one()
    } else {
        target_gas_used
    };

    let new_base_fee = {
        if gas_used > target_gas_used {
            base_fee_per_gas
                + ((base_fee_per_gas * (gas_used - target_gas_used)) / target_gas_used)
                    / U256::from(8u64)
        } else {
            base_fee_per_gas
                - ((base_fee_per_gas * (target_gas_used - gas_used)) / target_gas_used)
                    / U256::from(8u64)
        }
    };

    let seed = rand::thread_rng().gen_range(0..9);
    new_base_fee + seed
}
pub fn create_new_wallet() -> (LocalWallet, H160) {
    let wallet = LocalWallet::new(&mut thread_rng());
    let address = wallet.address();
    (wallet, address)
}
pub fn to_h160(str_address: &'static str) -> H160 {
    H160::from_str(str_address).unwrap()
}
pub fn is_main_currency(token_address: H160) -> bool {
    let main_currencies = vec![to_h160(WETH), to_h160(USDT), to_h160(USDC)];
    main_currencies.contains(&token_address)
}
/// 函数接收两个代币地址，返回主要代币和目标代币的地址对
pub fn return_main_and_target_currency(token0: H160, token1: H160) -> Option<(H160, H160)> {
    let token0_supported = is_main_currency(token0);
    let token1_supported = is_main_currency(token1);
    if !token0_supported && !token1_supported {
        return None;
    }
    // 如果两个代币都是主要代币（如 WETH/USDC 交易对）
    if token0_supported && token1_supported {
        // 创建主要代币对象以获取权重
        let mc0 = MainCurrency::new(token0);
        let mc1 = MainCurrency::new(token1);
        // 获取两个代币的权重
        let token0_weight = mc0.weight();
        let token1_weight = mc1.weight();
        // 返回权重更高的代币作为主要代币
        if token0_weight > token1_weight {
            return Some((token0, token1));
        } else {
            return Some((token1, token0));
        }
    }
    // 如果只有一个是主要代币
    if token0_supported {
        // 如果 token0 是主要代币，返回 (token0, token1)
        return Some((token0, token1));
    } else {
        // 如果 token1 是主要代币，返回 (token1, token0)
        return Some((token1, token0));
    }
}
#[derive(Debug, Clone)]
pub enum MainCurrency {
    WETH,
    USDT,
    USDC,
    Default, // Pairs that aren't WETH/Stable pairs. Default to WETH for now
}

impl MainCurrency {
    pub fn new(address: H160) -> Self {
        if address == to_h160(WETH) {
            MainCurrency::WETH
        } else if address == to_h160(USDT) {
            MainCurrency::USDT
        } else if address == to_h160(USDC) {
            MainCurrency::USDC
        } else {
            MainCurrency::Default
        }
    }
    /*
    We score the currencies by importance
    WETH has the highest importance, and USDT, USDC in the following order
    */
    pub fn weight(&self) -> u8 {
        match self {
            MainCurrency::WETH => 3,
            MainCurrency::USDT => 2,
            MainCurrency::USDC => 1,
            MainCurrency::Default => 3, // default is WETH
        }
    }
}
