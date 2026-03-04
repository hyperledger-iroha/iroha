---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: リブロ市長のガス機密校正
説明: 医療機関は秘密情報を公開します。
スラグ: /nexus/confidential-gas-calibration
---

# ガス機密の校正ベースのライン

ガス機密の校正ベンチマークの検証結果を登録します。 [機密資産および ZK 譲渡](./confidential-assets#calibration-baselines--acceptance-gates) の手順説​​明をリリースするためのドキュメントを作成します。

|フェチャ (協定世界時) |コミット |パーフィル | `ns/op` | `gas/op` | `ns/gas` |メモ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 |ベースライン-ネオン | 2.93e5 | 1.57e2 | 1.87e3 |ダーウィン 25.0.0 arm64e (ホスト情報); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |保留中 |ベースライン-simd-ニュートラル | - | - | - |ホスト CI `bench-x86-neon0` での x86_64 プログラムの排出中立。 verチケットGAS-214。ベンチの最終結果を確認する必要があります (リリース 2.1 のマージ前のチェックリスト)。 |
| 2026-04-13 |保留中 |ベースライン-avx2 | - | - | - | Calibracion AVX2 後部の usando el missmo コミット/ビルド que la corrida ニュートラル。ホスト `bench-x86-avx2a` が必要です。 GAS-214 デルタ コントラ `baseline-neon` を比較します。 |

`ns/op` 基準に関するメディアの指示を集約します。 `gas/op` `iroha_core::gas::meter_instruction` の通信スケジュールのコストを管理するメディアです。 `ns/gas` ロス ナノセグンドス スマドス エントリ ガス スマドと新しい命令を結合します。

*注記* ホスト arm64 の実際の出力が再開されない `raw.csv` の欠陥基準。 `CRITERION_OUTPUT_TO=csv` を参照して、アップストリームの手順を修正し、付属品の承認チェックリストをリリースする必要があります。 `target/criterion/` は、`--save-baseline` の不正なデータを収集し、ホスト Linux のシリアル化とコンソールのバンドルのリリースと一時停止を確認します。参照情報は、`docs/source/confidential_assets_calibration_neon_20251018.log` での究極の生存権を示すコンソール アーム 64 です。

不正行為の指示に関するメディア (`cargo bench -p iroha_core --bench isi_gas_calibration`):

|説明 |メディアナ `ns/op` |スケジュール `gas` | `ns/gas` |
| --- | --- | --- | --- |
|ドメインの登録 | 3.46e5 | 200 | 1.73e3 |
|アカウント登録 | 3.15e5 | 200 | 1.58e3 |
|登録資産定義 | 3.41e5 | 200 | 1.71e3 |
| SetAccountKV_small | 3.28e5 | 67 | 4.90e3 |
|アカウントロールを付与 | 3.33e5 | 96 | 3.47e3 |
|アカウントロールを取り消す | 3.12e5 | 96 | 3.25e3 |
|トリガー_空の引数を実行する | 1.42e5 | 224 | 6.33e2 |
|ミントアセット | 1.56e5 | 150 | 1.04e3 |
|資産の譲渡 | 3.68e5 | 180 | 2.04e3 |

`gas::tests::calibration_bench_gas_snapshot` でスケジュールを設定するための列 (合計 1,413 個のガスと新しい接続命令) は、未来のカンビア エル メーターを測定し、実際のフィクスチャーとキャリブレーションを確認します。