让我创建一个 CallFrame 的示例数据结构，包含主调用和子调用：

```rust
CallFrame {
    // 主调用信息
    call_type: "CALL",
    from: 0x742d35Cc6634C0532925a3b844Bc454e4438f44e,  // 调用者地址
    to: 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D,    // Uniswap Router 地址
    input: 0x38ed1739...,  // swap 函数的调用数据
    output: 0x....,        // 返回数据
    gas_used: 150000,

    // 事件日志
    logs: Some(vec![
        CallLogFrame {
            address: 0x...,  // 产生日志的合约地址
            topics: vec![    // 日志主题
                0x... // Transfer 事件签名
            ],
            data: 0x...     // 日志数据
        }
    ]),

    // 子调用列表
    calls: Some(vec![
        CallFrame {
            // 第一个子调用 - 可能是 token 授权
            call_type: "CALL",
            from: 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D,
            to: 0xC02aaA39b223FE8D0A0e5C4F27eAD9083C756Cc2,  // WETH 合约
            input: 0x...,
            output: 0x...,
            gas_used: 30000,
            logs: Some(vec![
                CallLogFrame {
                    // Approval 事件日志
                }
            ]),
            calls: None  // 这个子调用没有更深的调用
        },
        CallFrame {
            // 第二个子调用 - 可能是实际的 swap
            call_type: "CALL",
            from: 0x7a250d5630B4cF539739dF2C5dAcb4c659F2488D,
            to: 0x...,  // 交易对合约地址
            input: 0x...,  // swap 调用数据
            output: 0x...,
            gas_used: 90000,
            logs: Some(vec![
                CallLogFrame {
                    // Swap 事件日志
                },
                CallLogFrame {
                    // Transfer 事件日志
                }
            ]),
            calls: Some(vec![
                CallFrame {
                    // 更深层的子调用 - 可能是 token 转账
                }
            ])
        }
    ])
}
```

这个结构展示了：

1. 顶层调用

   - 调用 Uniswap Router 进行交换
   - 包含基本的调用信息

2. 子调用层级

   - token 授权调用
   - 实际的 swap 调用
   - 更深层的 token 转账

3. 事件日志
   - Transfer 事件
   - Approval 事件
   - Swap 事件

当 `extract_logs` 处理这个结构时，它会：

1. 收集顶层的日志
2. 递归进入第一个子调用，收集其日志
3. 递归进入第二个子调用，收集其日志
4. 最终得到完整的日志列表

# 通过例子展示 `extract_logs` 处理前后的数据变化：

处理前的 `CallFrame` 结构：

```rust
CallFrame {
    // Uniswap Router 调用
    logs: Some(vec![
        CallLogFrame {
            address: 0x123...,
            topics: vec!["RouterLog"],
            data: "RouterData"
        }
    ]),
    calls: Some(vec![
        CallFrame {
            // WETH 合约调用
            logs: Some(vec![
                CallLogFrame {
                    address: 0x456...,
                    topics: vec!["WETHLog"],
                    data: "WETHData"
                }
            ]),
            calls: None
        },
        CallFrame {
            // Pair 合约调用
            logs: Some(vec![
                CallLogFrame {
                    address: 0x789...,
                    topics: vec!["SwapLog"],
                    data: "SwapData"
                }
            ]),
            calls: Some(vec![
                CallFrame {
                    // Token 转账调用
                    logs: Some(vec![
                        CallLogFrame {
                            address: 0xabc...,
                            topics: vec!["TransferLog"],
                            data: "TransferData"
                        }
                    ]),
                    calls: None
                }
            ])
        }
    ])
}
```

处理后的 `logs` 向量：

```rust
Vec<CallLogFrame> [
    // 从顶层收集的日志
    CallLogFrame {
        address: 0x123...,
        topics: vec!["RouterLog"],
        data: "RouterData"
    },

    // 从第一个子调用收集的日志
    CallLogFrame {
        address: 0x456...,
        topics: vec!["WETHLog"],
        data: "WETHData"
    },

    // 从第二个子调用收集的日志
    CallLogFrame {
        address: 0x789...,
        topics: vec!["SwapLog"],
        data: "SwapData"
    },

    // 从最深层子调用收集的日志
    CallLogFrame {
        address: 0xabc...,
        topics: vec!["TransferLog"],
        data: "TransferData"
    }
]
```

变化说明：

1. 原始数据是树状结构，日志分散在各层调用中
2. 处理后变成一个扁平的数组，包含所有日志
3. 日志的顺序是按照调用层级依次收集的

这样处理的好处：

- 方便后续统一处理所有日志
- 保留了日志的顺序
- 不会遗漏任何层级的日志
