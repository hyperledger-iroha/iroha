---
lang: zh-hans
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3c952f92e009ea4b2ccba55940737889e8e70f506d09899598bd913a2ac68d2d
source_last_modified: "2026-01-22T14:35:36.839904+00:00"
translation_last_reviewed: 2026-02-07
id: nexus-refactor-plan
title: Sora Nexus ledger refactor plan
description: Mirror of `docs/source/nexus_refactor_plan.md`, detailing the phased clean-up work for the Iroha 3 codebase.
translator: machine-google-reviewed
---

:::注意规范来源
此页面镜像 `docs/source/nexus_refactor_plan.md`。保持两个副本对齐，直到多语言版本登陆门户。
:::

# Sora Nexus 账本重构计划

本文档捕获了 Sora Nexus Ledger（“Iroha 3”）重构的直接路线图。它反映了当前存储库布局以及在 genesis/WSV 簿记、Sumeragi 共识、智能合约触发器、快照查询、指针 ABI 主机绑定和 Norito 编解码器中观察到的回归。目标是集中在一个连贯的、可测试的架构上，而不是试图将所有修复都放在一个单一的补丁中。

## 0. 指导原则
- 跨异构硬件保留确定性行为；仅通过具有相同回退功能的选择加入功能标志来利用加速。
- Norito 是序列化层。任何状态/模式更改都必须包括 Norito 编码/解码往返测试和夹具更新。
- 配置流经 `iroha_config`（用户 → 实际 → 默认值）。从生产路径中删除临时环境切换。
- ABI 政策仍然是 V1 并且不可协商。主机必须明确拒绝未知的指针类型/系统调用。
- `cargo test --workspace` 和黄金测试（`ivm`、`norito`、`integration_tests`）仍然是每个里程碑的基线门。

## 1. 存储库拓扑快照
- `crates/iroha_core`：Sumeragi 参与者、WSV、创世加载器、管道（查询、覆盖、zk 通道）、智能合约主机胶水。
- `crates/iroha_data_model`：链上数据和查询的权威模式。
- `crates/iroha`：CLI、测试、SDK 使用的客户端 API。
- `crates/iroha_cli`：操作员 CLI，当前镜像 `iroha` 中的众多 API。
- `crates/ivm`：Kotodama 字节码 VM、指针 ABI 主机集成入口点。
- `crates/norito`：带有 JSON 适配器和 AoS/NCB 后端的序列化编解码器。
- `integration_tests`：跨组件断言，涵盖创世/引导、Sumeragi、触发器、分页等。
- 文档已经概述了 Sora Nexus Ledger 目标（`nexus.md`、`new_pipeline.md`、`ivm.md`），但相对于代码而言，实现是支离破碎且部分陈旧的。

## 2. 重构支柱和里程碑

### A 阶段 – 基础和可观察性
1. **WSV 遥测 + 快照**
   - 在查询、Sumeragi 和 CLI 使用的 `state`（`WorldStateSnapshot` 特征）中建立规范快照 API。
   - 使用 `scripts/iroha_state_dump.sh` 通过 `iroha state dump --format norito` 生成确定性快照。
2. **创世/引导决定论**
   - 重构创世摄取以流经单个 Norito 支持的管道 (`iroha_core::genesis`)。
  - 添加集成/回归覆盖，重放创世加上第一个块，并在arm64/x86_64上断言相同的WSV根（在`integration_tests/tests/genesis_replay_determinism.rs`下跟踪）。
3. **跨板条箱固定性测试**
   - 展开 `integration_tests/tests/genesis_json.rs` 以验证一个工具中的 WSV、管道和 ABI 不变量。
  - 引入一个 `cargo xtask check-shape` 脚手架，该脚手架会因架构漂移而发生恐慌（在 DevEx 工具积压下跟踪；请参阅 `scripts/xtask/README.md` 操作项）。

### B 阶段 – WSV 和查询界面
1. **状态存储交易**
   - 将 `state/storage_transactions.rs` 折叠到事务适配器中，强制执行提交顺序和冲突检测。
   - 单元测试现在验证资产/世界/触发器修改在失败时回滚。
2. **查询模型重构**
   - 将分页/光标逻辑移至 `crates/iroha_core/src/query/` 下的可重用组件中。对齐 `iroha_data_model` 中的 Norito 表示。
  - 添加具有确定性排序的触发器、资产和角色的快照查询（通过 `crates/iroha_core/tests/snapshot_iterable.rs` 跟踪当前覆盖范围）。
3. **快照一致性**
   - 确保 `iroha ledger query` CLI 使用与 Sumeragi/fetchers 相同的快照路径。
   - CLI 快照回归测试在 `tests/cli/state_snapshot.rs` 下进行（针对缓慢运行的功能门控）。

