---
lang: zh-hans
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 计算通道 (SSC-1)

计算通道接受确定性 HTTP 样式调用，将它们映射到 Kotodama
入口点以及记录计量/收据以进行计费和治理审查。
该 RFC 冻结了清单模式、呼叫/收据信封、沙箱护栏、
以及第一个版本的默认配置。

## 清单

- 架构：`crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`）。
- `abi_version` 固定到 `1`；不同版本的清单被拒绝
  验证期间。
- 每条路线声明：
  - `id` (`service`, `method`)
  - `entrypoint`（Kotodama 入口点名称）
  - 编解码器白名单 (`codecs`)
  - TTL/气体/请求/响应上限（`ttl_slots`、`gas_budget`、`max_*_bytes`）
  - 确定性/执行类（`determinism`、`execution_class`）
  - SoraFS 入口/模型描述符（`input_limits`，可选 `model`）
  - 定价系列 (`price_family`) + 资源配置文件 (`resource_profile`)
  - 身份验证策略 (`auth`)
- 沙盒护栏位于清单 `sandbox` 块中，并由所有人共享
  路由（模式/随机性/存储和非确定性系统调用拒绝）。

示例：`fixtures/compute/manifest_compute_payments.json`。

## 呼叫、请求和收据

- 架构：`ComputeRequest`、`ComputeCall`、`ComputeCallSummary`、`ComputeReceipt`、
  `ComputeMetering`、`ComputeOutcome` 中
  `crates/iroha_data_model/src/compute/mod.rs`。
- `ComputeRequest::hash()` 生成规范的请求哈希（标头保留
  在确定性 `BTreeMap` 中，有效负载作为 `payload_hash` 进行携带）。
- `ComputeCall` 捕获命名空间/路由、编解码器、TTL/gas/响应上限，
  资源配置文件 + 价格系列、身份验证（`Public` 或 UAID 绑定
  `ComputeAuthn`)、确定性（`Strict` 与 `BestEffort`）、执行类
  提示（CPU/GPU/TEE），声明 SoraFS 输入字节/块，可选赞助商
  预算和规范请求信封。请求哈希用于
  重放保护和路由。
- 路线可能嵌入可选的 SoraFS 型号参考和输入限制
  （内联/块盖）；清单沙盒规则门控 GPU/TEE 提示。
- `ComputePriceWeights::charge_units` 将计量数据转换为计费计算
  通过循环和出口字节上的 ceil-division 单位。
- `ComputeOutcome` 报告 `Success`、`Timeout`、`OutOfMemory`、
  `BudgetExhausted` 或 `InternalError` 并可选择包含响应哈希/
  用于审核的大小/编解码器。

示例：
- 致电：`fixtures/compute/call_compute_payments.json`
- 收据：`fixtures/compute/receipt_compute_payments.json`

## 沙箱和资源配置文件- `ComputeSandboxRules`默认将执行模式锁定为`IvmOnly`，
  从请求哈希中种子确定性随机性，允许只读 SoraFS
  访问，并拒绝非确定性系统调用。 GPU/TEE 提示由以下方式控制
  `allow_gpu_hints`/`allow_tee_hints` 以保持执行的确定性。
- `ComputeResourceBudget` 设置每个配置文件的周期、线性内存、堆栈上限
  大小、IO 预算和出口，以及 GPU 提示和 WASI-lite 帮助程序的切换。
- 默认情况下提供两个配置文件（`cpu-small`、`cpu-balanced`）
  `defaults::compute::resource_profiles` 具有确定性回退。

## 定价和计费单位

- 价格系列 (`ComputePriceWeights`) 将周期和出口字节映射到计算中
  单位；默认收费 `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)`
  `unit_label = "cu"`。家庭在清单中由 `price_family` 键入，
  入院时强制执行。
- 计量记录包含 `charged_units` 以及原始周期/入口/出口/持续时间
  核对总计。费用因执行级别而放大
  决定论乘数 (`ComputePriceAmplifiers`) 并上限为
  `compute.economics.max_cu_per_call`；出口被限制
  `compute.economics.max_amplification_ratio` 结合响应放大。
