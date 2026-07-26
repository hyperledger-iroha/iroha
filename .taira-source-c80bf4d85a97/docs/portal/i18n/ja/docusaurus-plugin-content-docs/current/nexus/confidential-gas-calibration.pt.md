---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: ガス機密情報の校正
説明: 医療機関は、ガス機密情報の維持またはクロノグラムを解除します。
スラグ: /nexus/confidential-gas-calibration
---

# ガス機密の基準値の校正

ガス機密情報のベンチマークの検証結果を登録します。 [機密資産と ZK の譲渡](./confidential-assets#calibration-baselines--acceptance-gates) の手順説​​明を取得するための、医療の適格性を確認するためのドキュメントを作成します。

|データ (UTC) |コミット |パーフィル | `ns/op` | `gas/op` | `ns/gas` |メモ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 |ベースライン-ネオン | 2.93e5 | 1.57e2 | 1.87e3 |ダーウィン 25.0.0 arm64e (ホスト情報); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |保留中 |ベースライン-simd-ニュートラル | - | - | - |ホスト CI `bench-x86-neon0` なしでニュートラ x86_64 プログラムを実行します。 verチケットGAS-214。 Os resultados serao adicionados quando a janela de bench terminar (ミラオリリース 2.1 のマージ前のチェックリスト)。 |
| 2026-04-13 |保留中 |ベースライン-avx2 | - | - | - | Calibracao AVX2 は、コンパニオンを使用してコミット/ビルドを実行するニュートラルです。ホスト `bench-x86-avx2a` を要求します。 GAS-214 は、デルタ コントラ `baseline-neon` の比較を実行するためのコマンドです。 |

`ns/op` メディア ペロの基準を示す壁時計の集合体。 `gas/op` `iroha_core::gas::meter_instruction` のスケジュール担当者はメディア管理者です。 `ns/gas` 分割された OS ナノセグンドス ソマドス ペロ ガス ソマドは、新しい命令を結合しません。

*注記* O ホスト arm64 は実際に resumos `raw.csv` を発行し、Criterion por Padrao を実行します。 com `CRITERION_OUTPUT_TO=csv` を使用して、上流のセキュリティ チェックリストをリリースし、最新のセキュリティ情報を確認してください。 `target/criterion/` は、`--save-baseline` を使用してホスト Linux を実行し、一時的にリリースされるバンドルなしでコンソールをシリアル化します。参照、コンソール arm64 のログ、`docs/source/confidential_assets_calibration_neon_20251018.log` の究極の実行。

管理者による実行指示 (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

`gas::tests::calibration_bench_gas_snapshot` のスケジュール (合計 1,413 個のガス、新規の指示がありません) は、校正用の機器の測定や調整に使用されるファルハー セ パッチを取得します。