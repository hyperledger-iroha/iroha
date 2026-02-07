---
lang: zh-hans
direction: ltr
source: docs/portal/docs/sorafs/reports/orchestrator-ga-parity.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a206b033b430fc9895f64d402cd53bfea35c3f269b2c18bb12a1f929114423aa
source_last_modified: "2025-12-29T18:16:35.200252+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# SoraFS Orchestrator GA 奇偶校验报告

现在每个 SDK 都会跟踪确定性多重获取奇偶校验，因此发布工程师可以确认
有效负载字节、块收据、提供商报告和记分板结果保持一致
实施。每个线束都会消耗规范的多提供商捆绑包
`fixtures/sorafs_orchestrator/multi_peer_parity_v1/`，包含 SF1 计划、提供商
元数据、遥测快照和协调器选项。

## Rust 基线

- **命令：** `cargo test -p sorafs_orchestrator --test orchestrator_parity -- --nocapture`
- **范围：** 通过进程内编排器运行 `MultiPeerFixture` 计划两次，验证
  组装有效负载字节、块收据、提供商报告和记分板结果。仪器仪表
  还跟踪峰值并发和有效工作集大小 (`max_parallel × max_chunk_length`)。
- **性能防护：** 每次运行必须在 CI 硬件上在 2 秒内完成。
- **工作设置上限：** 通过 SF1 型材，线束强制执行 `max_parallel = 3`，产生
  ≤196608字节窗口。

日志输出示例：

```
Rust orchestrator parity: duration_ms=142.63 total_bytes=1048576 max_inflight=3 peak_reserved_bytes=196608
```

## JavaScript SDK 工具

- **命令：** `npm run build:native && node --test javascript/iroha_js/test/sorafsOrchestrator.parity.test.js`
- **范围：** 通过 `iroha_js_host::sorafsMultiFetchLocal` 重播相同的装置，比较有效负载，
  连续运行的收据、提供商报告和记分板快照。
- **性能守卫：**每次执行必须在2秒内完成；线束打印测量值
  持续时间和保留字节上限（`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`）。

摘要行示例：

```
JS orchestrator parity: duration_ms=187.42 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Swift SDK 线束

- **命令：** `swift test --package-path IrohaSwift --filter SorafsOrchestratorParityTests/testLocalFetchParityIsDeterministic`
- **范围：** 运行 `IrohaSwift/Tests/IrohaSwiftTests/SorafsOrchestratorParityTests.swift` 中定义的奇偶校验套件，
  通过 Norito 桥 (`sorafsLocalFetch`) 重播 SF1 赛程两次。线束验证
  有效负载字节、块收据、提供商报告和记分板条目使用相同的确定性
  作为 Rust/JS 套件的提供者元数据和遥测快照。
- **Bridge bootstrap：** 线束按需解包 `dist/NoritoBridge.xcframework.zip` 并加载
  通过 `dlopen` 的 macOS 切片。当 xcframework 缺失或缺少 SoraFS 绑定时，它
  回退到 `cargo build -p connect_norito_bridge --release` 并链接到
  `target/release/libconnect_norito_bridge.dylib`，所以CI中不需要手动设置。
- **性能守卫：** CI 硬件上每次执行必须在 2 秒内完成；线束打印
  测量的持续时间和保留字节上限（`max_parallel = 3`、`peak_reserved_bytes ≤ 196 608`）。

摘要行示例：

```
Swift orchestrator parity: duration_ms=183.54 total_bytes=1048576 max_parallel=3 peak_reserved_bytes=196608
```

## Python 绑定线束

- **命令：** `python -m pytest python/iroha_python/tests/test_sorafs_orchestrator.py -k multi_fetch_fixture_round_trip`
- **范围：** 练习高级 `iroha_python.sorafs.multi_fetch_local` 包装器及其类型
  数据类，因此规范装置流经轮消费者调用的相同 API。测试
  从 `providers.json` 重建提供者元数据，注入遥测快照并验证
  有效负载字节、块收据、提供商报告和记分板内容，就像 Rust/JS/Swift 一样
  套房。
- **先决条件：** 运行 `maturin develop --release` （或安装轮子），以便 `_crypto` 公开
  `sorafs_multi_fetch_local` 调用 pytest 之前绑定；绑定时线束会自动跳过
  不可用。
- **性能保障：** 与 Rust 套件相同 ≤2s 预算； pytest 记录组装的字节数
  以及发布工件的提供商参与摘要。

发布门控应该捕获每个工具（Rust、Python、JS、Swift）的摘要输出，以便
归档报告可以在推广构建之前统一区分有效负载收据和指标。运行
`ci/sdk_sorafs_orchestrator.sh` 执行每个奇偶校验套件（Rust、Python 绑定、JS、Swift）
一趟； CI 工件应附加该助手的日志摘录以及生成的
`matrix.md`（SDK/状态/持续时间表）到发布票证，以便审核者可以审核奇偶校验
矩阵而无需在本地重新运行套件。