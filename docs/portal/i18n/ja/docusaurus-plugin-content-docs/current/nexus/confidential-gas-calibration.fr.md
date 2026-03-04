---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: ガス機密登録簿
説明: ガス機密情報の緊急措置。
スラグ: /nexus/confidential-gas-calibration
---

# ガス機密情報の基本校正

ガス機密の校正ベンチマークの有効性を確認するために訴訟を登録します。 Chaque ligne documente un jeu de mesures de qualite release capture avec laprocedrite dans [機密資産および ZK 譲渡](./confidential-assets#calibration-baselines--acceptance-gates)。

|日付 (UTC) |コミット |プロフィール | `ns/op` | `gas/op` | `ns/gas` |メモ |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 |ベースライン-ネオン | 2.93e5 | 1.57e2 | 1.87e3 |ダーウィン 25.0.0 arm64e (ホスト情報); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |保留中 |ベースライン-simd-ニュートラル | - | - | - |実行中立 x86_64 計画 sur l'hote CI `bench-x86-neon0`;ヴォワールチケット GAS-214。 Les resultats seront ajoutes une fois la fenetre de bench terminee (リリース 2.1 のマージ前のチェックリスト)。 |
| 2026-04-13 |保留中 |ベースライン-avx2 | - | - | - | AVX2 のキャリブレーションは、コミット/ビルドの実行中性を管理するユーティリティです。 `bench-x86-avx2a` が必要です。 GAS-214 クーブル レ ドゥは、アベック比較デルタ コントル `baseline-neon` を実行します。 |

`ns/op` 基準に従って指示された中央値の壁時計を評価します。 `gas/op` `iroha_core::gas::meter_instruction` の通信スケジュールの計算を開始します。 `ns/gas` は、ガス ソンム シュール アンサンブル ド ヌフの指示に従ってナノ秒ソムを分割します。

*注意* L'hote arm64 actuel ne produit pas les はデフォルトの基準 `raw.csv` を再開します。 relancez avec `CRITERION_OUTPUT_TO=csv` を修正するには、上流の事前予約を解除し、アーティファクトのリリースにはチェックリストが必要です。受け入れソエントは添付します。 `--save-baseline` 以降、`target/criterion/` はアンコールを実行し、Linux を起動して実行をキャプチャし、出撃コンソールでシリアル化し、一時停止のバンドルをリリースします。 `docs/source/confidential_assets_calibration_neon_20251018.log` での実行設定の参照、ログ コンソール arm64 のタイトル。

メディアンパー命令の実行 (`cargo bench -p iroha_core --bench isi_gas_calibration`):

|指示 |中央値 `ns/op` |スケジュール `gas` | `ns/gas` |
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

コロンヌのスケジュールは、`gas::tests::calibration_bench_gas_snapshot` (合計 1,413 個のガス シュール ランサンブル ド ニューフ命令) と、1 時間ごとのフィクスチャのキャリブレーションを含まない修正計量の修正を要求します。