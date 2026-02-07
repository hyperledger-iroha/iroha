---
slug: /nexus/confidential-gas-calibration
lang: zh-hant
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Confidential Gas Calibration Ledger
description: Release-quality measurements backing the confidential gas schedule.
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# 機密氣體校準基線

該分類賬跟踪機密氣體校準的驗證輸出
基準。每行記錄了使用捕獲的發布質量測量集
[機密資產和 ZK 轉移](./confidential-assets#calibration-baselines--acceptance-gates) 中描述的過程。

|日期 (UTC) |提交 |簡介 | `ns/op` | `gas/op` | `ns/gas` |筆記|
| ---| ---| ---| ---| ---| ---| ---|
| 2025年10月18日 | 3c70a7d3 | 3c70a7d3基線霓虹燈| 2.93e5 | 2.93e5 1.57e2 | 1.57e2 1.87e3 | 1.87e3達爾文 25.0.0 arm64e（主機信息）； `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`； `cargo test -p iroha_core bench_repro -- --ignored`； `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`； `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |待定 |基線 simd 中性 | — | — | — |計劃在 CI 主機 `bench-x86-neon0` 上運行 x86_64 中立；請參閱票 GAS-214。工作台窗口完成後將添加結果（預合併清單目標版本 2.1）。 |
| 2026-04-13 |待定 |基線 avx2 | — | — | — |使用與中性運行相同的提交/構建進行後續 AVX2 校準；需要主機 `bench-x86-avx2a`。 GAS-214 涵蓋了與 `baseline-neon` 進行增量比較的兩次運行。 |

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

時間表列由 `gas::tests::calibration_bench_gas_snapshot` 強制執行
（九個指令集中總共 1,413 個 Gas）並且如果將來的補丁出現問題將會跳閘
無需更新校準裝置即可更改計量。