- 赞助商预算（`ComputeCall::sponsor_budget_cu`）是针对
  每次通话/每日上限；计费单位不得超过赞助商声明的预算。
- 治理价格更新使用风险类别界限
  `compute.economics.price_bounds` 和记录在中的基线家族
  `compute.economics.price_family_baseline`；使用
  `ComputeEconomics::apply_price_update` 在更新之前验证增量
  活跃的家庭地图。 Torii 配置更新使用
  `ConfigUpdate::ComputePricing`，kiso 将其应用到相同的边界
  保持治理编辑的确定性。

## 配置

新的计算配置位于 `crates/iroha_config/src/parameters` 中：

- 用户视图：`Compute` (`user.rs`)，环境覆盖：
  - `COMPUTE_ENABLED`（默认`false`）
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- 定价/经济：`compute.economics` 捕获
  `max_cu_per_call`/`max_amplification_ratio`，费用分摊，赞助商上限
  （每次调用和每日 CU）、价格系列基线 + 风险类别/界限
  治理更新和执行级乘数（GPU/TEE/尽力而为）。
- 实际/默认：`actual.rs` / `defaults.rs::compute` 公开解析
  `Compute` 设置（命名空间、配置文件、价格系列、沙箱）。
- 无效的配置（空命名空间、默认配置文件/系列缺失、TTL 上限
  反转）在解析期间显示为 `InvalidComputeConfig`。

## 测试和装置

- 确定性助手（`request_hash`，定价）和夹具往返实时
  `crates/iroha_data_model/src/compute/mod.rs`（参见 `fixtures_round_trip`，
  `request_hash_is_stable`、`pricing_rounds_up_units`）。
- JSON 装置存在于 `fixtures/compute/` 中并由数据模型执行
  回归覆盖率测试。

## SLO 工具和预算- `compute.slo.*` 配置公开网关 SLO 旋钮（飞行队列
  深度、RPS 上限和延迟目标）
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`。默认值：32
  飞行中，每条航线 512 人排队，200 RPS，p50 25ms，p95 75ms，p99 120ms。
- 运行轻量级工作台工具来捕获 SLO 摘要和请求/出口
  快照：`cargo run -p xtask --bincompute_gateway --bench [manifest_path]
  [迭代] [并发] [out_dir]` (defaults: `fixtures/compute/manifest_compute_ payments.json`,
  128 次迭代，并发数 16，输出低于
  `artifacts/compute_gateway/bench_summary.{json,md}`）。长凳使用
  确定性有效负载 (`fixtures/compute/payload_compute_payments.json`) 和
  每个请求标头以避免锻炼时重播冲突
  `echo`/`uppercase`/`sha3` 入口点。

## SDK/CLI 奇偶校验装置

- 规范装置位于 `fixtures/compute/` 下：清单、调用、有效负载和
  网关式响应/收据布局。有效负载哈希必须与调用匹配
  `request.payload_hash`；辅助有效负载位于
  `fixtures/compute/payload_compute_payments.json`。
- CLI 附带 `iroha compute simulate` 和 `iroha compute invoke`：

```bash
iroha compute simulate \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json

iroha compute invoke \
  --endpoint http://127.0.0.1:8088 \
  --manifest fixtures/compute/manifest_compute_payments.json \
  --call fixtures/compute/call_compute_payments.json \
  --payload fixtures/compute/payload_compute_payments.json
```

- JS: `loadComputeFixtures`/`simulateCompute`/`buildGatewayRequest` 居住
  `javascript/iroha_js/src/compute.js` 进行回归测试
  `javascript/iroha_js/test/computeExamples.test.js`。
- Swift：`ComputeSimulator` 加载相同的装置，验证有效负载哈希值，
  并通过测试模拟入口点
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`。
- CLI/JS/Swift 助手都共享相同的 Norito 装置，因此 SDK 可以
  离线验证请求构造和哈希处理，无需点击
  运行网关。