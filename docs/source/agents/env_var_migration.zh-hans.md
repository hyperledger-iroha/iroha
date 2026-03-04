---
lang: zh-hans
direction: ltr
source: docs/source/agents/env_var_migration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c9ce6010594e495116c1397b984000d1ee5d45d064294eca046f8dc762fa73b6
source_last_modified: "2026-01-05T09:28:11.999442+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 环境 → 配置迁移跟踪器

该跟踪器总结了出现的面向生产的环境变量切换
通过 `docs/source/agents/env_var_inventory.{json,md}` 和预期的迁移
`iroha_config` 的路径（或显式仅限开发/测试范围）。


注意：当新的 **生产** 环境时，`ci/check_env_config_surface.sh` 现在会失败
垫片相对于 `AGENTS_BASE_REF` 出现，除非 `ENV_CONFIG_GUARD_ALLOW=1` 是
设置；在使用覆盖之前，在此处记录有意添加的内容。

## 已完成迁移- **IVM ABI 选择退出** — 删除了 `IVM_ALLOW_NON_V1_ABI`；编译器现在拒绝
  非 v1 ABI 无条件地通过单元测试来保护错误路径。
- **IVM 调试横幅环境垫片** — 删除了 `IVM_SUPPRESS_BANNER` 环境选择退出；
  横幅抑制仍然可以通过编程设置器实现。
