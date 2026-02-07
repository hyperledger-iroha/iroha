---
lang: zh-hans
direction: ltr
source: CHANGELOG.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 26f5115a14476de15fbc8f26c5a9807954df6884763a818b2bc98ec6cfe1a4cc
source_last_modified: "2026-01-05T09:28:11.640562+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 变更日志

[Unreleased]: https://github.com/hyperledger-iroha/iroha/compare/v2.0.0-rc.2.0...HEAD
[2.0.0-rc.2.0]: https://github.com/hyperledger-iroha/iroha/releases/tag/v2.0.0-rc.2.0

该项目的所有显着更改都将记录在该文件中。

## [未发布]

- 放下 SCALE 垫片； `norito::codec` 现在通过本机 Norito 序列化来实现。
- 将跨 crate 的 `parity_scale_codec` 用法替换为 `norito::codec`。
- 开始将工具迁移到本机 Norito 序列化。
- 从工作区中删除剩余的 `parity-scale-codec` 依赖项，以支持本机 Norito 序列化。
- 用本机 Norito 实现替换残留的 SCALE 特征派生，并重命名版本化编解码器模块。
- 使用功能门控宏将 `iroha_config_base_derive` 和 `iroha_futures_derive` 合并为 `iroha_derive`。
- *(multisig)* 使用稳定的错误代码/原因拒绝来自多重签名机构的直接签名，跨嵌套中继器强制执行多重签名 TTL 上限，并在提交之前在 CLI 中显示 TTL 上限（SDK 奇偶校验待定）。
- 将 FFI 程序宏移至 `iroha_ffi` 并删除 `iroha_ffi_derive` 箱。
- *(schema_g​​en)* 从 `iroha_data_model` 依赖项中删除不必要的 `transparent_api` 功能。
- *(data_model)* 缓存用于 `Name` 解析的 ICU NFC 标准化器，以减少重复的初始化开销。
- 📚 Torii 客户端的文档 JS 快速入门、配置解析器、发布工作流程和配置感知配方。
- *(IrohaSwift)* 将最低部署目标提高到 iOS 15 / macOS 12，跨 Torii 客户端 API 采用 Swift 并发，并将公共模型标记为 `Sendable`。
- *(IrohaSwift)* 添加了 `ToriiDaProofSummaryArtifact` 和 `DaProofSummaryArtifactEmitter.emit`，以便 Swift 应用程序可以构建/发出与 CLI 兼容的 DA 证明包，而无需向 CLI 进行 shell 操作，并包含涵盖内存中和磁盘上的文档和回归测试【F:IrohaSwift/Sources/IrohaSwift/ToriiDaProofSummaryArtifact.swift:1】【F:IrohaSwift/Tests/IrohaSwiftTests/ToriiDaProofSummaryArtifactTests.swift:1】【F:docs/source/sdk/swift/index.md:260】
- *(data_model/js_host)* 通过从 `KaigiParticipantCommitment` 中删除存档重用标志来修复 Kaigi 选项序列化，添加本机往返测试，并删除 JS 解码回退，以便 Kaigi 现在指示 Norito 往返之前提交。【F:crates/iroha_data_model/src/kaigi.rs:128】【F:crates/iroha_js_host/src/lib.rs:1379】【F:javascript/iroha_js/test/instructionBuilders.test.js:30】
- *(javascript)* 允许 `ToriiClient` 调用者删除默认标头（通过传递 `null`），以便 `getMetrics` 在 JSON 和 Prometheus 文本之间干净地切换 接受headers.【F:javascript/iroha_js/src/toriiClient.js:488】【F:javascript/iroha_js/src/toriiClient.js:761】
- *(javascript)* 为 NFT、每个账户资产余额和资产定义持有者（带有 TypeScript defs、文档和测试）添加了可迭代帮助程序，因此 Torii 分页现在涵盖了剩余的应用程序端点。【F:javascript/iroha_js/src/toriiClient.js:105】【F:javascript/iroha_js/index.d.ts:80】【F:javascript/iroha_js/test/toriiClient.test.js:365】【F:javascript/iroha_js/README.md:470】
- *(javascript)* 添加了治理指令/交易构建器以及治理配方，以便 JS 客户端可以端到端地部署提案、投票、颁布和理事会持久性。【F:javascript/iroha_js/src/instructionBuilders.js:1012】【F:javascript/iroha_js/src/transaction.js:1082】【F:javascript/iroha_js/recipes/governance.mjs:1】
- *(javascript)* 添加了 ISO 20022 pacs.008 提交/状态帮助程序和匹配的配方，让 JS 调用者无需定制 HTTP 即可行使 Torii ISO 桥管道。【F:javascript/iroha_js/src/toriiClient.js:888】【F:javascript/iroha_js/index.d.ts:706】【F:javascript/iroha_js/recipes/iso_bridge.mjs:1】
- *(javascript)* 添加了 pacs.008/pacs.009 构建器帮助程序以及配置驱动的配方，以便 JS 调用者可以在点击之前用经过验证的 BIC/IBAN 元数据合成 ISO 20022 有效负载桥.【F:javascript/iroha_js/src/isoBridge.js:1】【F:javascript/iroha_js/test/isoBridge.test.js:1】【F:javascript/iroha_js/recipes/iso_bridge_builder.mjs:1】【F:javascript/iroha_js/index.d.ts:1】
- *(javascript)* 完成了 DA 摄取/获取/证明循环：`ToriiClient.fetchDaPayloadViaGateway` 现在自动派生分块器句柄（通过新的 `deriveDaChunkerHandle` 绑定），可选证明摘要重用本机 `generateDaProofSummary`，并且刷新了 README/打字/测试，以便 SDK 调用者可以镜像`iroha da get-blob/prove-availability` 无定制管道.【F:javascript/iroha_js/src/toriiClient.js:1123】【F:javascript/iroha_js/src/dataAvailability.js:1】【F:javascript/iroha_js/test/toriiClient.test.js:1454】【F:javascript/iroha_js/index.d.ts:3275】【F:javascript/iroha_js/README.md:760】
- *(javascript/js_host)* `sorafsGatewayFetch` 记分板元数据现在在使用网关提供商时记录网关清单 id/CID，以便采用工件与 CLI 捕获保持一致。【F:crates/iroha_js_host/src/lib.rs:3017】【F:docs/source/sorafs_orchestrator_rollout.md:23】
- *(torii/cli)* 强制 ISO 人行横道：Torii 现在拒绝具有未知代理 BIC 的 `pacs.008` 提交，并且 DvP CLI 预览通过以下方式验证 `--delivery-instrument-id` `--iso-reference-crosswalk`.【F:crates/iroha_torii/src/iso20022_bridge.rs:704】【F:crates/iroha_cli/src/main.rs:3892】
- *(torii)* 通过 `POST /v1/iso20022/pacs009` 添加 PvP 现金摄取，在构建转账之前强制执行 `Purp=SECU` 和 BIC 参考数据检查。【F:crates/iroha_torii/src/iso20022_bridge.rs:1070】【F:crates/iroha_torii/src/lib.rs:4759】
- *（工具）* 添加了 `cargo xtask iso-bridge-lint`（加上 `ci/check_iso_reference_data.sh`）以验证 ISIN/CUSIP、BIC↔LEI 和 MIC 快照以及存储库固定装置。【F:xtask/src/main.rs:146】【F:ci/check_iso_reference_data.sh:1】
- *(javascript)* 通过声明存储库元数据、显式文件白名单、启用来源的 `publishConfig`、`prepublishOnly` 变更日志/测试防护以及在以下位置执行 Node 18/20 的 GitHub Actions 工作流程来强化 npm 发布CI【F:javascript/iroha_js/package.json:1】【F:javascript/iroha_js/scripts/check-changelog.mjs:1】【F:docs/source/sdk/js/publishing.md:1】【F:.github/workflows/javascript-sdk.yml:1】
- *(ivm/cuda)* BN254 字段 add/sub/mul 现在通过 `bn254_launch_kernel` 在新的 CUDA 内核上执行，并通过主机端批处理，为 Poseidon 和 ZK 小工具启用硬件加速，同时保持确定性回退。【F:crates/ivm/cuda/bn254.cu:1】【F:crates/ivm/src/cuda.rs:66】【F:crates/ivm/src/cuda.rs:1244】

## [2.0.0-rc.2.0] - 2025-05-08

### 🚀 特点

