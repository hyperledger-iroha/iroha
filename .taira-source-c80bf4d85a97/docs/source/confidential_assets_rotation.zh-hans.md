---
lang: zh-hans
direction: ltr
source: docs/source/confidential_assets_rotation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fd1e43316c492cc96ed107f6318841ad8db160735d4698c4f05562ff6127fda9
source_last_modified: "2026-01-22T14:35:37.492932+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//！ `roadmap.md:M3` 引用的机密资产轮换手册。

# 机密资产轮换操作手册

本手册解释了操作员如何安排和执行机密资产
轮换（参数集、验证密钥和策略转换），同时
确保钱包、Torii 客户端和内存池守卫保持确定性。

## 生命周期和状态

机密参数集（`PoseidonParams`、`PedersenParams`、验证密钥）
用于导出给定高度的有效状态的格子和助手住在
`crates/iroha_core/src/state.rs:7540`–`7561`。运行时助手清除待处理的
一旦达到目标高度就进行转换并记录稍后的失败
重播 (`crates/iroha_core/src/state.rs:6725`–`6765`)。

资产政策嵌入
`pending_transition { transition_id, new_mode, effective_height, conversion_window }`
因此治理可以通过以下方式安排升级
`ScheduleConfidentialPolicyTransition` 并根据需要取消它们。参见
`crates/iroha_data_model/src/asset/definition.rs:320` 和 Torii DTO 镜像
（`crates/iroha_torii/src/routing.rs:1539`–`1580`）。

## 轮换工作流程

1. **发布新的参数包。** 运营商提交
   `PublishPedersenParams`/`PublishPoseidonParams` 指令（CLI
   `iroha app zk params publish ...`）使用元数据上演新的发电机组，
   激活/弃用窗口和状态标记。执行人拒绝
   重复的 ID、不增加的版本或每个错误的状态转换
   `crates/iroha_core/src/smartcontracts/isi/world.rs:2499`–`2635`，以及
   注册表测试涵盖故障模式 (`crates/iroha_core/tests/confidential_params_registry.rs:93`–`226`)。
2. **注册/验证密钥更新。** `RegisterVerifyingKey` 强制后端，
   密钥可以进入之前的承诺和电路/版本约束
   注册表（`crates/iroha_core/src/smartcontracts/isi/world.rs:2067`–`2137`）。
   更新密钥会自动弃用旧条目并擦除内联字节，
   由 `crates/iroha_core/tests/zk_vk_deprecate_marks_status.rs:1` 行使。
3. **安排资产政策转换。** 新参数 ID 生效后，
   治理调用 `ScheduleConfidentialPolicyTransition` 并提供所需的信息
   模式、转换窗口和审核哈希。执行人拒绝冲突
   具有出色的透明供应的过渡或资产。测试如
   `crates/iroha_core/tests/confidential_policy_gates.rs:300`–`384` 验证
   中止转换清除 `pending_transition`，同时
   `confidential_policy_transition_reaches_shielded_only_on_schedule` 在
   第 385-433 行确认预定的升级翻转到 `ShieldedOnly` 恰好在
   有效高度。
4. **策略应用和mempool守卫。** 区块执行器清除所有待处理的区块
   在每个块的开始处转换（`apply_policy_if_due`）并发出
   如果转换失败，则进行遥测，以便操作员可以重新安排。入院期间
   内存池拒绝有效政策会在区块中改变的交易，
   确保整个过渡窗口的确定性包含
   （`docs/source/confidential_assets.md:60`）。

## 钱包和 SDK 要求- Swift 和其他移动 SDK 公开 Torii 帮助程序来获取活动策略
  加上任何待处理的转换，因此钱包可以在签名之前警告用户。参见
  `IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:309` (DTO) 和相关的
  在 `IrohaSwift/Tests/IrohaSwiftTests/ToriiClientTests.swift:591` 进行测试。
- CLI 通过 `iroha ledger assets data-policy get`（帮助程序）镜像相同的元数据
  `crates/iroha_cli/src/main.rs:1497`–`1670`)，使操作员能够审核
  策略/参数 ID 连接到资产定义中，无需深入探究
  块存储。

## 测试和遥测覆盖范围

- `crates/iroha_core/tests/zk_ledger_scaffold.rs:288`–`345` 验证该策略
  转换传播到元数据快照并在应用后清除。
- `crates/iroha_core/tests/zk_dedup.rs:1`证明`Preverify`缓存
  拒绝双花/双证明，包括轮换场景
  承诺不同。
- `crates/iroha_core/tests/zk_confidential_events.rs` 和
  `zk_shield_transfer_audit.rs` 覆盖端到端屏蔽→传输→取消屏蔽
  流，确保审计跟踪在参数轮换中得以保留。
- `dashboards/grafana/confidential_assets.json` 和
  `docs/source/confidential_assets.md:401` 记录承诺树 &
  伴随每次校准/旋转运行的验证器缓存仪表。

## 运行手册所有权

- **DevRel / Wallet SDK Leads：** 维护 SDK 片段 + 显示的快速入门
  如何显示待处理的过渡并重放薄荷 → 传输 → 显示
  本地测试（在 `docs/source/project_tracker/confidential_assets_phase_c.md:M3.2` 下跟踪）。
- **计划管理/机密资产 TL：** 批准过渡请求，保留
  `status.md` 更新了即将到来的轮换，并确保豁免（如果有）
  与校准分类账一起记录。