- **IVM 缓存/大小调整** — 线程缓存/验证器/GPU 大小调整
  `iroha_config` (`pipeline.{cache_size,ivm_cache_max_decoded_ops,ivm_cache_max_bytes,ivm_prover_threads}`,
  `accel.max_gpus`）并删除了运行时环境垫片。主持人现在致电
  `ivm::ivm_cache::configure_limits` 和 `ivm::zk::set_prover_threads`，测试使用
  `CacheLimitsGuard` 而不是 env 覆盖。
- **连接队列根** — 添加了 `connect.queue.root`（默认值：
  `~/.iroha/connect`）到客户端配置并通过 CLI 进行线程化，
  JS 诊断。 JS 帮助程序解析配置（或显式 `rootDir`）并
  仅通过 `allowEnvOverride` 在开发/测试中尊重 `IROHA_CONNECT_QUEUE_ROOT`；
  模板记录了旋钮，因此操作员不再需要环境覆盖。
- **Izanami 网络选择加入** — 添加了显式 `allow_net` CLI/config 标志
  Izanami 混沌工具；现在运行需要 `allow_net=true`/`--allow-net` 和
- **IVM 横幅蜂鸣声** — 将 `IROHA_BEEP` env shim 替换为配置驱动
  `ivm.banner.{show,beep}` 切换（默认值：true/true）。启动横幅/蜂鸣声
  接线现在仅在生产中读取配置；开发/测试版本仍然很荣幸
  手动切换的环境覆盖。
- **DA 假脱机覆盖（仅测试）** — `IROHA_DA_SPOOL_DIR` 覆盖现在
  被 `cfg(test)` 助手围起来；生产代码始终来源线轴
  配置中的路径。
- **加密内在函数** — 已替换 `IROHA_DISABLE_SM_INTRINSICS` /
  `IROHA_ENABLE_SM_INTRINSICS` 带配置驱动
  `crypto.sm_intrinsics` 策略 (`auto`/`force-enable`/`force-disable`) 和
  移除了 `IROHA_SM_OPENSSL_PREVIEW` 防护装置。主机应用该策略的位置为
  启动、工作台/测试可以通过 `CRYPTO_SM_INTRINSICS` 和 OpenSSL 选择加入
  预览现在仅尊重配置标志。
  Izanami 已经需要 `--allow-net`/persisted 配置，测试现在依赖于
  该旋钮而不是环境环境切换。
- **FastPQ GPU 调整** — 添加了 `fastpq.metal.{max_in_flight,threadgroup_width,metal_trace,metal_debug_enum,metal_debug_fused}`
  配置旋钮（默认值：`None`/`None`/`false`/`false`/`false`）并通过 CLI 解析将它们线程化
  `FASTPQ_METAL_*` / `FASTPQ_DEBUG_*` 垫片现在充当开发/测试后备，并且
  配置加载后将被忽略（即使配置未设置它们）；文档/库存是
  刷新以标记迁移。【crates/irohad/src/main.rs:2609】【crates/iroha_core/src/fastpq/lane.rs:109】【crates/fastpq_prover/src/overrides.rs:11】
  （`IVM_DECODE_TRACE`、`IVM_DEBUG_WSV`、`IVM_DEBUG_COMPACT`、`IVM_DEBUG_INVALID`、
  `IVM_DEBUG_REGALLOC`、`IVM_DEBUG_METAL_ENUM`、`IVM_DEBUG_METAL_SELFTEST`、
  `IVM_FORCE_METAL_ENUM`、`IVM_FORCE_METAL_SELFTEST_FAIL`、`IVM_FORCE_CUDA_SELFTEST_FAIL`、
  `IVM_DISABLE_METAL`、`IVM_DISABLE_CUDA`）现在通过共享的调试/测试版本进行门控
  帮助器，以便生产二进制文件忽略它们，同时保留用于本地诊断的旋钮。环境
  重新生成了库存以反映仅限开发/测试的范围。- **FASTPQ 夹具更新** — `FASTPQ_UPDATE_FIXTURES` 现在仅出现在 FASTPQ 集成中
  测试；生产源不再读取环境切换，库存仅反映测试
  范围。
- **清单刷新 + 范围检测** — env 库存工具现在将 `build.rs` 文件标记为
  构建范围并跟踪 `#[cfg(test)]`/集成线束模块，以便仅测试切换（例如，
  `IROHA_TEST_*`、`IROHA_RUN_IGNORED`）和 CUDA 构建标志显示在生产计数之外。
  2025 年 12 月 7 日重新生成了清单（518 个引用/144 个变量），以保持环境配置保护差异为绿色。
- **P2P 拓扑环境 shim 释放防护** — `IROHA_P2P_TOPOLOGY_UPDATE_MS` 现在触发确定性
  发布版本中的启动错误（仅在调试/测试中发出警告），因此生产节点仅依赖于
  `network.peer_gossip_period_ms`。重新生成了环境库存以反映守卫和
  更新后的分类器现在将 `cfg!` 保护的切换范围限定为调试/测试。

## 高优先级迁移（生产路径）

- _无（使用 cfg!/debug 检测刷新清单；P2P shim 强化后 env-config 保护绿色）。_

## 仅开发/测试切换到围栏

- 当前扫描（2025 年 12 月 7 日）：仅构建 CUDA 标志 (`IVM_CUDA_*`) 的范围为 `build` 和
  线束开关（`IROHA_TEST_*`、`IROHA_RUN_IGNORED`、`IROHA_SKIP_BIND_CHECKS`）现已注册为
  库存中的 `test`/`debug`（包括 `cfg!` 保护垫片）。不需要额外的围栏；
  当垫片是临时的时，将未来添加的内容保留在带有 TODO 标记的 `cfg(test)`/仅限工作台的帮助程序后面。

## 构建时环境（保持原样）

- 货物/功能环境（`CARGO_*`、`OUT_DIR`、`DOCS_RS`、`PROFILE`、`CUDA_HOME`、
  `CUDA_PATH`、`JSONSTAGE1_CUDA_ARCH`、`FASTPQ_SKIP_GPU_BUILD` 等）保留
  构建脚本问题超出了运行时配置迁移的范围。

## 下一步行动

1) 在配置表面更新后运行 `make check-env-config-surface` 以捕获新的生产环境垫片
   及早分配子系统所有者/ETA。  
2）每次扫描后刷新库存（`make check-env-config-surface`）
   跟踪器与新的护栏保持对齐，并且环境配置防护差异保持无噪音。