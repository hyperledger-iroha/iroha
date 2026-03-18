---
lang: zh-hant
direction: ltr
source: docs/source/compute_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 624ca40bb09d616d2820a7229022507b73dc3c0692f7eb83f5169aee32a64c4f
source_last_modified: "2025-12-29T18:16:35.929771+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 計算通道 (SSC-1)

計算通道接受確定性 HTTP 樣式調用，將它們映射到 Kotodama
入口點以及記錄計量/收據以進行計費和治理審查。
該 RFC 凍結了清單模式、呼叫/收據信封、沙箱護欄、
以及第一個版本的默認配置。

## 清單

- 架構：`crates/iroha_data_model/src/compute/mod.rs` (`ComputeManifest` /
  `ComputeRoute`）。
- `abi_version` 固定到 `1`；不同版本的清單被拒絕
  驗證期間。
- 每條路線聲明：
  - `id` (`service`, `method`)
  - `entrypoint`（Kotodama 入口點名稱）
  - 編解碼器白名單 (`codecs`)
  - TTL/氣體/請求/響應上限（`ttl_slots`、`gas_budget`、`max_*_bytes`）
  - 確定性/執行類（`determinism`、`execution_class`）
  - SoraFS 入口/模型描述符（`input_limits`，可選 `model`）
  - 定價系列 (`price_family`) + 資源配置文件 (`resource_profile`)
  - 身份驗證策略 (`auth`)
- 沙盒護欄位於清單 `sandbox` 塊中，並由所有人共享
  路由（模式/隨機性/存儲和非確定性系統調用拒絕）。

示例：`fixtures/compute/manifest_compute_payments.json`。

## 呼叫、請求和收據

- 架構：`ComputeRequest`、`ComputeCall`、`ComputeCallSummary`、`ComputeReceipt`、
  `ComputeMetering`、`ComputeOutcome` 中
  `crates/iroha_data_model/src/compute/mod.rs`。
- `ComputeRequest::hash()` 生成規範的請求哈希（標頭保留
  在確定性 `BTreeMap` 中，有效負載作為 `payload_hash` 進行攜帶）。
- `ComputeCall` 捕獲命名空間/路由、編解碼器、TTL/gas/響應上限，
  資源配置文件 + 價格系列、身份驗證（`Public` 或 UAID 綁定
  `ComputeAuthn`)、確定性（`Strict` 與 `BestEffort`）、執行類
  提示（CPU/GPU/TEE），聲明 SoraFS 輸入字節/塊，可選贊助商
  預算和規範請求信封。請求哈希用於
  重放保護和路由。
- 路線可能嵌入可選的 SoraFS 型號參考和輸入限制
  （內聯/塊蓋）；清單沙盒規則門控 GPU/TEE 提示。
- `ComputePriceWeights::charge_units` 將計量數據轉換為計費計算
  通過循環和出口字節上的 ceil-division 單位。
- `ComputeOutcome` 報告 `Success`、`Timeout`、`OutOfMemory`、
  `BudgetExhausted` 或 `InternalError` 並可選擇包含響應哈希/
  用於審核的大小/編解碼器。

示例：
- 致電：`fixtures/compute/call_compute_payments.json`
- 收據：`fixtures/compute/receipt_compute_payments.json`

## 沙箱和資源配置文件- `ComputeSandboxRules`默認將執行模式鎖定為`IvmOnly`，
  從請求哈希中種子確定性隨機性，允許只讀 SoraFS
  訪問，並拒絕非確定性系統調用。 GPU/TEE 提示由以下方式控制
  `allow_gpu_hints`/`allow_tee_hints` 以保持執行的確定性。
- `ComputeResourceBudget` 設置每個配置文件的周期、線性內存、堆棧上限
  大小、IO 預算和出口，以及 GPU 提示和 WASI-lite 幫助程序的切換。
- 默認情況下提供兩個配置文件（`cpu-small`、`cpu-balanced`）
  `defaults::compute::resource_profiles` 具有確定性回退。

## 定價和計費單位

- 價格系列 (`ComputePriceWeights`) 將周期和出口字節映射到計算中
  單位；默認收費 `ceil(cycles/1_000_000) + ceil(egress_bytes/1024)`
  `unit_label = "cu"`。家庭在清單中由 `price_family` 鍵入，
  入院時強制執行。
