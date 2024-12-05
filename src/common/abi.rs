use ethers::abi::parse_abi;
use ethers::prelude::BaseContract;
/// 智能合约 ABI 集合,包含了与 DEX 和机器人交互所需的合约接口
#[derive(Clone, Debug)]
pub struct Abi {
    /// UniswapV2/V3 工厂合约的 ABI,用于创建和查询交易对
    pub factory: BaseContract,
    /// 交易对合约的 ABI,用于查询价格、执行交易等操作
    pub pair: BaseContract,
    /// ERC20 代币合约的 ABI,用于查询余额、授权等操作
    pub token: BaseContract,
    /// 三明治机器人合约的 ABI,用于执行套利交易
    pub sando_bot: BaseContract,
}

impl Abi {
    pub fn new() -> Self {
        let factory = BaseContract::from(
            parse_abi(&["function getPair(address,address) external view returns (address)"])
                .unwrap(),
        );

        let pair = BaseContract::from(
            parse_abi(&[
                "function token0() external view returns (address)",
                "function token1() external view returns (address)",
                "function getReserves() external view returns (uint112,uint112,uint32)",
            ])
            .unwrap(),
        );

        let token = BaseContract::from(
            parse_abi(&[
                "function owner() external view returns (address)",
                "function name() external view returns (string)",
                "function symbol() external view returns (string)",
                "function decimals() external view returns (uint8)",
                "function totalSupply() external view returns (uint256)",
                "function balanceOf(address) external view returns (uint256)",
                "function approve(address,uint256) external view returns (bool)",
                "function transfer(address,uint256) external returns (bool)",
                "function allowance(address,address) external view returns (uint256)",
            ])
            .unwrap(),
        );

        let sando_bot = BaseContract::from(
            parse_abi(&["function recoverToken(address,uint256) public"]).unwrap(),
        );

        Self {
            factory,
            pair,
            token,
            sando_bot,
        }
    }
}