- *(cli)* 添加 `iroha transaction get` 和其他重要命令 (#5289)
- [**破坏**]分离可替代和不可替代资产（#5308）
- [**break**] 通过允许后面有空块来完成非空块 (#5320)
- 在架构和客户端中公开遥测类型 (#5387)
- *(iroha_torii)* 用于功能门控端点的存根 (#5385)
- 添加提交时间指标 (#5380)

### 🐛 错误修复

- 修改 NonZeros (#5278)
- 文档文件中的拼写错误 (#5309)
- *（加密）* 暴露 `Signature::payload` getter (#5302) (#5310)
- *（核心）* 在授予角色之前添加角色存在检查（#5300）
- *（核心）* 重新连接断开的对等点 (#5325)
- 修复与商店资产和 NFT 相关的 pytests (#5341)
- *(CI)* 修复诗歌 v2 的 python 静态分析工作流程 (#5374)
- 提交后出现过期事务事件 (#5396)

### 💼 其他

- 包括 `rust-toolchain.toml` (#5376)
- 对 `unused` 发出警告，而不是 `deny` (#5377)

### 🚜 重构

- 伞 Iroha CLI (#5282)
- *(iroha_test_network)* 使用漂亮的日志格式 (#5331)
- [**突破**] 简化 `genesis.json` 中 `NumericSpec` 的序列化 (#5340)
- 改进 p2p 连接失败的日志记录 (#5379)
- 恢复 `logger.level`，添加 `logger.filter`，扩展配置路由 (#5384)

### 📚 文档

- 将 `network.public_address` 添加到 `peer.template.toml` (#5321)

### ⚡ 性能

- *(kura)* 防止冗余块写入磁盘 (#5373)
- 为交易哈希实现自定义存储（#5405）

### ⚙️ 杂项任务

- 修复诗歌的使用（#5285）
- 从 `iroha_torii_const` 中删除冗余常量 (#5322)
- 删除未使用的 `AssetEvent::Metadata*` (#5339)
- 碰撞 Sonarqube 动作版本 (#5337)
- 删除未使用的权限 (#5346)
- 将解压包添加到 ci-image (#5347)
- 修复一些评论 (#5397)
- 将集成测试从 `iroha` 箱中移出 (#5393)
- 禁用defectdojo工作（#5406）
- 为缺失的提交添加 DCO 签核
- 重新组织工作流程（第二次尝试）（#5399）
- 不要在推送到主程序时运行 Pull Request CI (#5415)

<!-- generated by git-cliff -->

## [2.0.0-rc.1.3] - 2025-03-07

### 添加

- 通过在其后允许空块来最终确定非空块（#5320）

## [2.0.0-rc.1.2] - 2025-02-25

### 已修复

- 重新注册的对等点现在可以正确反映在对等点列表中（#5327）

## [2.0.0-rc.1.1] - 2025-02-12

### 添加

- 添加 `iroha transaction get` 和其他重要命令 (#5289)

## [2.0.0-rc.1.0] - 2024-12-06

### 添加- 实现查询预测（#5242）
- 使用持久执行器 (#5082)
- 向 iroha cli 添加监听超时 (#5241)
- 将 /peers API 端点添加到 torii (#5235)
- 地址不可知的 p2p (#5176)
- 提高多重签名实用性和可用性 (#5027)
- 保护 `BasicAuth::password` 不被打印 (#5195)
- 在 `FindTransactions` 查询中降序排序 (#5190)
- 将块头引入每个智能合约执行上下文中（#5151）
- 基于视图更改索引的动态提交时间 (#4957)
- 定义默认权限集 (#5075)
- 添加 `Option<Box<R>>` 的 Niche 实现 (#5094)
- 交易和块谓词 (#5025)
- 报告查询中剩余项目的数量（#5016）
- 有界离散时间 (#4928)
- 将缺失的数学运算添加到 `Numeric` (#4976)
- 验证块同步消息（#4965）
- 查询过滤器（#4833）

### 已更改

- 简化对等 ID 解析 (#5228)
- 将交易错误移出块有效负载（#5118）
- 将 JsonString 重命名为 Json (#5154)
- 将客户端实体添加到智能合约中（#5073）
- 作为交易排序服务的领导者（#4967）
- 让 kura 从内存中删除旧块 (#5103)
- 使用 `ConstVec` 来获取 `Executable` 中的指令 (#5096)
- 最多发送一次八卦 (#5079)
- 减少 `CommittedTransaction` 的内存使用量 (#5089)
- 使查询游标错误更加具体（#5086）
- 重组板条箱（#4970）
- 引入 `FindTriggers` 查询，删除 `FindTriggerById` (#5040)
- 不依赖签名进行更新 (#5039)
- 更改 genesis.json 中的参数格式 (#5020)
- 只发送当前和之前的视图更改证明 (#4929)
- 未准备好时禁用发送消息以防止繁忙循环 (#5032)
- 将总资产数量移至资产定义（#5029）
- 仅签署块的标头，而不签署整个有效负载 (#5000)
- 使用 `HashOf<BlockHeader>` 作为块哈希的类型 (#4998)
- 简化 `/health` 和 `/api_version` (#4960)
- 将 `configs` 重命名为 `defaults`，删除 `swarm` (#4862)

### 已修复

- 扁平化 json 中的内部角色 (#5198)
- 修复 `cargo audit` 警告 (#5183)
- 添加范围检查到签名索引（#5157）
- 修复文档中的模型宏示例 (#5149)
- 在块/事件流中正确关闭 ws (#5101)
- 损坏的可信对等点检查 (#5121)
- 检查下一个块的高度是否+1 (#5111)
- 修复创世块的时间戳（#5098）
- 修复没有 `transparent_api` 功能的 `iroha_genesis` 编译 (#5056)
- 正确处理 `replace_top_block` (#4870)
- 修复执行器克隆 (#4955)
- 显示更多错误详细信息 (#4973)
- 对块流使用 `GET` (#4990)
- 改进队列事务处理（#4947）
- 防止冗余的块同步块消息（#4909）
- 防止同时发送大消息时出现死锁 (#4948)
- 从缓存中删除过期的交易 (#4922)
- 修复牌坊 url 的路径 (#4903)

### 已删除

- 从客户端删除基于模块的 api (#5184)
- 删除 `riffle_iter` (#5181)
- 删除未使用的依赖项 (#5173)
- 从 `blocks_in_memory` 中删除 `max` 前缀 (#5145)
- 删除共识估计（#5116）
- 从块中删除 `event_recommendations` (#4932)

### 安全

## [2.0.0-pre-rc.22.1] - 2024-07-30

### 已修复

- 将 `jq` 添加到 docker 镜像

## [2.0.0-pre-rc.22.0] - 2024-07-25

### 添加

- 在创世中明确指定链上参数 (#4812)
- 允许涡轮鱼具有多个 `Instruction` (#4805)
- 重新实现多重签名交易（#4788）
- 实现内置与自定义链上参数（#4731）
- 改进自定义指令的使用（#4778）
- 通过实现 JsonString 使元数据动态化 (#4732)
- 允许多个节点提交创世块（#4775）
- 向对等方提供 `SignedBlock` 而不是 `SignedTransaction` (#4739)
- 执行器中的自定义指令 (#4645)
- 扩展客户端 cli 来请求 json 查询 (#4684)
 - 添加对 `norito_decoder` 的检测支持 (#4680)
- 将权限模式推广到执行者数据模型 (#4658)
- 在默认执行器中添加了注册触发器权限 (#4616)
 - 支持 `norito_cli` 中的 JSON
- 引入p2p空闲超时

### 已更改

- 将 `lol_alloc` 替换为 `dlmalloc` (#4857)
- 在架构中将 `type_` 重命名为 `type` (#4855)
- 将架构中的 `Duration` 替换为 `u64` (#4841)
- 使用类似 `RUST_LOG` 的 EnvFilter 进行日志记录 (#4837)
- 尽可能保留投票块 (#4828)
- 从经线迁移到阿克苏姆（#4718）
- 分割执行器数据模型（#4791）
- 浅层数据模型 (#4734) (#4792)
- 不要发送带签名的公钥 (#4518)
- 将 `--outfile` 重命名为 `--out-file` (#4679)
- 重命名 iroha 服务器和客户端 (#4662)
- 将 `PermissionToken` 重命名为 `Permission` (#4635)
- 急切地拒绝 `BlockMessages` (#4606)
- 使 `SignedBlock` 不可变 (#4620)
- 将 TransactionValue 重命名为 CommiedTransaction (#4610)
- 通过 ID 验证个人帐户 (#4411)
- 对私钥使用多重哈希格式 (#4541)
 - 将 `parity_scale_decoder` 重命名为 `norito_cli`
- 将区块发送到 Set B 验证器
- 使 `Role` 透明 (#4886)
- 从标头导出块哈希 (#4890)

### 已修复

- 检查权限是否拥有要转移的域 (#4807)
- 删除记录器双重初始化（#4800）
- 修复资产和权限的命名约定 (#4741)
- 在创世块中的单独交易中升级执行器（#4757）
- `JsonString` 的正确默认值 (#4692)
- 改进反序列化错误消息（#4659）
- 如果传递的 Ed25519Sha512 公钥长度无效，请不要惊慌 (#4650)
- 在初始化块加载上使用正确的视图更改索引（#4612）
- 不要在 `start` 时间戳之前过早执行时间触发器 (#4333)
- 支持 `https` 为 `torii_url` (#4601) (#4617)
- 从 SetKeyValue/RemoveKeyValue 中删除 serde(flatten) (#4547)
- 触发器集已正确序列化
- 撤销在 `Upgrade<Executor>` 上删除 `PermissionToken` (#4503)
- 报告当前回合的正确视图变化索引
- 删除 `Unregister<Domain>` 上相应的触发器 (#4461)
- 在创世轮中检查创世公钥
- 阻止注册创世域或帐户
- 删除角色对实体注销的权限
- 触发元数据可在智能合约中访问
- 使用 rw 锁来防止不一致的状态视图 (#4867)
- 处理快照中的软分叉 (#4868)
- 修复 ChaCha20Poly1305 的最小尺寸
- 对 LiveQueryStore 添加限制以防止内存使用率过高 (#4893)

### 已删除

- 从 ed25519 私钥中删除公钥 (#4856)
- 删除 kura.lock (#4849)
- 恢复配置中的 `_ms` 和 `_bytes` 后缀 (#4667)
- 从创世字段中删除 `_id` 和 `_file` 后缀 (#4724)
- 通过 AssetDefinitionId 删除 AssetsMap 中的索引资产 (#4701)
- 从触发器身份中删除域 (#4640)
- 从 Iroha 中删除创世签名 (#4673)
- 删除 `Validate` 中绑定的 `Visit` (#4642)
- 删除 `TriggeringEventFilterBox` (#4866)
- 删除 p2p 握手中的 `garbage` (#4889)
- 从块中删除 `committed_topology` (#4880)

### 安全

- 防止秘密泄露

## [2.0.0-pre-rc.21] - 2024-04-19

### 添加

- 在触发器入口点中包含触发器 ID (#4391)
- 将事件集公开为架构中的位字段 (#4381)
- 引入具有精细访问权限的新 `wsv` (#2664)
- 添加 `PermissionTokenSchemaUpdate`、`Configuration` 和 `Executor` 事件的事件过滤器
- 引入快照“模式”(#4365)
- 允许授予/撤销角色的权限（#4244）
- 为资产引入任意精度数字类型（删除所有其他数字类型）（#3660）
- 执行器的不同燃料限制（#3354）
- 集成 pprof 分析器 (#4250)
- 在客户端 CLI 中添加 asset 子命令 (#4200)
- `Register<AssetDefinition>` 权限 (#4049)
- 添加 `chain_id` 以防止重放攻击 (#4185)
- 添加子命令以在客户端 CLI 中编辑域元数据 (#4175)
- 在客户端 CLI 中实现存储设置、删除、获取操作 (#4163)
- 计算触发器的相同智能合约（#4133）
- 将子命令添加到客户端 CLI 中以传输域 (#3974)
- 支持 FFI 中的盒装切片 (#4062)
- git commit SHA 到客户端 CLI (#4042)
- 默认验证器样板的 proc 宏 (#3856)
- 将查询请求构建器引入客户端 API (#3124)
- 智能合约内的惰性查询（#3929）
- `fetch_size` 查询参数 (#3900)
- 资产商店转移指令（#4258）
- 防止秘密泄露（#3240）
- 使用相同源代码删除重复触发器 (#4419)

### 已更改- 将 Rust 工具链升级为 nightly-2024-04-18
- 将块发送到 Set B 验证器 (#4387)
- 将管道事件拆分为块事件和事务事件 (#4366)
- 将 `[telemetry.dev]` 配置部分重命名为 `[dev_telemetry]` (#4377)
- 使 `Action` 和 `Filter` 非通用类型 (#4375)
- 使用构建器模式改进事件过滤 API (#3068)
- 统一各种事件过滤器API，引入流畅的构建器API
- 将 `FilterBox` 重命名为 `EventFilterBox`
- 将 `TriggeringFilterBox` 重命名为 `TriggeringEventFilterBox`
- 改进过滤器命名，例如`AccountFilter` -> `AccountEventFilter`
- 根据配置 RFC 重写配置 (#4239)
- 从公共 API 中隐藏版本化结构的内部结构 (#3887)
- 在多次失败的视图更改后暂时引入可预测的排序（#4263）
- 在 `iroha_crypto` 中使用具体的密钥类型 (#4181)
- 分割视图与普通消息不同 (#4115)
- 使 `SignedTransaction` 不可变 (#4162)
- 通过 `iroha_client` 导出 `iroha_config` (#4147)
- 通过 `iroha_client` 导出 `iroha_crypto` (#4149)
- 通过 `iroha_client` 导出 `data_model` (#4081)
- 从 `iroha_crypto` 中删除 `openssl-sys` 依赖性，并向 `iroha_client` 引入可配置的 tls 后端 (#3422)
- 用内部解决方案 `iroha_crypto` 替换未维护的 EOF `hyperledger/ursa` (#3422)
- 优化执行器性能（#4013）
- 拓扑对等点更新 (#3995)

### 已修复

- 删除 `Unregister<Domain>` 上相应的触发器 (#4461)
- 删除实体注销角色的权限 (#4242)
- 断言创世交易由创世公钥签名 (#4253)
- 为 p2p 中无响应的对等点引入超时 (#4267)
- 防止注册创世域或帐户 (#4226)
- `MinSize` 为 `ChaCha20Poly1305` (#4395)
- 启用 `tokio-console` 时启动控制台 (#4377)
- 用 `\n` 分隔每个项目，并递归地为 `dev-telemetry` 文件日志创建父目录
- 防止未经签名的帐户注册（#4212）
- 密钥对生成现在不会出错 (#4283)
- 停止将 `X25519` 密钥编码为 `Ed25519` (#4174)
- 在 `no_std` 中进行签名验证 (#4270)
- 在异步上下文中调用阻塞方法（#4211）
- 撤销实体注销时的关联令牌 (#3962)
- 启动 Sumeragi 时的异步阻塞错误
- 修复了 `(get|set)_config` 401 HTTP (#4177)
- Docker 中的 `musl` 存档器名称 (#4193)
- 智能合约调试打印（#4178）
- 重启时拓扑更新 (#4164)
- 新节点的注册 (#4142)
- 链上可预测迭代顺序 (#4130)
- 重新构建记录器和动态配置（#4100）
- 触发原子性（#4106）
- 查询存储消息排序问题 (#4057)
- 为使用 Norito 进行回复的端点设置 `Content-Type: application/x-norito`

### 已删除

- `logger.tokio_console_address` 配置参数 (#4377)
- `NotificationEvent` (#4377)
- `Value` 枚举 (#4305)
- iroha 的 MST 聚合 (#4229)
- 智能合约中 ISI 和查询执行的克隆 (#4182)
- `bridge` 和 `dex` 功能 (#4152)
- 扁平化事件（#3068）
- 表达式 (#4089)
- 自动生成的配置参考
- `warp` 日志中的噪音 (#4097)

### 安全

- 防止 p2p 中的 pub 密钥欺骗 (#4065)
- 确保来自 OpenSSL 的 `secp256k1` 签名标准化 (#4155)

## [2.0.0-pre-rc.20] - 2023-10-17

### 添加

- 转让`Domain`所有权
- `Domain` 所有者权限
- 将 `owned_by` 字段添加到 `Domain`
- 将 `iroha_client_cli` 中的过滤器解析为 JSON5 (#3923)
- 添加对在 serde 部分标记的枚举中使用 Self 类型的支持
- 标准化区块 API (#3884)
- 实现 `Fast` kura 初始化模式
- 添加 iroha_swarm 免责声明标头
- 初步支持 WSV 快照

### 已修复

- 修复 update_configs.sh 中的执行程序下载 (#3990)
- devShell 中正确的 rustc
- 修复刻录 `Trigger` 重述
- 修复传输 `AssetDefinition`
- 修复 `RemoveKeyValue` 为 `Domain`
- 修复 `Span::join` 的使用
- 修复拓扑不匹配错误 (#3903)
- 修复 `apply_blocks` 和 `validate_blocks` 基准测试
- `mkdir -r` 具有存储路径，而不是锁定路径 (#3908)
- 如果 test_env.py 中存在 dir，则不会失败
- 修复身份验证/授权文档字符串 (#3876)
- 更好的查询查找错误的错误消息
- 将创世账户公钥添加到 dev docker compose
- 将权限令牌有效负载与 JSON 进行比较 (#3855)
- 修复 `#[model]` 宏中的 `irrefutable_let_patterns`
- 允许创世执行任何 ISI (#3850)
- 修复创世验证（#3844）
- 修复 3 个或更少对等点的拓扑
- 更正 tx_amounts 直方图的计算方式。
- `genesis_transactions_are_validated()` 测试片状
- 默认验证器生成
- 修复 iroha 正常关机问题

### 重构

- 删除未使用的依赖项 (#3992)
- 凹凸依赖项 (#3981)
- 将验证器重命名为执行器 (#3976)
- 删除 `IsAssetDefinitionOwner` (#3979)
- 将智能合约代码包含到工作区中 (#3944)
- 将 API 和遥测端点合并到单个服务器中
- 将表达式 len 从公共 API 移至核心 (#3949)
- 避免角色查找中的克隆
- 角色范围查询
- 将帐户角色移至 `WSV`
- 将 ISI 从 *Box 重命名为 *Expr (#3930)
- 从版本化容器中删除“Versioned”前缀 (#3913)
- 将 `commit_topology` 移动到块有效负载中 (#3916)
- 将 `telemetry_future` 宏迁移到 syn 2.0
- 在 ISI 范围内注册可识别 (#3925)
- 为 `derive(HasOrigin)` 添加基本泛型支持
- 清理 Emitter API 文档以使 Clippy 满意
- 添加对导出（HasOrigin）宏的测试，减少导出（IdEqOrdHash）中的重复，修复稳定版上的错误报告
- 改进命名，简化重复的 .filter_maps 并删除derive(Filter)中不必要的 . except
- 让 PartiallyTaggedSerialize/Deserialize 使用亲爱的
- 让derive(IdEqOrdHash)使用亲爱的，添加测试
- 让衍生（过滤器）使用亲爱的
- 更新 iroha_data_model_derive 以使用 syn 2.0
- 添加签名检查条件单元测试
- 只允许一组固定的签名验证条件
- 将 ConstBytes 推广到保存任何 const 序列的 ConstVec
- 对不改变的字节值使用更有效的表示
- 将最终的 wsv 存储在快照中
- 添加 `SnapshotMaker` 演员
- 解析的文档限制源自 proc 宏
- 清理评论
- 提取用于解析 lib.rs 属性的通用测试实用程序
- 使用 parse_display 并更新 Attr -> Attrs 命名
- 允许在 ffi 函数参数中使用模式匹配
- 减少 getset attrs 解析中的重复
- 将 Emitter::into_token_stream 重命名为 Emitter::finish_token_stream
- 使用parse_display来解析getset标记
- 修复拼写错误并改进错误消息
- iroha_ffi_derive：使用darling解析属性并使用syn 2.0
- iroha_ffi_derive：用 Manyhow 替换 proc-macro-error
- 简化 kura 锁文件代码
- 使所有数值序列化为字符串文字
- 分离出 Kagami (#3841)
- 重写`scripts/test-env.sh`
- 区分智能合约和触发入口点
- 在 `data_model/src/block.rs` 中删除 `.cloned()`
- 更新 `iroha_schema_derive` 以使用 syn 2.0

## [2.0.0-pre-rc.19] - 2023-08-14

### 添加

- hyperledger#3309 Bump IVM 运行时改进
- hyperledger#3383 实现宏以在编译时解析套接字地址
- hyperledger#2398 添加查询过滤器的集成测试
- 在 `InternalError` 中包含实际错误消息
- 使用 `nightly-2023-06-25` 作为默认工具链
- hyperledger#3692 验证器迁移
- [DSL实习] hyperledger#3688：将基本算术实现为 proc 宏
- hyperledger#3371 拆分验证器 `entrypoint` 以确保验证器不再被视为智能合约
- hyperledger#3651 WSV 快照，允许在崩溃后快速启动 Iroha 节点
- hyperledger#3752 将 `MockValidator` 替换为接受所有交易的 `Initial` 验证器
- hyperledger#3276 添加名为 `Log` 的临时指令，该指令将指定字符串记录到 Iroha 节点的主日志中
- hyperledger#3641 使权限令牌有效负载易于理解
- hyperledger#3324 添加 `iroha_client_cli` 相关的 `burn` 检查和重构
- hyperledger#3781 验证创世交易
- hyperledger#2885 区分可以和不能用于触发器的事件
- hyperledger#2245 基于 `Nix` 的 iroha 节点二进制文件构建为 `AppImage`

### 已修复- hyperledger#3613 回归可能允许接受错误签名的交易
- 尽早拒绝不正确的配置拓扑
- hyperledger#3445 修复回归并使 `/configuration` 端点上的 `POST` 再次工作
- hyperledger#3654 修复要部署的基于 `iroha2` `glibc` 的 `Dockerfiles`
- hyperledger#3451 修复 Apple Silicon Mac 上的 `docker` 构建
- hyperledger#3741 修复 `kagami validator` 中的 `tempfile` 错误
- hyperledger#3758 修复无法构建单个板条箱的回归，但可以将其构建为工作区的一部分
- hyperledger#3777 角色注册中的补丁漏洞未得到验证
- hyperledger#3805 修复 Iroha 收到 `SIGTERM` 后不关闭的问题

### 其他

- hyperledger#3648 在 CI 流程中包含 `docker-compose.*.yml` 检查
- 将指令 `len()` 从 `iroha_data_model` 移至 `iroha_core`
- hyperledger#3672 在派生宏中将 `HashMap` 替换为 `FxHashMap`
- hyperledger#3374 统一错误的文档注释和 `fmt::Display` 实现
- hyperledger#3289 在整个项目中使用 Rust 1.70 工作区继承
- hyperledger#3654 添加 `Dockerfiles` 以在 `GNU libc <https://www.gnu.org/software/libc/>`_ 上构建 iroha2
- 为 proc 宏引入 `syn` 2.0、`manyhow` 和 `darling`
- hyperledger#3802 Unicode `kagami crypto` 种子

## [2.0.0-rc.18 之前]

### 添加

- hyperledger#3468：服务器端游标，允许延迟评估可重入分页，这应该对查询延迟产生重大的积极性能影响
- hyperledger#3624：通用权限令牌；具体来说
  - 权限令牌可以具有任何结构
  - 令牌结构在 `iroha_schema` 中进行自我描述并序列化为 JSON 字符串
  - 令牌值是 `Norito` 编码的
  - 由于此更改，权限令牌命名约定从 `snake_case` 移至 `UpeerCamelCase`
- hyperledger#3615 验证后保留 wsv

### 已修复

- hyperledger#3627 现在通过克隆 `WorlStateView` 强制执行事务原子性
- hyperledger#3195 扩展接收被拒绝的创世交易时的恐慌行为
- hyperledger#3042 修复错误的请求消息
- hyperledger#3352 将控制流和数据消息拆分到单独的通道中
- hyperledger#3543 提高指标精度

## 2.0.0-rc.17 之前

### 添加

- hyperledger#3330 扩展 `NumericValue` 反序列化
- FFI 中的 hyperledger#2622 `u128`/`i128` 支持
- hyperledger#3088 引入队列限制，以防止 DoS
- hyperledger#2373 `kagami swarm file` 和 `kagami swarm dir` 用于生成 `docker-compose` 文件的命令变体
- hyperledger#3597 权限令牌分析（Iroha 侧）
- hyperledger#3353 通过枚举错误条件并使用强类型错误，从 `block.rs` 中删除 `eyre`
- hyperledger#3318 Interleave 块中拒绝和接受的交易以保留交易处理顺序

### 已修复

- hyperledger#3075 对 `genesis.json` 中的无效交易产生恐慌，以防止处理无效交易
- hyperledger#3461 正确处理默认配置中的默认值
- hyperledger#3548 修复 `IntoSchema` 透明属性
- hyperledger#3552 修复验证器路径模式表示
- hyperledger#3546 修复时间触发器卡住的问题
- hyperledger#3162 禁止块流请求中的高度为 0
- 配置宏初始测试
- hyperledger#3592 修复了 `release` 上更新的配置文件
- hyperledger#3246 不要涉及没有 `fault <https://en.wikipedia.org/wiki/Byzantine_fault>`_ 的 `Set B validators <https://github.com/hyperledger-iroha/iroha/blob/main/docs/source/iroha_2_whitepaper.md#2-system-architecture>`_
- hyperledger#3570 正确显示客户端字符串查询错误
- hyperledger#3596 `iroha_client_cli` 显示块/事件
- hyperledger#3473 使 `kagami validator` 从 iroha 存储库根目录外部工作

### 其他

- hyperledger#3063 将交易 `hash` 映射到 `wsv` 中的区块高度
- `Value` 中的强类型 `HashOf<T>`

## [2.0.0-rc.16 之前]

### 添加

- hyperledger#2373 `kagami swarm` 用于生成 `docker-compose.yml` 的子命令
- hyperledger#3525 标准化交易 API
- hyperledger#3376 添加 Iroha 客户端 CLI `pytest <https://docs.pytest.org/en/7.4.x/>`_ 自动化框架
- hyperledger#3516 在 `LoadedExecutable` 中保留原始 blob 哈希值

### 已修复

- hyperledger#3462 将 `burn` 资产命令添加到 `client_cli`
- hyperledger#3233 重构错误类型
- hyperledger#3330 通过手动为 `partially-tagged <https://serde.rs/enum-representations.html>`_ `enums` 实现 `serde::de::Deserialize` 修复回归
- hyperledger#3487 将缺失的类型返回到模式中
- hyperledger#3444 将判别式返回到模式中
- hyperledger#3496 修复 `SocketAddr` 字段解析
- hyperledger#3498 修复软分叉检测
- hyperledger#3396 在发出块提交事件之前将块存储在 `kura` 中

### 其他

- hyperledger#2817 从 `WorldStateView` 中删除内部可变性
- hyperledger#3363 Genesis API 重构
- 重构现有拓扑并补充新的拓扑测试
- 从 `Codecov <https://about.codecov.io/>`_ 切换到 `Coveralls <https://coveralls.io/>`_ 以进行测试覆盖
- hyperledger#3533 在架构中将 `Bool` 重命名为 `bool`

## [2.0.0-rc.15 之前]

### 添加

- hyperledger#3231 整体验证器
- hyperledger#3015 支持 FFI 中的利基优化
- hyperledger#2547 将徽标添加到 `AssetDefinition`
- hyperledger#3274 在 `kagami` 中添加一个生成示例的子命令（向后移植到 LTS）
- hyperledger#3415 `Nix <https://nixos.wiki/wiki/Flakes>`_ 薄片
- hyperledger#3412 将交易八卦移至单独的参与者
- hyperledger#3435 引入 `Expression` 访客
- hyperledger#3168 提供创世验证器作为单独的文件
- hyperledger#3454 将 LTS 设置为大多数 Docker 操作和文档的默认设置
- hyperledger#3090 将链上参数从区块链传播到 `sumeragi`

### 已修复

- hyperledger#3330 使用 `u128` 叶修复未标记枚举反序列化（向后移植到 RC14）
- hyperledger#2581 减少了日志中的噪音
- hyperledger#3360 修复 `tx/s` 基准
- hyperledger#3393 打破 `actors` 中的通信死锁循环
- hyperledger#3402 修复 `nightly` 版本
- hyperledger#3411 正确处理对等点同时连接
- hyperledger#3440 不赞成在转移期间进行资产转换，而是由智能合约处理
- hyperledger#3408：修复 `public_keys_cannot_be_burned_to_nothing` 测试

### 其他

- hyperledger#3362 迁移到 `tokio` 参与者
- hyperledger#3349 从智能合约中删除 `EvaluateOnHost`
- hyperledger#1786 为套接字地址添加 `iroha` 本机类型
- 禁用 IVM 缓存
- 重新启用 IVM 缓存
- 将权限验证器重命名为验证器
- hyperledger#3388 使 `model!` 成为模块级属性宏
- hyperledger#3370 将 `hash` 序列化为十六进制字符串
- 将 `maximum_transactions_in_block` 从 `queue` 移动到 `sumeragi` 配置
- 弃用并删除 `AssetDefinitionEntry` 类型
- 将 `configs/client_cli` 重命名为 `configs/client`
- 更新 `MAINTAINERS.md`

## [2.0.0-rc.14 之前]

### 添加

- hyperledger#3127 数据模型 `structs` 默认情况下不透明
- hyperledger#3122 使用 `Algorithm` 存储摘要函数（社区贡献者）
- hyperledger#3153 `iroha_client_cli` 输出是机器可读的
- hyperledger#3105 为 `AssetDefinition` 实现 `Transfer`
- 添加了 hyperledger#3010 `Transaction` 过期管道事件

### 已修复

- hyperledger#3113 不稳定网络测试的修订版
- hyperledger#3129 修复 `Parameter` 反/序列化
- hyperledger#3141 为 `Hash` 手动实现 `IntoSchema`
- hyperledger#3155 修复测试中的恐慌钩子，防止死锁
- hyperledger#3166 不要在空闲时查看更改，提高性能
- hyperledger#2123 从多重哈希返回公钥反/序列化
- hyperledger#3132 添加 NewParameter 验证器
- hyperledger#3249 将块哈希拆分为部分和完整版本
- hyperledger#3031 修复缺少配置参数的 UI/UX
- hyperledger#3247 从 `sumeragi` 中删除了故障注入。

### 其他

- 添加缺失的 `#[cfg(debug_assertions)]` 以修复虚假故障
- hyperledger#2133 重写拓扑以更接近白皮书
- 删除 `iroha_client` 对 `iroha_core` 的依赖
- hyperledger#2943 派生 `HasOrigin`
- hyperledger#3232 共享工作区元数据
- hyperledger#3254 重构 `commit_block()` 和 `replace_top_block()`
- 使用稳定的默认分配器处理程序
- hyperledger#3183 重命名 `docker-compose.yml` 文件
- 改进了`Multihash`显示格式
- hyperledger#3268 全球唯一的项目标识符
- 新的公关模板

## [2.0.0-rc.13 之前]

### 添加- hyperledger#2399 将参数配置为 ISI。
- hyperledger#3119 添加 `dropped_messages` 指标。
- hyperledger#3094 与 `n` 对等点生成网络。
- hyperledger#3082 在 `Created` 事件中提供完整数据。
- hyperledger#3021 不透明指针导入。
- hyperledger#2794 在 FFI 中拒绝具有显式判别式的无字段枚举。
- hyperledger#2922 将 `Grant<Role>` 添加到默认创世。
- hyperledger#2922 省略 `NewRole` json 反序列化中的 `inner` 字段。
- hyperledger#2922 在 json 反序列化中省略 `object(_id)`。
- hyperledger#2922 在 json 反序列化中省略 `Id`。
- hyperledger#2922 在 json 反序列化中省略 `Identifiable`。
- hyperledger#2963 将 `queue_size` 添加到指标中。
- hyperledger#3027 为 Kura 实现锁定文件。
- hyperledger#2813 Kagami 生成默认对等配置。
- hyperledger#3019 支持 JSON5。
- hyperledger#2231 生成 FFI 包装器 API。
- hyperledger#2999 累积区块签名。
- hyperledger#2995 软分叉检测。
- hyperledger#2905 扩展算术运算以支持 `NumericValue`
- hyperledger#2868 发出 iroha 版本并在日志中提交哈希值。
- hyperledger#2096 查询资产总量。
- hyperledger#2899 将多指令子命令添加到“client_cli”中
- hyperledger#2247 消除 websocket 通信噪音。
- hyperledger#2889 在 `iroha_client` 中添加块流支持
- hyperledger#2280 授予/撤销角色时生成权限事件。
- hyperledger#2797 丰富事件。
- hyperledger#2725 将超时重新引入 `submit_transaction_blocking`
- hyperledger#2712 配置属性测试。
- hyperledger#2491 FFi 中的枚举支持。
- hyperledger#2775 在合成创世中生成不同的密钥。
- hyperledger#2627 配置完成、代理入口点、kagami docgen。
- hyperledger#2765 在 `kagami` 中生成合成创世
- hyperledger#2698 修复 `iroha_client` 中不清楚的错误消息
- hyperledger#2689 添加权限令牌定义参数。
- hyperledger#2502 存储构建的 GIT 哈希值。
- hyperledger#2672 添加 `ipv4Addr`、`ipv6Addr` 变体和谓词。
- hyperledger#2626 实现 `Combine` 派生、拆分 `config` 宏。
- hyperledger#2586 `Builder` 和 `LoadFromEnv` 用于代理结构。
- hyperledger#2611 为通用不透明结构派生 `TryFromReprC` 和 `IntoFfi`。
- hyperledger#2587 将 `Configurable` 拆分为两个特征。 ＃2587：将 `Configurable` 分成两个特征
- hyperledger#2488 在 `ffi_export` 中添加对特征实现的支持
- hyperledger#2553 向资产查询添加排序。
- hyperledger#2407 参数化触发器。
- hyperledger#2536 为 FFI 客户端引入 `ffi_import`。
- hyperledger#2338 添加 `cargo-all-features` 检测。
- hyperledger#2564 Kagami 工具算法选项。
- hyperledger#2490 为独立函数实现 ffi_export。
- hyperledger#1891 验证触发器执行。
- hyperledger#1988 派生可识别、Eq、Hash、Ord 的宏。
- hyperledger#2434 FFI 绑定生成库。
- hyperledger#2073 对于区块链中的类型，更喜欢 ConstString 而不是 String。
- hyperledger#1889 添加域范围的触发器。
- hyperledger#2098 区块头查询。 ＃2098：添加块头查询
- hyperledger#2467 将帐户授予子命令添加到 iroha_client_cli 中。
- hyperledger#2301 在查询时添加交易的块哈希。
 - hyperledger#2454 将构建脚本添加到 Norito 解码器工具。
- hyperledger#2061 导出过滤器宏。
- hyperledger#2228 将未经授权的变体添加到智能合约查询错误。
- hyperledger#2395 如果无法应用创世，请添加恐慌。
- hyperledger#2000 禁止空名称。 #2000：禁止空名称
 - hyperledger#2127 添加健全性检查，以确保消耗由 Norito 编解码器解码的所有数据。
- hyperledger#2360 再次使 `genesis.json` 可选。
- hyperledger#2053 向私有区块链中的所有剩余查询添加测试。
- hyperledger#2381 统一 `Role` 注册。
- hyperledger#2053 添加对私有区块链中资产相关查询的测试。
- hyperledger#2053 将测试添加到“private_blockchain”
- hyperledger#2302 添加“FindTriggersByDomainId”存根查询。
- hyperledger#1998 向查询添加过滤器。
- hyperledger#2276 将当前块哈希包含到 BlockHeaderValue 中。
- hyperledger#2161 句柄 id 和共享 FFI fns。
- 添加句柄 id 并实现共享特征的 FFI 等效项（Clone、Eq、Ord）
- hyperledger#1638 `configuration` 返回文档子树。
- hyperledger#2132 添加 `endpointN` 过程宏。
- hyperledger#2257 Revoke<Role> 发出 RoleRevoked 事件。
- hyperledger#2125 添加 FindAssetDefinitionById 查询。
- hyperledger#1926 添加信号处理和正常关闭。
- hyperledger#2161 为 `data_model` 生成 FFI 函数
- hyperledger#1149 每个目录的块文件计数不超过 1000000。
- hyperledger#1413 添加 API 版本端点。
- hyperledger#2103 支持查询区块和交易。添加 `FindAllTransactions` 查询
- hyperledger#2186 为 `BigQuantity` 和 `Fixed` 添加传输 ISI。
- hyperledger#2056 为 `AssetValueType` `enum` 添加派生过程宏包。
- hyperledger#2100 添加查询以查找所有拥有资产的账户。
- hyperledger#2179 优化触发器执行。
- hyperledger#1883 删除嵌入的配置文件。
- hyperledger#2105 处理客户端中的查询错误。
- hyperledger#2050 添加与角色相关的查询。
- hyperledger#1572 专门的权限令牌。
- hyperledger#2121 检查密钥对在构造时是否有效。
 - hyperledger#2003 引入 Norito 解码器工具。
- hyperledger#1952 添加 TPS 基准作为优化标准。
- hyperledger#2040 添加具有事务执行限制的集成测试。
- hyperledger#1890 引入基于 Orillion 用例的集成测试。
- hyperledger#2048 添加工具链文件。
- hyperledger#2100 添加查询以查找所有拥有资产的账户。
- hyperledger#2179 优化触发器执行。
- hyperledger#1883 删除嵌入的配置文件。
- hyperledger#2004 禁止 `isize` 和 `usize` 变为 `IntoSchema`。
- hyperledger#2105 处理客户端中的查询错误。
- hyperledger#2050 添加与角色相关的查询。
- hyperledger#1572 专门的权限令牌。
- hyperledger#2121 检查密钥对在构造时是否有效。
 - hyperledger#2003 引入 Norito 解码器工具。
- hyperledger#1952 添加 TPS 基准作为优化标准。
- hyperledger#2040 添加具有事务执行限制的集成测试。
- hyperledger#1890 引入基于 Orillion 用例的集成测试。
- hyperledger#2048 添加工具链文件。
- hyperledger#2037 引入预提交触发器。
- hyperledger#1621 引入调用触发器。
- hyperledger#1970 添加可选的模式端点。
- hyperledger#1620 引入基于时间的触发器。
- hyperledger#1918 实现 `client` 的基本身份验证
- hyperledger#1726 实施发布 PR 工作流程。
- hyperledger#1815 使查询响应更加类型结构化。
- hyperledger#1928 使用 `gitchangelog` 实现变更日志生成
- hyperledger#1902 裸机 4 点设置脚本。

  添加了 setup_test_env.sh 的版本，该版本不需要 docker-compose 并使用 Iroha 的调试版本。
- hyperledger#1619 引入基于事件的触发器。
- hyperledger#1195 干净地关闭 websocket 连接。
- hyperledger#1606 在域结构中添加 ipfs 链接到域徽标。
- hyperledger#1754 添加 Kura 检查器 CLI。
- hyperledger#1790 通过使用基于堆栈的向量提高性能。
- hyperledger#1805 用于紧急错误的可选终端颜色。
- `data_model` 中的超级账本#1749 `no_std`
- hyperledger#1179 添加撤销权限或角色指令。
- hyperledger#1782 使 iroha_crypto no_std 兼容。
- hyperledger#1172 实现指令事件。
- hyperledger#1734 验证 `Name` 以排除空格。
- hyperledger#1144 添加元数据嵌套。
- #1210 块流（服务器端）。
- hyperledger#1331 实施更多 `Prometheus` 指标。
- hyperledger#1689 修复功能依赖性。 ＃1261：添加货物膨胀。
- hyperledger#1675 对版本化项目使用类型而不是包装结构。
- hyperledger#1643 等待同行在测试中提交创世。
- 超级账本#1678 `try_allocate`
- hyperledger#1216 添加 Prometheus 端点。 #1216：指标端点的初始实现。
- hyperledger#1238 运行时日志级别更新。创建了基本的基于 `connection` 入口点的重新加载。
- hyperledger#1652 PR 标题格式。
- 将已连接对等点的数量添加到 `Status`

  - 恢复“删除与连接对等点数量相关的内容”

  这将恢复提交 b228b41dab3c035ce9973b6aa3b35d443c082544。
  - 澄清 `Peer` 仅在握手后才具有真正的公钥
  - `DisconnectPeer` 未经测试
  - 实现取消注册对等执行
  - 将（取消）注册对等子命令添加到 `client_cli`
  - 通过其地址拒绝来自未注册对等点的重新连接在您的对等点取消注册并断开另一个对等点的连接后，
  您的网络将听到来自对等方的重新连接请求。
  首先你只能知道端口号是任意的地址。
  因此，请通过端口号以外的部分来记住未注册的对等点
  并拒绝从那里重新连接
- 将 `/status` 端点添加到特定端口。

### 修复- hyperledger#3129 修复 `Parameter` 反/序列化。
- hyperledger#3109 防止 `sumeragi` 在角色不可知消息后休眠。
- hyperledger#3046 确保 Iroha 可以在空时正常启动
  `./storage`
- hyperledger#2599 删除 Nursery lints。
- hyperledger#3087 在视图更改后从 Set B 验证器收集投票。
- hyperledger#3056 修复 `tps-dev` 基准测试挂起。
- hyperledger#1170 实现克隆 wsv 风格的软分叉处理。
- hyperledger#2456 使创世块不受限制。
- hyperledger#3038 重新启用多重签名。
- hyperledger#2894 修复 `LOG_FILE_PATH` env 变量反序列化。
- hyperledger#2803 返回签名错误的正确状态代码。
- hyperledger#2963 `Queue` 正确删除交易。
- hyperledger#0000 Vergen 破坏 CI。
- hyperledger#2165 删除工具链烦躁。
- hyperledger#2506 修复区块验证。
- hyperledger#3013 正确的链燃烧验证器。
- hyperledger#2998 删除未使用的链码。
- hyperledger#2816 将访问区块的责任移交给 kura。
- hyperledger#2384 将解码替换为decode_all。
- hyperledger#1967 将 ValueName 替换为 Name。
- hyperledger#2980 修复区块值 ffi 类型。
- hyperledger#2858 引入 parking_lot::Mutex 而不是 std。
- hyperledger#2850 修复 `Fixed` 的反序列化/解码
- hyperledger#2923 当 `AssetDefinition` 不返回时返回 `FindError`
  存在。
- hyperledger#0000 修复 `panic_on_invalid_genesis.sh`
- hyperledger#2880 正确关闭 websocket 连接。
- hyperledger#2880 修复块流。
- hyperledger#2804 `iroha_client_cli` 提交交易阻塞。
- hyperledger#2819 将非必要成员移出 WSV。
- 修复表达式序列化递归错误。
- hyperledger#2834 改进速记语法。
- hyperledger#2379 添加将新 Kura 块转储到blocks.txt 的功能。
- hyperledger#2758 将排序结构添加到模式中。
- CI。
- hyperledger#2548 对大型创世文件发出警告。
- hyperledger#2638 更新 `whitepaper` 并传播更改。
- hyperledger#2678 修复暂存分支上的测试。
- hyperledger#2678 修复了 Kura 强制关闭时测试中止的问题。
- hyperledger#2607 重构 sumeragi 代码以使其更加简单和
  稳健性修复。
- hyperledger#2561 重新引入视图更改以达成共识。
- hyperledger#2560 添加回 block_sync 和对等断开连接。
- hyperledger#2559 添加 sumeragi 线程关闭。
- hyperledger#2558 在从 kura 更新 wsv 之前验证创世。
- hyperledger#2465 将 sumeragi 节点重新实现为单线程状态
  机。
- hyperledger#2449 Sumeragi 重组的初步实施。
- hyperledger#2802 修复配置的环境加载。
- hyperledger#2787 通知每个监听器在恐慌时关闭。
- hyperledger#2764 删除最大消息大小的限制。
- #2571：更好的 Kura Inspector UX。
- hyperledger#2703 修复 Orillion 开发环境错误。
- 修复 schema/src 中文档注释中的拼写错误。
- hyperledger#2716 公开正常运行时间的持续时间。
- hyperledger#2700 在 docker 镜像中导出 `KURA_BLOCK_STORE_PATH`。
- hyperledger#0 从构建器中删除 `/iroha/rust-toolchain.toml`
  图像。
- hyperledger#0 修复 `docker-compose-single.yml`
- hyperledger#2554 如果 `secp256k1` 种子短于 32，则引发错误
  字节。
- hyperledger#0 修改 `test_env.sh` 为每个对等点分配存储。
- hyperledger#2457 在测试中强制关闭 kura。
- hyperledger#2623 修复 VariantCount 的文档测试。
- 更新 ui_fail 测试中的预期错误。
- 修复权限验证器中不正确的文档注释。
- hyperledger#2422 在配置端点响应中隐藏私钥。
- hyperledger#2492：修复并非所有正在执行的与事件匹配的触发器。
- hyperledger#2504 修复失败的 tps 基准测试。
- hyperledger#2477 修复不计算角色权限时的错误。
- hyperledger#2416 修复 macOS 手臂上的 lints。
- hyperledger#2457 修复与恐慌关闭相关的测试不稳定问题。
  ＃2457：添加紧急关闭配置
- hyperledger#2473 解析 rustc --version 而不是 RUSTUP_TOOLCHAIN。
- hyperledger#1480 因恐慌而关闭。 ＃1480：添加恐慌挂钩以在恐慌时退出程序
- hyperledger#2376 简化的 Kura，无异步，两个文件。
- hyperledger#0000 Docker 构建失败。
- hyperledger#1649 从 `do_send` 中删除 `spawn`
- hyperledger#2128 修复 `MerkleTree` 构造和迭代。
- hyperledger#2137 为多进程上下文准备测试。
- hyperledger#2227 实现资产注册和注销。
- hyperledger#2081 修复角色授予错误。
- hyperledger#2358 添加带有调试配置文件的版本。
- hyperledger#2294 将火焰图生成添加到 oneshot.rs。
- hyperledger#2202 修复查询响应中的总计字段。
- hyperledger#2081 修复测试用例以授予角色。
- hyperledger#2017 修复角色注销问题。
- hyperledger#2303 修复 docker-compose' 对等点无法正常关闭的问题。
- hyperledger#2295 修复取消注册触发错误。
- hyperledger#2282 改进源自 getset 实现的 FFI。
- hyperledger#1149 删除 nocheckin 代码。
- hyperledger#2232 当创世有太多 isi 时，使 Iroha 打印有意义的消息。
- hyperledger#2170 修复 M1 机器上 docker 容器中的构建。
- hyperledger#2215 使 nightly-2022-04-20 对于 `cargo build` 成为可选
- hyperledger#1990 在没有 config.json 的情况下通过环境变量启用对等启动。
- hyperledger#2081 修复角色注册。
- hyperledger#1640 生成 config.json 和 genesis.json。
- hyperledger#1716 修复 f=0 情况下共识失败的问题。
- hyperledger#1845 不可铸造的资产只能铸造一次。
- hyperledger#2005 修复 `Client::listen_for_events()` 未关闭 WebSocket 流。
- hyperledger#1623 创建一个 RawGenesisBlockBuilder。
- hyperledger#1917 添加 easy_from_str_impl 宏。
- hyperledger#1990 在没有 config.json 的情况下通过环境变量启用对等启动。
- hyperledger#2081 修复角色注册。
- hyperledger#1640 生成 config.json 和 genesis.json。
- hyperledger#1716 修复 f=0 情况下共识失败的问题。
- hyperledger#1845 不可铸造的资产只能铸造一次。
- hyperledger#2005 修复 `Client::listen_for_events()` 未关闭 WebSocket 流。
- hyperledger#1623 创建一个 RawGenesisBlockBuilder。
- hyperledger#1917 添加 easy_from_str_impl 宏。
- hyperledger#1922 将 crypto_cli 移至工具中。
- hyperledger#1969 使 `roles` 功能成为默认功能集的一部分。
- hyperledger#2013 修补程序 CLI 参数。
- hyperledger#1897 从序列化中删除 usize/isize。
- hyperledger#1955 修复了在 `web_login` 内部传递 `:` 的可能性
- hyperledger#1943 将查询错误添加到架构中。
- hyperledger#1939 `iroha_config_derive` 的正确功能。
- hyperledger#1908 修复遥测分析脚本的零值处理。
- hyperledger#0000 使隐式忽略的 doc-test 显式忽略。
- hyperledger#1848 防止公钥被烧毁。
- hyperledger#1811 添加了测试和检查以删除受信任的对等密钥。
- hyperledger#1821 为 MerkleTree 和 VersionedValidBlock 添加 IntoSchema，修复 HashOf 和 SignatureOf 架构。
- hyperledger#1819 从验证中的错误报告中删除回溯。
- hyperledger#1774 记录验证失败的确切原因。
- hyperledger#1714 仅通过键比较 PeerId。
- hyperledger#1788 减少 `Value` 的内存占用。
- hyperledger#1804 修复了 HashOf、SignatureOf 的模式生成，添加测试以确保没有模式丢失。
- hyperledger#1802 日志记录可读性改进。
  - 事件日志移至跟踪级别
  - ctx 从日志捕获中删除
  - 终端颜色是可选的（为了更好地将日志输出到文件）
- hyperledger#1783 修复了牌坊基准。
- hyperledger#1772 在#1764 之后修复。
- hyperledger#1755 对 #1743、#1725 进行了小修复。
  - 根据 #1743 `Domain` 结构更改修复 JSON
- hyperledger#1751 共识修复。 ＃1715：共识修复以处理高负载（＃1746）
  - 查看更改处理修复
  - 查看独立于特定交易哈希的变更证明
  - 减少消息传递
  - 收集视图更改投票而不是立即发送消息（提高网络弹性）
  - 在 Sumeragi 中充分使用 Actor 框架（将消息安排给自己而不是任务生成）
  - 改进了 Sumeragi 测试的故障注入
  - 使测试代码更接近生产代码
  - 删除过于复杂的包装
  - 允许 Sumeragi 在测试代码中使用 actor 上下文
- hyperledger#1734 更新创世以适应新的域验证。
- hyperledger#1742 `core` 指令中返回具体错误。
- hyperledger#1404 验证已修复。
- hyperledger#1636 删除 `trusted_peers.json` 和 `structopt`
  #1636：删除 `trusted_peers.json`。
- hyperledger#1706 通过拓扑更新更新 `max_faults`。
- hyperledger#1698 修复了公钥、文档和错误消息。
- 铸币问题（1593 和 1405）第 1405 期

### 重构- 从 sumeragi 主循环中提取函数。
- 将 `ProofChain` 重构为新类型。
- 从 `Metrics` 中删除 `Mutex`
- 删除 adt_const_generics 夜间功能。
- hyperledger#3039 引入多重签名等待缓冲区。
- 简化主。
- hyperledger#3053 修复 Clippy lints。
- hyperledger#2506 添加更多关于块验证的测试。
- 删除 Kura 中的 `BlockStoreTrait`。
- 更新 `nightly-2022-12-22` 的 lint
- hyperledger#3022 删除 `transaction_cache` 中的 `Option`
- hyperledger#3008 将利基价值添加到 `Hash` 中
- 将 lint 更新至 1.65。
- 添加小测试以扩大覆盖范围。
- 从 `FaultInjection` 中删除无效代码
- 减少从 sumeragi 调用 p2p 的次数。
- hyperledger#2675 验证项目名称/ID 而不分配 Vec。
- hyperledger#2974 在没有完全重新验证的情况下防止区块欺骗。
- 组合器中的 `NonEmpty` 更高效。
- hyperledger#2955 从 BlockSigned 消息中删除块。
- hyperledger#1868 防止发送经过验证的交易
  同伴之间。
- hyperledger#2458 实现通用组合器 API。
- 将存储文件夹添加到 gitignore 中。
- hyperledger#2909 nextest 的硬编码端口。
- hyperledger#2747 更改 `LoadFromEnv` API。
- 改进配置失败时的错误消息。
- 向 `genesis.json` 添加额外示例
- 在 `rc9` 发布之前删除未使用的依赖项。
- 完成新 Sumeragi 上的 linting。
- 在主循环中提取子过程。
- hyperledger#2774 将 `kagami` 创世生成模式从标志更改为
  子命令。
- hyperledger#2478 添加 `SignedTransaction`
- hyperledger#2649 从 `Kura` 中删除 `byteorder` 箱
- 将 `DEFAULT_BLOCK_STORE_PATH` 从 `./blocks` 重命名为 `./storage`
- hyperledger#2650 添加 `ThreadHandler` 以关闭 iroha 子模块。
- hyperledger#2482 将 `Account` 权限令牌存储在 `Wsv` 中
- 在 1.62 中添加新的 lint。
- 改进 `p2p` 错误消息。
- hyperledger#2001 `EvaluatesTo` 静态类型检查。
- hyperledger#2052 使权限令牌可通过定义进行注册。
  ＃2052：实施 PermissionTokenDefinition
- 确保所有功能组合均有效。
- hyperledger#2468 从权限验证器中删除调试超级特征。
- hyperledger#2419 删除显式 `drop`s。
- hyperledger#2253 将 `Registrable` 特征添加到 `data_model`
- 对于数据事件实施 `Origin` 而不是 `Identifiable`。
- hyperledger#2369 重构权限验证器。
- hyperledger#2307 使 `WorldStateView` 中的 `events_sender` 成为非可选。
- hyperledger#1985 减少 `Name` 结构的大小。
- 添加更多 `const fn`。
- 使用 `default_permissions()` 进行集成测试
- 在 private_blockchain 中添加权限令牌包装器。
- hyperledger#2292 删除 `WorldTrait`，从 `IsAllowedBoxed` 中删除泛型
- hyperledger#2204 使资产相关操作变得通用。
- hyperledger#2233 将 `impl` 替换为 `derive`（对于 `Display` 和 `Debug`）。
- 可识别的结构改进。
- hyperledger#2323 增强 kura init 错误消息。
- hyperledger#2238 添加对等构建器进行测试。
- hyperledger#2011 更多描述性配置参数。
- hyperledger#1896 简化 `produce_event` 实施。
- 围绕 `QueryError` 进行重构。
- 将 `TriggerSet` 移至 `data_model`。
- hyperledger#2145 重构客户端的 `WebSocket` 端，提取纯数据逻辑。
- 删除 `ValueMarker` 特征。
- hyperledger#2149 在 `prelude` 中公开 `Mintable` 和 `MintabilityError`
- hyperledger#2144 重新设计客户端的 http 工作流程，公开内部 api。
- 移至 `clap`。
- 创建 `iroha_gen` 二进制文件，合并文档，schema_bin。
- hyperledger#2109 使 `integration::events::pipeline` 测试稳定。
- hyperledger#1982 封装对 `iroha_crypto` 结构的访问。
- 添加 `AssetDefinition` 构建器。
- 从 API 中删除不必要的 `&mut`。
- 封装对数据模型结构的访问。
- hyperledger#2144 重新设计客户端的 http 工作流程，公开内部 api。
- 移至 `clap`。
- 创建 `iroha_gen` 二进制文件，合并文档，schema_bin。
- hyperledger#2109 使 `integration::events::pipeline` 测试稳定。
- hyperledger#1982 封装对 `iroha_crypto` 结构的访问。
- 添加 `AssetDefinition` 构建器。
- 从 API 中删除不必要的 `&mut`。
- 封装对数据模型结构的访问。
- 核心，`sumeragi`，实例函数，`torii`
- hyperledger#1903 将事件发射移至 `modify_*` 方法。
- 拆分 `data_model` lib.rs 文件。
- 将 wsv 引用添加到队列中。
- hyperledger#1210 分割事件流。
  - 将交易相关功能移至 data_model/transaction 模块
- hyperledger#1725 删除 Torii 中的全局状态。
  - 实施 `add_state macro_rules` 并删除 `ToriiState`
- 修复 linter 错误。
- hyperledger#1661 `Cargo.toml` 清理。
  - 整理货物依赖关系
- hyperledger#1650 整理 `data_model`
  - 将 World 移至 wsv，修复角色功能，为 CommiedBlock 派生 IntoSchema
- `json` 文件和自述文件的组织。更新自述文件以符合模板。
- 1529：结构化日志记录。
  - 重构日志消息
- `iroha_p2p`
  - 添加 p2p 私有化。

### 文档

- 更新 Iroha 客户端 CLI 自述文件。
- 更新教程片段。
- 将“sort_by_metadata_key”添加到 API 规范中。
- 更新文档链接。
- 使用与资产相关的文档扩展教程。
- 删除过时的文档文件。
- 检查标点符号。
- 将一些文档移至教程存储库。
- 暂存分支的片状报告。
- 为 rc.7 之前的版本生成变更日志。
- 7 月 30 日的不稳定报告。
- 凹凸版本。
- 更新测试片状性。
- hyperledger#2499 修复 client_cli 错误消息。
- hyperledger#2344 为 2.0.0-pre-rc.5-lts 生成变更日志。
- 添加教程链接。
- 更新有关 git hooks 的信息。
- 片状测试记录。
- hyperledger#2193 更新 Iroha 客户端文档。
- hyperledger#2193 更新 Iroha CLI 文档。
- hyperledger#2193 更新宏箱的自述文件。
 - hyperledger#2193 更新 Norito 解码器工具文档。
- hyperledger#2193 更新 Kagami 文档。
- hyperledger#2193 更新基准文档。
- hyperledger#2192 查看贡献指南。
- 修复损坏的代码内引用。
- hyperledger#1280 记录 Iroha 指标。
- hyperledger#2119 添加有关如何在 Docker 容器中热重载 Iroha 的指南。
- hyperledger#2181 查看自述文件。
- hyperledger#2113 Cargo.toml 文件中的文档功能。
- hyperledger#2177 清理 gitchangelog 输出。
- hyperledger#1991 将自述文件添加到 Kura 检查器。
- hyperledger#2119 添加有关如何在 Docker 容器中热重载 Iroha 的指南。
- hyperledger#2181 查看自述文件。
- hyperledger#2113 Cargo.toml 文件中的文档功能。
- hyperledger#2177 清理 gitchangelog 输出。
- hyperledger#1991 将自述文件添加到 Kura 检查器。
- 生成最新的变更日志。
- 生成变更日志。
- 更新过时的自述文件。
- 将缺失的文档添加到 `api_spec.md`。

### CI/CD 更改- 添加另外五个自托管运行器。
- 为Soramitsu 注册表添加常规图像标签。
- libgit2-sys 0.5.0 的解决方法。恢复到 0.4.4。
- 尝试使用基于拱门的图像。
- 更新工作流程以处理新的仅夜间容器。
- 从覆盖范围中删除二进制入口点。
- 将开发测试切换到 Equinix 自托管运行器。
- hyperledger#2865 从 `scripts/check.sh` 中删除 tmp 文件的使用
- hyperledger#2781 添加覆盖范围偏移。
- 禁用缓慢的集成测试。
- 用 docker 缓存替换基础镜像。
- hyperledger#2781 添加 codecov 提交父功能。
- 将工作转移到 github runner。
- hyperledger#2778 客户端配置检查。
- hyperledger#2732 添加更新 iroha2-base 镜像的条件并添加
  公关标签。
- 修复夜间图像构建。
- 修复 `buildx` 错误与 `docker/build-push-action`
- 无功能急救 `tj-actions/changed-files`
- 在 #2662 之后启用图像的顺序发布。
- 添加港口登记处。
- 自动标记 `api-changes` 和 `config-changes`
- 再次提交图像中的哈希值、工具链文件、UI 隔离、
  模式跟踪。
- 使发布工作流程顺序化，并补充#2427。
- hyperledger#2309：在 CI 中重新启用文档测试。
- hyperledger#2165 删除 codecov 安装。
- 移动到新容器以防止与当前用户发生冲突。
 - hyperledger#2158 升级 `parity_scale_codec` 和其他依赖项。 （Norito 编解码器）
- 修复构建。
- hyperledger#2461 改进 iroha2 CI。
- 更新 `syn`。
- 将覆盖范围转移到新的工作流程。
- 反向docker登录版本。
- 删除`archlinux:base-devel`的版本规范
- 更新 Dockerfiles 和 Codecov 报告重用和并发性。
- 生成变更日志。
- 添加 `cargo deny` 文件。
- 添加 `iroha2-lts` 分支，工作流程从 `iroha2` 复制
- hyperledger#2393 更改 Docker 基础镜像的版本。
- hyperledger#1658 添加文档检查。
- 包的版本提升并删除未使用的依赖项。
- 删除不必要的覆盖率报告。
- hyperledger#2222 根据是否涉及覆盖范围进行拆分测试。
- hyperledger#2153 修复#2154。
- 版本碰撞了所有板条箱。
- 修复部署管道。
- hyperledger#2153 修复覆盖范围。
- 添加创世检查和更新文档。
- 将铁锈、霉菌和夜间分别提升至 1.60、1.2.0 和 1.62。
- 负载RS触发器。
- hyperledger#2153 修复#2154。
- 版本碰撞了所有板条箱。
- 修复部署管道。
- hyperledger#2153 修复覆盖范围。
- 添加创世检查和更新文档。
- 将铁锈、霉菌和夜间分别提升至 1.60、1.2.0 和 1.62。
- 负载RS触发器。
- load-rs:release 工作流程触发器。
- 修复推送工作流程。
- 将遥测添加到默认功能。
- 添加适当的标签以将工作流程推送到主干上。
- 修复失败的测试。
- hyperledger#1657 将镜像更新为 Rust 1.57。 #1630：回到自托管运行器。
- CI 改进。
- 将覆盖范围切换为使用 `lld`。
- CI 依赖性修复。
- CI 分段改进。
- 在 CI 中使用固定的 Rust 版本。
- 修复 Docker 发布和 iroha2-dev 推送 CI。将报道和替补转移到 PR
- 删除 CI docker 测试中不必要的完整 Iroha 构建。

  Iroha 构建变得毫无用处，因为它现在是在 docker 镜像本身中完成的。因此 CI 只构建用于测试的客户端 cli。
- 在 CI 管道中添加对 iroha2 分支的支持。
  - 长时间测试仅在 iroha2 的 PR 上运行
  - 仅从 iroha2 发布 docker 镜像
- 额外的 CI 缓存。

### 网络组装


### 版本颠簸

- rc.13 之前的版本。
- rc.11 之前的版本。
- RC.9 版本。
- RC.8 版本。
- 将版本更新至 RC7。
- 发布前的准备工作。
- 更新模具1.0。
- 碰撞依赖性。
- 更新 api_spec.md：修复请求/响应主体。
- 将 Rust 版本更新至 1.56.0。
- 更新贡献指南。
- 更新 README.md 和 `iroha/config.json` 以匹配新的 API 和 URL 格式。
- 将 docker 发布目标更新为 hyperledger/iroha2 #1453。
- 更新工作流程，使其与主要工作流程匹配。
- 更新 API 规范并修复运行状况端点。
- Rust 更新至 1.54。
- 文档（iroha_crypto）：更新 `Signature` 文档并对齐 `verify` 的参数
- Ursa 版本从 0.3.5 升级到 0.3.6。
- 更新新跑步者的工作流程。
- 更新 dockerfile 以进行缓存和更快的 ci 构建。
- 更新 libssl 版本。
- 更新 dockerfiles 和 async-std。
- 修复更新的剪辑。
- 更新资产结构。
  - 支持资产中的键值指令
  - 资产类型作为枚举
  - 修复资产ISI溢出漏洞
- 更新贡献指南。
- 更新过时的库。
- 更新白皮书并修复 linting 问题。
- 更新 cucumber_rust 库。
- 密钥生成的自述文件更新。
- 更新 Github Actions 工作流程。
- 更新 Github Actions 工作流程。
- 更新requirements.txt。
- 更新 common.yaml。
- Sara 的文档更新。
- 更新指令逻辑。
- 更新白皮书。
- 更新网络功能描述。
- 根据评论更新白皮书。
- WSV 更新和迁移到 Scale 的分离。
- 更新 gitignore。
- 稍微更新了 WP 中 kura 的描述。
- 更新白皮书中有关 kura 的描述。

### 架构

- hyperledger#2114 模式中的排序集合支持。
- hyperledger#2108 添加分页。
- hyperledger#2114 模式中的排序集合支持。
- hyperledger#2108 添加分页。
- 使架构、版本和宏 no_std 兼容。
- 修复架构中的签名。
- 改变了模式中 `FixedPoint` 的表示。
- 将 `RawGenesisBlock` 添加到架构自省。
- 更改了对象模型以创建模式 IR-115。

### 测试

- hyperledger#2544 教程文档测试。
- hyperledger#2272 添加“FindAssetDefinitionById”查询的测试。
- 添加 `roles` 集成测试。
- 标准化 ui 测试格式，将派生 ui 测试移至派生 crate。
- 修复模拟测试（期货无序错误）。
- 删除了 DSL 箱并将测试移至 `data_model`
- 确保不稳定的网络测试通过有效代码。
- 添加了对 iroha_p2p 的测试。
- 捕获测试中的日志，除非测试失败。
- 添加测试轮询并修复很少破坏的测试。
- 测试并行设置。
- 从 iroha init 和 iroha_client 测试中删除 root。
- 修复测试剪辑警告并添加对 ci 的检查。
- 修复基准测试期间的 `tx` 验证错误。
- hyperledger#860：Iroha 查询和测试。
- Iroha 自定义 ISI 指南和 Cucumber 测试。
- 添加对非标准客户端的测试。
- 桥梁注册变更和测试。
- 使用网络模拟进行共识测试。
- 使用临时目录来执行测试。
- 工作台测试阳性病例。
- 带有测试的初始默克尔树功能。
- 修复了测试和世界状态视图初始化。

＃＃＃ 其他- 将参数化移至特征并删除 FFI IR 类型。
- 添加对联合的支持，引入 `non_robust_ref_mut` * 实现 conststring FFI 转换。
- 改进 IdOrdEqHash。
- 从（反）序列化中删除 FilterOpt::BySome。
- 使不透明。
- 使 ContextValue 透明。
- 使 Expression::Raw 标记可选。
- 增加一些说明的透明度。
- 改进 RoleId 的（反）序列化。
- 改进验证器::Id 的（反）序列化。
- 改进 PermissionTokenId 的（反）序列化。
- 改进 TriggerId 的（反）序列化。
- 改进资产（定义）ID 的（反）序列化。
- 改进 AccountId 的（反）序列化。
- 改进 Ipfs 和 DomainId 的（反）序列化。
- 从客户端配置中删除记录器配置。
- 添加对 FFI 中透明结构的支持。
- 将 &Option<T> 重构为 Option<&T>
- 修复剪辑警告。
- 在 `Find` 错误描述中添加更多详细信息。
- 修复 `PartialOrd` 和 `Ord` 实现。
- 使用 `rustfmt` 代替 `cargo fmt`
- 删除 `roles` 功能。
- 使用 `rustfmt` 代替 `cargo fmt`
- 将工作目录作为卷与开发 docker 实例共享。
- 删除执行中的 Diff 关联类型。
- 使用自定义编码而不是多值返回。
- 删除 serde_json 作为 iroha_crypto 依赖项。
- 仅允许版本属性中的已知字段。
- 澄清端点的不同端口。
- 删除 `Io` 派生。
- key_pairs 的初始文档。
- 回到自托管运行器。
- 修复代码中新的 Clippy lints。
- 从维护者中删除 i1i1。
- 添加演员文档和小修复。
- 轮询而不是推送最新的区块。
- 对 7 个对等点中的每一个进行测试的交易状态事件。
- `FuturesUnordered` 而不是 `join_all`
- 切换到 GitHub Runners。
- 将 VersionedQueryResult 与 QueryResult 用于 /query 端点。
- 重新连接遥测。
- 修复依赖机器人配置。
- 添加 commit-msg git hook 以包含签核。
- 修复推送管道。
- 升级依赖机器人。
- 检测队列推送的未来时间戳。
- hyperledger#1197：Kura 处理错误。
- 添加取消注册对等指令。
- 添加可选的随机数来区分交易。关闭#1493。
- 删除了不必要的 `sudo`。
- 域的元数据。
- 修复 `create-docker` 工作流程中的随机弹跳。
- 按照失败管道的建议添加了 `buildx`。
- hyperledger#1454：使用特定状态代码和提示修复查询错误响应。
- hyperledger#1533：通过哈希查找交易。
- 修复 `configure` 端点。
- 添加基于布尔的资产可铸造性检查。
- 添加类型化加密原语并迁移到类型安全加密。
- 日志记录改进。
- hyperledger#1458：将参与者通道大小添加到配置中，作为 `mailbox`。
- hyperledger#1451：如果 `faulty_peers = 0` 和 `trusted peers count > 1` 添加有关错误配置的警告
- 添加用于获取特定块哈希的处理程序。
- 添加了新查询 FindTransactionByHash。
- hyperledger#1185：更改板条箱名称和路径。
- 修复日志和一般改进。
- hyperledger#1150：将 1000 个块分组到每个文件中
- 队列压力测试。
- 日志级别修复。
- 将标头规范添加到客户端库。
- 队列恐慌失败修复。
- 修复队列。
- 修复 dockerfile 版本构建。
- HTTPS 客户端修复。
- 加速ci。
- 1. 删除了除 iroha_crypto 之外的所有 ursa 依赖项。
- 修复减去持续时间时的溢出问题。
- 在客户端中公开字段。
- 每晚将 Iroha2 推送到 Dockerhub。
- 修复 http 状态代码。
- 将 iroha_error 替换为 thiserror、eyre 和 color-eyre。
- 用横梁一代替队列。
- 删除一些无用的 lint 配额。
- 引入资产定义的元数据。
- 从 test_network 箱中删除参数。
- 删除不必要的依赖项。
- 修复 iroha_client_cli::events。
- hyperledger#1382：删除旧的网络实现。
- hyperledger#1169：增加了资产的精度。
- 对等启动的改进：
  - 允许仅从环境加载创世公钥
  - 现在可以在 cli 参数中指定 config、genesis 和 trust_peers 路径
- hyperledger#1134：集成 Iroha P2P。
- 将查询端点更改为 POST 而不是 GET。
- 在actor中同步执行on_start。
- 迁移到扭曲。
- 通过代理错误修复重新提交提交。
- 恢复“引入多个代理修复”提交（9c148c33826067585b5868d297dcdd17c0efe246）
- 引入多个代理修复：
  - 在演员停止时取消订阅经纪人
  - 支持同一参与者类型的多个订阅（以前是 TODO）
  - 修复了经纪人总是将自己作为演员 ID 的错误。
- 经纪人错误（测试展示）。
- 添加数据模型的派生。
- 从鸟居中删除 rwlock。
- OOB 查询权限检查。
- hyperledger#1272：对等计数的实现，
- 递归检查指令内的查询权限。
- 安排停止演员。
- hyperledger#1165：对等计数的实现。
- 检查torii端点中帐户的查询权限。
- 删除了系统指标中公开的 CPU 和内存使用情况。
 - 将 WS 消息的 JSON 替换为 Norito。
- 存储视图更改的证明。
- hyperledger#1168：如果交易未通过签名检查条件，则添加日志记录。
- 修复了小问题，添加了连接监听代码。
- 引入网络拓扑生成器。
- 为 Iroha 实现 P2P 网络。
- 添加块大小指标。
- PermissionValidator 特征重命名为 IsAllowed。以及相应的其他名称更改
- API 规范 Web 套接字更正。
- 从 docker 镜像中删除不必要的依赖项。
- Fmt 使用 Crate import_grainarity。
- 引入通用权限验证器。
- 迁移到参与者框架。
- 更改代理设计并为参与者添加一些功能。
- 配置 codecov 状态检查。
- 使用 grcov 进行基于源的覆盖。
- 修复了多个构建参数格式并为中间构建容器重新声明了 ARG。
- 引入 SubscriptionAccepted 消息。
- 操作后从账户中删除零价值资产。
- 修复了 docker 构建参数格式。
- 修复了未找到子块时的错误消息。
- 添加了供应商的 OpenSSL 来构建，修复了 pkg-config 依赖性。
- 修复 dockerhub 的存储库名称和覆盖范围差异。
- 如果无法加载 TrustedPeers，则添加了清晰的错误文本和文件名。
- 将文本实体更改为文档中的链接。
- 修复 Docker 发布中错误的用户名密码。
- 修复白皮书中的小错字。
- 允许使用 mod.rs 以获得更好的文件结构。
- 将 main.rs 移至单独的 crate 中并为公共区块链授予权限。
- 在客户端 cli 内添加查询。
- 从 clap 迁移到 cli 的 structopts。
- 将遥测限制为不稳定的网络测试。
- 将特征移至智能合约模块。
- sed -i“s/world_state_view/wsv/g”
- 将智能合约移至单独的模块中。
- Iroha 网络内容长度错误修复。
- 为参与者 ID 添加任务本地存储。对于死锁检测很有用。
- 在CI中添加死锁检测测试
- 添加内省宏。
- 消除工作流程名称的歧义并进行格式更正
- 更改查询 API。
- 从 async-std 迁移到 tokio。
- 向 ci 添加遥测分析。
- 为 iroha 添加期货遥测。
- 将 iroha futures 添加到每个异步函数中。
- 添加 iroha futures 以方便观察民意调查数量。
- 自述文件中添加了手动部署和配置。
- 记者修复。
- 添加派生消息宏。
- 添加简单的演员框架。
- 添加dependabot配置。
- 添加漂亮的恐慌和错误报告器。
- Rust 版本迁移到 1.52.1 并进行相应修复。
- 在单独的线程中生成阻塞 CPU 密集型任务。
- 使用来自 crates.io 的 unique_port 和 Cargo-lints。
- 修复无锁 WSV：
  - 删除 API 中不必要的 Dashmap 和锁定
  - 修复了创建块数量过多的错误（未记录被拒绝的交易）
  - 显示错误的完整错误原因
- 添加遥测订阅者。
- 角色和权限查询。
- 将块从 kura 移动到 wsv。
- 更改为 wsv 内的无锁数据结构。
- 网络超时修复。
- 修复健康端点。
- 介绍角色。
- 添加来自 dev 分支的推送 docker 镜像。
- 添加更积极的 linting 并消除代码中的恐慌。
- 指令执行特征的返工。
- 从 iroha_config 中删除旧代码。
- IR-1060 添加对所有现有权限的授予检查。
- 修复 iroha_network 的 ulimit 和超时。
- Ci 超时测试修复。
- 当定义被删除时，删除所有资产。
- 修复添加资产时的 wsv 恐慌。
- 删除通道的 Arc 和 Rwlock。
- Iroha 网络修复。
- 权限验证器在检查中使用引用。
- 授予指令。
- 添加了字符串长度限制的配置以及 NewAccount、Domain 和 AssetDefinition IR-1036 的 ID 验证。
- 用跟踪库替换日志。
- 添加 ci 检查文档并拒绝 dbg 宏。
- 引入可授予的权限。
- 添加 iroha_config 箱。
- 添加@alerdenisov 作为代码所有者以批准所有传入的合并请求。
- 修复了共识期间交易大小检查的问题。
- 恢复异步标准的升级。
- 用 2 IR-1035 的幂替换一些常量。
- 添加查询以检索交易历史记录 IR-1024。- 添加存储权限验证和权限验证器重组。
- 添加NewAccount用于帐户注册。
- 添加资产定义类型。
- 引入可配置的元数据限制。
- 引入交易元数据。
- 在查询中添加表达式。
- 添加 lints.toml 并修复警告。
- 将 trust_peers 与 config.json 分开。
- 修复 Telegram 中 Iroha 2 社区 URL 中的拼写错误。
- 修复剪辑警告。
- 引入了对帐户的键值元数据支持。
- 添加块的版本控制。
- 修复 ci linting 重复。
- 添加 mul、div、mod、raise_to 表达式。
- 添加 into_v* 用于版本控制。
- 用错误宏替换 Error::msg。
- 重写 iroha_http_server 并修改 torii 错误。
 - 将 Norito 版本升级到 2。
- 白皮书版本控制描述。
- 可靠的分页。修复由于错误而可能不需要分页的情况，而不是返回空集合。
- 为枚举添加导出（错误）。
- 修复夜间版本。
- 添加 iroha_error 箱。
- 版本化消息。
- 引入容器版本控制原语。
- 修复基准。
- 添加分页。
- 添加 Varint 编码解码。
- 将查询时间戳更改为 u128。
- 为管道事件添加 RejectionReason 枚举。
- 从创世文件中删除过时的行。在之前的提交中，目标已从寄存器 ISI 中删除。
- 简化注册和取消注册 ISI。
- 修复提交超时未在 4 对等网络中发送的问题。
- 更改视图时的拓扑随机播放。
- 为 FromVariant 派生宏添加其他容器。
- 添加对客户端 cli 的 MST 支持。
- 添加 FromVariant 宏和清理代码库。
- 将 i1i1 添加到代码所有者。
- 八卦交易。
- 添加指令和表达式的长度。
- 添加文档以阻止时间和提交时间参数。
- 用 TryFrom 替换了验证和接受特征。
- 引入仅等待最少数量的对等点。
- 添加 github 操作以使用 iroha2-java 测试 api。
- 添加 docker-compose-single.yml 的起源。
- 帐户的默认签名检查条件。
- 添加对具有多个签名者的帐户的测试。
- 添加对 MST 的客户端 API 支持。
- 在码头工人中构建。
- 将创世添加到 docker compose。
- 引入条件 MST。
- 添加 wait_for_active_peers 实现。
- 在 iroha_http_server 中添加 isahc 客户端测试。
- 客户端 API 规范。
- 表达式中的查询执行。
- 集成表达式和 ISI。
- ISI 的表达式。
- 修复帐户配置基准。
- 为客户端添加帐户配置。
- 修复 `submit_blocking`。
- 发送管道事件。
- Iroha 客户端 Web 套接字连接。
- 管道事件和数据事件的事件分离。
- 权限集成测试。
- 添加对burn 和mint 的权限检查。
- 取消注册 ISI 权限。
- 修复世界结构 PR 的基准。
- 引入世界结构。
- 实现创世块加载组件。
- 介绍创世账户。
- 引入权限验证器构建器。
- 使用 Github Actions 将标签添加到 Iroha2 PR。
- 引入权限框架。
- 队列 tx tx 数量限制和 Iroha 初始化修复。
- 将哈希包装在结构中。
- 提高日志级别：
  - 将信息级别日志添加到共识中。
  - 将网络通信日志标记为跟踪级别。
  - 从 WSV 中删除块向量，因为它是重复的，并且它在日志中显示了所有区块链。
  - 将信息日志级别设置为默认值。
- 删除可变的 WSV 引用以进行验证。
- 海姆版本增量。
- 将默认可信对等点添加到配置中。
- 客户端 API 迁移到 http。
- 将传输 isi 添加到 CLI。
- Iroha 对等相关指令的配置。
- 实施缺失的 ISI 执行方法和测试。
- URL查询参数解析
- 添加 `HttpResponse::ok()`、`HttpResponse::upgrade_required(..)`
- 使用 Iroha DSL 方法替换旧的指令和查询模型。
- 添加 BLS 签名支持。
- 引入 http 服务器箱。
- 使用符号链接修补了 libssl.so.1.0.0。
- 验证交易的帐户签名。
- 重构事务阶段。
- 初始域改进。
- 实现 DSL 原型。
- 改进 Torii 基准：禁用基准中的日志记录，添加成功率断言。
- 改进测试覆盖率管道：用 `grcov` 替换 `tarpaulin`，将测试覆盖率报告发布到 `codecov.io`。
- 修复 RTD 主题。
- iroha 子项目的交付工件。
- 介绍 `SignedQueryRequest`。
- 修复签名验证的错误。
- 回滚事务支持。
- 将生成的密钥对打印为 json。
- 支持 `Secp256k1` 密钥对。
- 初步支持不同的加密算法。
- 去中心化交易所功能。
- 用 cli 参数替换硬编码的配置路径。
- 工作台主工作流程修复。
- Docker 事件连接测试。
- Iroha 监视器指南和 CLI。
- 事件 CLI 改进。
- 事件过滤器。
- 事件连接。
- 修复主工作流程。
- iroha2 的RTD。
- 用于区块交易的 Merkle 树根哈希。
- 发布到 docker hub。
- 用于维护连接的 CLI 功能。
- 用于维护连接的 CLI 功能。
- Eprintln 记录宏。
- 日志改进。
- IR-802 订阅块状态更改。
- 交易和区块的事件发送。
- 将 Sumeragi 消息处理移至消息实现中。
- 通用连接机制。
- 为非标准客户端提取 Iroha 域实体。
- 交易 TTL。
- 每个块配置的最大交易量。
- 存储无效的块哈希值。
- 批量同步区块。
- 连接功能的配置。
- 连接到 Iroha 功能。
- 块验证更正。
- 块同步：图表。
- 连接到 Iroha 功能。
- 桥接：删除客户端。
- 块同步。
- 添加对等 ISI。
- 命令到指令重命名。
- 简单的指标端点。
- Bridge：获取注册的桥梁和外部资产。
- Docker 在管道中编写测试。
- 票数不足 Sumeragi 测试。
- 块链。
- 桥：手动外部传输处理。
- 简单的维护端点。
- 迁移到 serde-json。
- 消灭三军情报局。
- 添加桥接客户端、AddSignatory ISI 和 CanAddSignatory 权限。
- Sumeragi：b 组中的同级相关 TODO 修复。
- 在登录 Sumeragi 之前验证区块。
- 桥接外部资产。
- Sumeragi 消息中的签名验证。
- 二进制资产商店。
- 将 PublicKey 别名替换为类型。
- 准备用于发布的板条箱。
- NetworkTopology 内的最低投票逻辑。
- TransactionReceipt 验证重构。
- OnWorldStateViewChange 触发更改：IrohaQuery 而不是指令。
- 将网络拓扑中的构造与初始化分开。
- 添加与 Iroha 事件相关的 Iroha 特别说明。
- 块创建超时处理。
- 术语表和如何添加 Iroha 模块文档。
- 用原始 Iroha 模型替换硬编码桥模型。
- 引入 NetworkTopology 结构。
- 通过指令转换添加权限实体。
- Sumeragi 消息模块中的消息。
- Kura 的创世块功能。
- 添加 Iroha 包的自述文件。
- 桥接和寄存器桥接 ISI。
- 与 Iroha 的初始工作改变了听众。
- 将权限检查注入 OOB ISI。
- Docker 多个对等点修复。
- 点对点 Docker 示例。
- 交易收据处理。
- Iroha 权限。
- Dex 模块和 Bridges 板条箱。
- 修复与多个同行的资产创建的集成测试。
- 将资产模型重新实施到 EC-S- 中。
- 提交超时处理。
- 块头。
- 域实体的 ISI 相关方法。
- Kura 模式枚举和可信对等配置。
- 文档检查规则。
- 添加CommitedBlock。
- 将 kura 与 `sumeragi` 解耦。
- 在创建区块之前检查交易是否不为空。
- 重新实施 Iroha 特殊说明。
- 交易和区块转换的基准。
- 交易生命周期和状态重新设计。
- 块生命周期和状态。
- 修复验证错误，`sumeragi` 循环周期与 block_build_time_ms 配置参数同步。
- Sumeragi 算法封装在 `sumeragi` 模块内。
- 通过通道实现 Iroha 网络箱的模拟模块。
- 迁移到 async-std API。
- 网络模拟功能。
- 异步相关代码清理。
- 事务处理循环中的性能优化。
- 密钥对的生成是从 Iroha 开始提取的。
- Docker Iroha 可执行文件的打包。
- 介绍Sumeragi基本场景。
- Iroha CLI 客户端。
- 替补组执行后，伊洛哈掉落。
- 集成 `sumeragi`。
- 将 `sort_peers` 实现更改为使用先前块哈希作为种子的 rand shuffle。
- 删除对等模块中的消息包装器。
- 将网络相关信息封装在`torii::uri`和`iroha_network`内。
- 添加实施的对等指令，而不是硬编码处理。
- 通过受信任的同行列表进行同行通信。
- Torii 内部网络请求处理的封装。
- 将加密逻辑封装在加密模块内。- 使用时间戳和前一个块哈希作为有效负载的块符号。
- 加密功能放置在模块顶部，并与封装到签名中的 ursa 签名者一起使用。
- Sumeragi 初始。
- 在提交存储之前验证世界状态视图克隆上的事务指令。
- 验证交易接受时的签名。
- 修复请求反序列化中的错误。
- Iroha 签名的实现。
- 删除区块链实体以清理代码库。
- 事务 API 的更改：更好地创建和处理请求。
- 修复会创建具有空交易向量的块的错误
- 转发待处理的交易。
 - 修复 u128 Norito 编码 TCP 数据包中丢失字节的错误。
- 用于方法跟踪的属性宏。
- P2p 模块。
- 在torii和客户端中使用iroha_network。
- 添加新的 ISI 信息。
- 网络状态的特定类型别名。
- Box<dyn Error> 替换为字符串。
- 网络监听状态。
- 交易的初始验证逻辑。
- Iroha_network 箱子。
- 派生 Io、IntoContract 和 IntoQuery 特征的宏。
- Iroha-客户端的查询实现。
- 将命令转变为三军情报局合同。
- 添加条件多重签名的建议设计。
- 迁移到 Cargo 工作区。
- 模块迁移。
- 通过环境变量进行外部配置。
- Torii 的获取和放置请求处理。
- Github ci 修正。
- Cargo-make 在测试后清理块。
- 引入 `test_helper_fns` 模块，具有用块清理目录的功能。
- 通过默克尔树实施验证。
- 删除未使用的派生。
- 传播异步/等待并修复未等待的 `wsv::put`。
- 使用 `futures` 箱中的连接。
- 实现并行存储执行：写入磁盘和更新 WSV 并行发生。
- 使用引用而不是所有权进行（反）序列化。
- 从文件中弹出代码。
- 使用 ursa::blake2。
- 贡献指南中有关 mod.rs 的规则。
- 哈希 32 字节。
- Blake2 哈希。
- 磁盘接受对块的引用。
- 重构命令模块和初始 Merkle 树。
- 重构模块结构。
- 正确的格式。
- 将文档注释添加到 read_all。
- 实现`read_all`，重新组织存储测试，并将异步函数测试转变为异步测试。
- 删除不必要的可变捕获。
- 审查问题，修复剪辑。
- 删除破折号。
- 添加格式检查。
- 添加令牌。
- 为 github 操作创建 rust.yml。
- 引入磁盘存储原型。
- 转移资产测试和功能。
- 将默认初始化程序添加到结构中。
- 更改 MSTCache 结构的名称。
- 添加忘记借用。
- iroha2 代码的初始轮廓。
- 初始 Kura API。
- 添加一些基本文件，并发布概述 iroha v2 愿景的白皮书初稿。
- 基本 iroha v2 分支。

## [1.5.0] - 2022-04-08

### CI/CD 更改
- 删除 Jenkinsfile 和 JenkinsCI。

### 添加

- 为 Burrow 添加 RocksDB 存储实现。
- 使用布隆过滤器引入流量优化
- 将 `MST` 模块网络更新为位于 `batches_cache` 中的 `OS` 模块中。
- 提出流量优化建议。

### 文档

- 修复构建。添加数据库差异、迁移实践、健康检查端点、有关 iroha-swarm 工具的信息。

### 其他

- 文档构建的要求修复。
- 修剪发布文档以突出剩余的关键后续项目。
- 修复“检查 docker 映像是否存在”/build all Skip_testing。
- /构建所有skip_testing。
- /build 跳过测试；还有更多文档。
- 添加 `.github/_README.md`。
- 删除 `.packer`。
- 删除测试参数的更改。
- 使用新参数跳过测试阶段。
- 添加到工作流程。
- 删除存储库调度。
- 添加存储库调度。
- 为测试人员添加参数。
- 删除 `proposal_delay` 超时。

## [1.4.0] - 2022-01-31

### 添加

- 添加同步节点状态
- 添加 RocksDB 指标
- 通过 http 和指标添加健康检查接口。

### 修复

- 修复 Iroha v1.4-rc.2 中的列族
- 在 Iroha v1.4-rc.1 中添加 10 位布隆过滤器

### 文档

- 将 zip 和 pkg-config 添加到构建依赖列表。
- 更新自述文件：修复构建状态、构建指南等的损坏链接。
- 修复配置和 Docker 指标。

### 其他

- 更新 GHA docker 标签。
- 修复使用 g++11 编译时的 Iroha 1 编译错误。
- 将 `max_rounds_delay` 替换为 `proposal_creation_timeout`。
- 更新示例配置文件以删除旧的数据库连接参数。