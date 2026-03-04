---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/confidential-gas-calibration.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: bde1236c24270aafef53488c6c2b81454ed978ea551f9eb0fe98fd8e1b238608
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: 機密ガス校正レジャー
description: 機密ガススケジュールを裏付けるリリース品質の測定値。
slug: /nexus/confidential-gas-calibration
---

# 機密ガス校正ベースライン

このレジャーは機密ガス校正ベンチマークの検証済み出力を追跡します。各行は、[Confidential Assets & ZK Transfers](./confidential-assets#calibration-baselines--acceptance-gates) に記載された手順で取得したリリース品質の測定セットを記録します。

| 日付 (UTC) | Commit | プロファイル | `ns/op` | `gas/op` | `ns/gas` | 注記 |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e (hostinfo); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | - | - | - | CIホスト `bench-x86-neon0` で x86_64 ニュートラル実行を予定; チケット GAS-214 を参照。ベンチ窓が完了次第結果を追加 (pre-merge チェックリストは release 2.1 を対象)。 |
| 2026-04-13 | pending | baseline-avx2 | - | - | - | ニュートラル実行と同一 commit/build で AVX2 校正を追跡; ホスト `bench-x86-avx2a` が必要。GAS-214 は両実行を `baseline-neon` とのデルタ比較で管理。 |

`ns/op` は Criterion が測定した命令あたりのウォールクロック中央値の集計、`gas/op` は `iroha_core::gas::meter_instruction` の対応するスケジュールコストの算術平均、`ns/gas` は9命令セットの合計ナノ秒を合計ガスで割った値です。

*注記.* 現在の arm64 ホストは Criterion の `raw.csv` サマリをデフォルトで出力しません。リリースにタグを付ける前に `CRITERION_OUTPUT_TO=csv` を指定して再実行するか upstream 修正を適用し、受け入れチェックリストで必要なアーティファクトを添付してください。`--save-baseline` 後も `target/criterion/` が見つからない場合は、Linux ホストで実行するかコンソール出力をリリースバンドルにシリアライズして一時的な stopgap としてください。参考として、最新の arm64 実行のコンソールログは `docs/source/confidential_assets_calibration_neon_20251018.log` にあります。

同一実行の命令別中央値 (`cargo bench -p iroha_core --bench isi_gas_calibration`):

| Instruction | median `ns/op` | schedule `gas` | `ns/gas` |
| --- | --- | --- | --- |
| RegisterDomain | 3.46e5 | 200 | 1.73e3 |
| RegisterAccount | 3.15e5 | 200 | 1.58e3 |
| RegisterAssetDef | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
| GrantAccountRole | 3.33e5 | 96 | 3.47e3 |
| RevokeAccountRole | 3.12e5 | 96 | 3.25e3 |
| ExecuteTrigger_empty_args | 1.42e5 | 224 | 6.33e2 |
| MintAsset | 1.56e5 | 150 | 1.04e3 |
| TransferAsset | 3.68e5 | 180 | 2.04e3 |

スケジュール列は `gas::tests::calibration_bench_gas_snapshot` (9命令セット合計 1,413 gas) で強制されており、将来のパッチがメータリングを変更しても校正フィクスチャを更新しなければ失敗します。