- 計量記錄包含 `charged_units` 以及原始週期/入口/出口/持續時間
  核對總計。費用因執行級別而放大
  決定論乘數 (`ComputePriceAmplifiers`) 並上限為
  `compute.economics.max_cu_per_call`；出口被限制
  `compute.economics.max_amplification_ratio` 結合響應放大。
- 贊助商預算（`ComputeCall::sponsor_budget_cu`）是針對
  每次通話/每日上限；計費單位不得超過贊助商聲明的預算。
- 治理價格更新使用風險類別界限
  `compute.economics.price_bounds` 和記錄在中的基線家族
  `compute.economics.price_family_baseline`；使用
  `ComputeEconomics::apply_price_update` 在更新之前驗證增量
  活躍的家庭地圖。 Torii 配置更新使用
  `ConfigUpdate::ComputePricing`，kiso 將其應用到相同的邊界
  保持治理編輯的確定性。

## 配置

新的計算配置位於 `crates/iroha_config/src/parameters` 中：

- 用戶視圖：`Compute` (`user.rs`)，環境覆蓋：
  - `COMPUTE_ENABLED`（默認`false`）
  - `COMPUTE_DEFAULT_TTL_SLOTS` / `COMPUTE_MAX_TTL_SLOTS`
  - `COMPUTE_MAX_REQUEST_BYTES` / `COMPUTE_MAX_RESPONSE_BYTES`
  - `COMPUTE_MAX_GAS_PER_CALL`
  - `COMPUTE_DEFAULT_RESOURCE_PROFILE` / `COMPUTE_DEFAULT_PRICE_FAMILY`
  - `COMPUTE_AUTH_POLICY`
- 定價/經濟：`compute.economics` 捕獲
  `max_cu_per_call`/`max_amplification_ratio`，費用分攤，贊助商上限
  （每次調用和每日 CU）、價格系列基線 + 風險類別/界限
  治理更新和執行級乘數（GPU/TEE/盡力而為）。
- 實際/默認：`actual.rs` / `defaults.rs::compute` 公開解析
  `Compute` 設置（命名空間、配置文件、價格系列、沙箱）。
- 無效的配置（空命名空間、默認配置文件/系列缺失、TTL 上限
  反轉）在解析期間顯示為 `InvalidComputeConfig`。

## 測試和裝置

- 確定性助手（`request_hash`，定價）和夾具往返實時
  `crates/iroha_data_model/src/compute/mod.rs`（參見 `fixtures_round_trip`，
  `request_hash_is_stable`、`pricing_rounds_up_units`）。
- JSON 裝置存在於 `fixtures/compute/` 中並由數據模型執行
  回歸覆蓋率測試。

## SLO 工具和預算- `compute.slo.*` 配置公開網關 SLO 旋鈕（飛行隊列
  深度、RPS 上限和延遲目標）
  `crates/iroha_config/src/parameters/{user,actual,defaults}.rs`。默認值：32
  飛行中，每條航線 512 人排隊，200 RPS，p50 25ms，p95 75ms，p99 120ms。
- 運行輕量級工作台工具來捕獲 SLO 摘要和請求/出口
  快照：`cargo run -p xtask --bincompute_gateway --bench [manifest_path]
  [迭代] [並發] [out_dir]` (defaults: `fixtures/compute/manifest_compute_ payments.json`,
  128 次迭代，並發數 16，輸出低於
  `artifacts/compute_gateway/bench_summary.{json,md}`）。長凳使用
  確定性有效負載 (`fixtures/compute/payload_compute_payments.json`) 和
  每個請求標頭以避免鍛煉時重播衝突
  `echo`/`uppercase`/`sha3` 入口點。

## SDK/CLI 奇偶校驗裝置

- 規範裝置位於 `fixtures/compute/` 下：清單、調用、有效負載和
  網關式響應/收據佈局。有效負載哈希必須與調用匹配
  `request.payload_hash`；輔助有效負載位於
  `fixtures/compute/payload_compute_payments.json`。
- CLI 附帶 `iroha compute simulate` 和 `iroha compute invoke`：

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
  `javascript/iroha_js/src/compute.js` 進行回歸測試
  `javascript/iroha_js/test/computeExamples.test.js`。
- Swift：`ComputeSimulator` 加載相同的裝置，驗證有效負載哈希值，
  並通過測試模擬入口點
  `IrohaSwift/Tests/IrohaSwiftTests/ComputeSimulatorTests.swift`。
- CLI/JS/Swift 助手都共享相同的 Norito 裝置，因此 SDK 可以
  離線驗證請求構造和哈希處理，無需點擊
  運行網關。