### C 阶段 – Sumeragi 管道
1. **拓扑和纪元管理**
   - 将 `EpochRosterProvider` 提取到具有 WSV 权益快照支持的实现的特征中。
  - `WsvEpochRosterAdapter::from_peer_iter` 为工作台/测试提供了一个简单的模拟友好构造函数。
2. **共识流程简化**
   - 将 `crates/iroha_core/src/sumeragi/*` 重新组织为模块：`pacemaker`、`aggregation`、`availability`、`witness`，并在 `consensus` 下共享类型。
  - 用键入的 Norito 信封替换临时消息传递，并引入视图更改属性测试（在 Sumeragi 消息积压中跟踪）。
3. **车道/证明集成**
   - 将车道证明与 DA 承诺保持一致，并确保 RBC 门控是统一的。
   - 端到端集成测试 `integration_tests/tests/extra_functional/seven_peer_consistency.rs` 现在验证启用 RBC 的路径。

### D 阶段 – 智能合约和 Pointer-ABI 主机
1. **主机边界审计**
   - 合并指针类型检查 (`ivm::pointer_abi`) 和主机适配器 (`iroha_core::smartcontracts::ivm::host`)。
   - 指针表期望和主机清单绑定由 `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` 和 `ivm_host_mapping.rs` 涵盖，它们执行黄金 TLV 映射。
2. **触发执行沙箱**
   - 重构触发器以通过通用 `TriggerExecutor` 运行，该 `TriggerExecutor` 强制执行气体、指针验证和事件日志记录。
  - 添加覆盖故障路径的呼叫/时间触发器回归测试（通过 `crates/iroha_core/tests/trigger_failure.rs` 跟踪）。
3. **CLI 和客户端调整**
   - 确保 CLI 操作（`audit`、`gov`、`sumeragi`、`ivm`）依赖于共享的 `iroha` 客户端功能以避免漂移。
   - CLI JSON 快照测试位于 `tests/cli/json_snapshot.rs` 中；让它们保持最新，以便核心命令输出继续与规范的 JSON 参考相匹配。

### E 阶段 – Norito 编解码器强化
1. **架构注册表**
   - 在 `crates/norito/src/schema/` 下创建 Norito 架构注册表，以获取核心数据类型的规范编码。
   - 添加了验证示例有效负载编码的文档测试（`norito::schema::SamplePayload`）。
2. **黄金赛程刷新**
   - 一旦重构落地，更新 `crates/norito/tests/*` 黄金装置以匹配新的 WSV 模式。
   - `scripts/norito_regen.sh` 通过 `norito_regen_goldens` 帮助程序确定性地重新生成 Norito JSON 黄金。
3. **IVM/Norito 集成**
   - 通过 Norito 端到端验证 Kotodama 清单序列化，确保指针 ABI 元数据一致。
   - `crates/ivm/tests/manifest_roundtrip.rs` 保留清单的 Norito 编码/解码奇偶校验。

## 3. 跨领域关注点
- **测试策略**：每个阶段都会促进单元测试→板条箱测试→集成测试。失败的测试捕获当前的回归；新的测试阻止它们重新出现。
- **文档**：每个阶段落地后，更新 `status.md` 并将未完成的项目滚动到 `roadmap.md`，同时修剪已完成的任务。
- **性能基准**：维护 `iroha_core`、`ivm` 和 `norito` 中的现有基准；添加重构后的基线测量以验证没有回归。
- **功能标志**：仅为需要外部工具链的后端保留板条箱级切换（`cuda`、`zk-verify-batch`）。 CPU SIMD 路径始终在运行时构建和选择；为不受支持的硬件提供确定性标量回退。

## 4. 下一步立即行动
- A 阶段脚手架（快照特征 + 遥测接线）——查看路线图更新中的可操作任务。
- 最近对 `sumeragi`、`state` 和 `ivm` 的缺陷审计发现了以下亮点：
  - `sumeragi`：死代码津贴保护视图更改证明广播、VRF 重播状态和 EMA 遥测导出。这些将一直处于封闭状态，直到 C 阶段的共识流程简化和通道/证明集成交付成果落地。
  - `state`：`Cell` 清理和遥测路由移至 A 阶段 WSV 遥测轨道，而 SoA/并行应用注释并入 C 阶段管道优化待办事项中。
  - `ivm`：CUDA 切换曝光、包络验证和 Halo2/Metal 覆盖映射到 D 阶段主机边界工作以及横切 GPU 加速主题；内核保留在专用 GPU 待办事项上，直到准备就绪。
- 准备跨团队 RFC，总结该计划，以便在进行侵入性代码更改之前签署。

## 5. 开放性问题
- RBC 在 P1 之后是否应该保持可选状态，还是对于 Nexus 账本通道是强制的？需要利益相关者做出决定。
- 我们是否在 P1 中强制执行 DS 可组合性组，或者在车道证明成熟之前将其禁用？
- ML-DSA-87 参数的规范位置是什么？候选：新的 `crates/fastpq_isi` 箱子（待创建）。

---

_最后更新：2025-09-12_