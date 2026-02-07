---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: سجل معايرة الغاز السري
説明: 重要な情報。
スラグ: /nexus/confidential-gas-calibration
---

# خطوط اساس لمعايرة الغاز السري

あなたのことを忘れないでください。 كل صف يوثق مجموعة قياسات بجودة اصدار تم التقاطها وفق الاجراء الموضح في [機密資産 & ZK]転送](./confidential-assets#calibration-baselines--acceptance-gates)。

|世界時間 (UTC) |コミット | और देखें `ns/op` | `gas/op` | `ns/gas` | और देखें
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 |ベースライン-ネオン | 2.93e5 | 1.57e2 | 1.87e3 |ダーウィン 25.0.0 arm64e (ホスト情報); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |保留中 |ベースライン-simd-ニュートラル | - | - | - | تشغيل محايد x86_64 مجدول على مضيف CI `bench-x86-neon0`; GAS-214 です。ベンチ (マージ前のバージョン 2.1)。 |
| 2026-04-13 |保留中 |ベースライン-avx2 | - | - | - | AVX2 のコミット/ビルドの実行`bench-x86-avx2a` です。 تغطي GAS-214 التشغيلين مع مقارنة الفروقات مقابل `baseline-neon`. |

`ns/op` 評価基準`gas/op` هو المتوسط الحسابي لكلف الجدول المقابلة من `iroha_core::gas::meter_instruction`; و`ns/gas` يقسم مجموع النانوثانية على مجموع الغاز عبر مجموعة التعليمات التسع.

*ملاحظة.* المضيف arm64 الحالي لا يصدر ملخصات Criterion `raw.csv` بشكل افتراضي؛上流の `CRITERION_OUTPUT_TO=csv` 上流のアーティファクトを確認してください。ああ。 `target/criterion/` セキュリティ `--save-baseline` セキュリティ Linux セキュリティ重要な問題は、次のとおりです。 arm64 は `docs/source/confidential_assets_calibration_neon_20251018.log` です。

回答 (`cargo bench -p iroha_core --bench isi_gas_calibration`):

|ログイン | ログイン`ns/op` | `gas` | `ns/gas` |
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

عمود الجدول مفروض بواسطة `gas::tests::calibration_bench_gas_snapshot` (اجمالي 1,413 غاز عبر مجموعة التعليمات التسع) وسيفشلフィクスチャーを確認してください。