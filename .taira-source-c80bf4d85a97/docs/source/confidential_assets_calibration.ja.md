<!-- Japanese translation of docs/source/confidential_assets_calibration.md -->

---
lang: ja
direction: ltr
source: docs/source/confidential_assets_calibration.md
status: complete
translator: manual
---

# 機密資産ガスキャリブレーション基準

この台帳は機密機能のガスキャリブレーションベンチマークで検証済みの結果を記録します。各行は `docs/source/confidential_assets.md#calibration-baselines--acceptance-gates` に記載の手順で取得したリリース品質の測定値です。

| Date (UTC) | Commit | Profile | `ns/op` | `gas/op` | `ns/gas` | Notes |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 | baseline-neon | 2.93e5 | 1.57e2 | 1.87e3 | Darwin 25.0.0 arm64e（`hostinfo`）; `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 | pending | baseline-simd-neutral | — | — | — | CI ホスト `bench-x86-neon0` で予定されている x86_64 ニュートラル計測。チケット GAS-214 参照。ベンチウィンドウ完了後に結果を追記予定（プレリリース 2.1 のチェックリスト項目）。 |
| 2026-04-13 | pending | baseline-avx2 | — | — | — | ニュートラル計測と同じコミット／ビルドを用いた AVX2 追跡キャリブレーション。ホスト `bench-x86-avx2a` が必要。GAS-214 で両方のランを `baseline-neon` とのデルタ比較込みで管理。 |

`ns/op` は Criterion による命令当たり中央値、`gas/op` は `iroha_core::gas::meter_instruction` におけるスケジュールコストの算術平均、`ns/gas` は 9 命令サンプルセットでの総ナノ秒を総ガスで割った値です。

*メモ:* 現在の arm64 ホストでは Criterion の `raw.csv` が標準で出力されません。リリースタグ前に `CRITERION_OUTPUT_TO=csv` を設定するか upstream 修正を適用し、受け入れチェックリストに必要な成果物を添付してください。`--save-baseline` 実行後も `target/criterion/` が生成されない場合、Linux ホストで収集するか、当面はコンソール出力をリリースバンドルへ保存してください。最新ランの arm64 コンソールログは `docs/source/confidential_assets_calibration_neon_20251018.log` にあります。

同ランの命令別中央値（`cargo bench -p iroha_core --bench isi_gas_calibration`）:

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

スケジュール列は `gas::tests::calibration_bench_gas_snapshot`（合計 1,413 gas）で検証されており、将来メータリングを変更する場合はキャリブレーションフィクスチャの更新が必須です。
