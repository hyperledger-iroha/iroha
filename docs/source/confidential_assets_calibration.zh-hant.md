---
lang: zh-hant
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 01bfdc70f601098acaefc60c6a3b4c464218b8c6f01f2f20eb3632994ff7110f
source_last_modified: "2025-12-29T18:16:35.932211+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# 機密氣體校準基線

該分類賬跟踪機密氣體校準的驗證輸出
基準。每行記錄了使用捕獲的發布質量測量集
`docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` 中描述的過程。

|日期 (UTC) |提交 |簡介 | `ns/op` | `gas/op` | `ns/gas` |筆記|
| ---| ---| ---| ---| ---| ---| ---|
| 2025年10月18日 | 3c70a7d3 | 3c70a7d3基線霓虹燈| 2.93e5 | 2.93e5 1.57e2 | 1.57e2 1.87e3 | 1.87e3達爾文 25.0.0 arm64e（主機信息）； `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`； `cargo test -p iroha_core bench_repro -- --ignored`； `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`； `rustc 1.88.0 (6b00bc3)` |
| 2026-04-28 | 8ea9b2a7 | 8ea9b2a7基線-neon-20260428 | 4.29e6 | 4.29e6 1.57e2 | 1.57e2 2.73e4 | 2.73e4達爾文 25.0.0 arm64 (`rustc 1.91.0`)。命令：`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`；登錄 `docs/source/confidential_assets_calibration_neon_20260428.log`。 x86_64 奇偶校驗運行（SIMD 中性 + AVX2）計劃於 2026 年 3 月 19 日蘇黎世實驗室時段進行；文物將與匹配的命令一起降落在 `artifacts/confidential_assets_calibration/2026-03-x86/` 下，並且一旦捕獲就會合併到基線表中。 |
| 2026-04-28 | — |基線 simd 中性 | — | — | — | **在 Apple Silicon 上被放棄** - `ring` 為平台 ABI 強制實施 NEON，因此 `RUSTFLAGS="-C target-feature=-neon"` 在工作台運行之前會失敗 (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`)。中性數據在 CI 主機 `bench-x86-neon0` 上保持門控狀態。 |
| 2026-04-28 | — |基線 avx2 | — | — | — | **推遲**直到 x86_64 運行程序可用。 `arch -x86_64` 無法在此計算機上生成二進製文件（“可執行文件中的 CPU 類型錯誤”；請參閱 `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`）。 CI 主機 `bench-x86-avx2a` 仍然是記錄源。 |

`ns/op` 聚合 Criterion 測量的每條指令的中位掛鐘；
`gas/op` 是相應調度成本的算術平均值
`iroha_core::gas::meter_instruction`； `ns/gas` 將納秒總和除以
九個指令樣本集的氣體總和。

*注意。 * 當前的arm64主機不會發出標準`raw.csv`摘要
盒子；在標記之前使用 `CRITERION_OUTPUT_TO=csv` 或上游修復程序重新運行
發布，以便附上驗收清單所需的工件。
如果 `--save-baseline` 之後仍然缺少 `target/criterion/`，則收集運行
在 Linux 主機上或將控制台輸出序列化到發行包中作為
暫時的權宜之計。作為參考，最新運行的arm64控制台日誌
住在 `docs/source/confidential_assets_calibration_neon_20251018.log`。

同一運行中的每條指令中位數 (`cargo bench -p iroha_core --bench isi_gas_calibration`)：

|說明 |中位數 `ns/op` |時間表 `gas` | `ns/gas` |
| ---| ---| ---| ---|
|註冊域名 | 3.46e5 | 3.46e5 200 | 200 1.73e3 | 1.73e3
|註冊帳號 | 3.15e5 | 3.15e5 200 | 200 1.58e3 | 1.58e3
|註冊資產定義| 3.41e5 | 3.41e5 200 | 200 1.71e3 | 1.71e3
|設置AccountKV_small | 3.28e5 | 3.28e5 67 | 67 4.90e3 | 4.90e3
|授予帳戶角色 | 3.33e5 | 3.33e5 96 | 96 3.47e3 | 3.47e3
|撤銷帳戶角色 | 3.12e5 | 3.12e5 96 | 96 3.25e3 | 3.25e3
|執行觸發器_空_參數| 1.42e5 | 1.42e5 224 | 224 6.33e2 | 6.33e2
|薄荷資產 | 1.56e5 | 1.56e5 150 | 150 1.04e3 | 1.04e3
|轉讓資產 | 3.68e5 | 3.68e5 180 | 180 2.04e3 | 2.04e3

### 2026-04-28（Apple Silicon，啟用 NEON）

2026 年 4 月 28 日刷新的平均延遲 (`cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=10 --warm-up-time=2 --noplot --save-baseline baseline-neon-20260428`)：|說明 |中位數 `ns/op` |時間表 `gas` | `ns/gas` |
| ---| ---| ---| ---|
|註冊域名 | 8.58e6 | 8.58e6 200 | 200 4.29e4 | 4.29e4
|註冊帳號 | 4.40e6 | 4.40e6 200 | 200 2.20e4 | 2.20e4
|註冊資產定義| 4.23e6 | 4.23e6 200 | 200 2.12e4 | 2.12e4
|設置AccountKV_small | 3.79e6 | 3.79e6 67 | 67 5.66e4 | 5.66e4
|授予帳戶角色 | 3.60e6 | 3.60e6 96 | 96 3.75e4 | 3.75e4
|撤銷帳戶角色 | 3.76e6 | 3.76e6 96 | 96 3.92e4 | 3.92e4
|執行觸發器_空_參數| 2.71e6 | 2.71e6 224 | 224 1.21e4 | 1.21e4
|薄荷資產 | 3.92e6 | 3.92e6 150 | 150 2.61e4 | 2.61e4
|轉讓資產 | 3.59e6 | 3.59e6 180 | 180 1.99e4 | 1.99e4

上表中的 `ns/op` 和 `ns/gas` 聚合源自以下總和
這些中位數（九個指令集的總 `3.85717e7`ns 和 1,413
氣體單位）。

時間表列由 `gas::tests::calibration_bench_gas_snapshot` 強制執行
（九個指令集中總共 1,413 個 Gas）並且如果將來的補丁出現問題將會跳閘
無需更新校準裝置即可更改計量。

## 承諾樹遙測證據 (M2.2)

根據路線圖任務 **M2.2**，每次校準運行都必須捕獲新的
承諾樹指標和驅逐計數器來證明 Merkle 邊界保持不變
在配置的範圍內：

- `iroha_confidential_tree_commitments{asset_id}`
- `iroha_confidential_tree_depth{asset_id}`
- `iroha_confidential_root_history_entries{asset_id}`
- `iroha_confidential_frontier_checkpoints{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_height{asset_id}`
- `iroha_confidential_frontier_last_checkpoint_commitments{asset_id}`
- `iroha_confidential_root_evictions_total{asset_id}`
- `iroha_confidential_frontier_evictions_total{asset_id}`
- `iroha_zk_verifier_cache_events_total{cache,event}`

記錄校準工作負載之前和之後的值。一個
每個資產單個命令就足夠了； `xor#wonderland` 的示例：

```bash
curl -s http://127.0.0.1:8180/metrics \
  | rg 'iroha_confidential_(tree_(commitments|depth)|root_history_entries|frontier_(checkpoints|last_checkpoint_height|last_checkpoint_commitments)|root_evictions_total|frontier_evictions_total){asset_id="xor#wonderland"}'
```

將原始輸出（或 Prometheus 快照）附加到校準票，以便
治理審核者可以確認根歷史上限和檢查點間隔
很榮幸。 `docs/source/telemetry.md#confidential-tree-telemetry-m22` 中的遙測指南
擴展了警報期望和相關的 Grafana 面板。

在同一個抓取中包含驗證者緩存計數器，以便審閱者可以確認
未命中率保持在 40% 警告閾值以下：

```bash
curl -s http://127.0.0.1:8180/metrics \\
  | rg 'iroha_zk_verifier_cache_events_total{cache="vk",event="(hit|miss)"}'
```

在校準記錄中記錄導出的比率 (`miss / (hit + miss)`)
展示 SIMD 中性成本建模練習重用熱緩存而不是
破壞 Halo2 驗證程序註冊表。

## 中立和 AVX2 豁免

SDK 委員會授予 PhaseC 門的臨時豁免，要求
`baseline-simd-neutral` 和 `baseline-avx2` 測量：

- **SIMD 中性：** 在 Apple Silicon 上，`ring` 加密後端強制使用 NEON
  ABI 正確性。禁用該功能 (`RUSTFLAGS="-C target-feature=-neon"`)
  在生成基準二進製文件之前中止構建 (`docs/source/confidential_assets_calibration_simd_neutral_attempt_20260428.log`)。
- **AVX2:** 本地工具鏈無法生成 x86_64 二進製文件 (`arch -x86_64 rustc -V`
  →“可執行文件中的 CPU 類型錯誤”；看到
  `docs/source/confidential_assets_calibration_avx2_attempt_20260428.log`）。

直到 CI 主機 `bench-x86-neon0` 和 `bench-x86-avx2a` 上線，NEON 才會運行
上述加上遙測證據滿足 PhaseC 驗收標準。
該豁免記錄在 `status.md` 中，一旦 x86 硬件發布，將重新審視該豁免。
可用。