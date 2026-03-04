---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/confidential-gas-calibration.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
タイトル: خفیہ گیس کیلیبریشن لیجر
説明: ریلیز معیار کی پیمائشیں جو خفیہ گیس شیڈول کی پشت پناہی کرتی ہیں۔
スラグ: /nexus/confidential-gas-calibration
---

# خفیہ گیس کیلیبریشن بیس لائنز

یہ لیجر خفیہ گیس کیلیبریشن بینچ مارکس کے تصدیق شدہ نتائج ٹریک کرتا ہے۔ [機密資産と ZK の譲渡](./confidential-assets#calibration-baselines--acceptance-gates) میں بیان کردہ طریقہ کار سے حاصل کیا گیا تھا۔

|時間 (UTC) |コミット | और देखें `ns/op` | `gas/op` | `ns/gas` |にゅう |
| --- | --- | --- | --- | --- | --- | --- |
| 2025-10-18 | 3c70a7d3 |ベースライン-ネオン | 2.93e5 | 1.57e2 | 1.87e3 |ダーウィン 25.0.0 arm64e (ホスト情報); `cargo bench -p iroha_core --bench isi_gas_calibration -- --sample-size=200 --warm-up-time=5 --save-baseline neon-20251018`; `cargo test -p iroha_core bench_repro -- --ignored`; `cargo bench -p ivm --bench gas_calibration -- --sample-size=200 --warm-up-time=5`; `rustc 1.88.0 (6b00bc3)` |
| 2026-04-12 |保留中 |ベースライン-simd-ニュートラル | - | - | - | CI ホスト `bench-x86-neon0` پر x86_64 نیوٹرل رن شیڈیول ہے؛ ٹکٹ GAS-214 ๑یکھیں۔ベンチ ونڈو مکمل ہونے پر شامل ہوں گے (マージ前 چیک لسٹ ریلیز 2.1 کو ہدف بناتی ہے)۔ |
| 2026-04-13 |保留中 |ベースライン-avx2 | - | - | - |コミット/ビルド AVX2 のコミット/ビルドホスト `bench-x86-avx2a` درکار ہے۔ GAS-214 دونوں رنز کو `baseline-neon` کے مقابلے ڈیلٹا کمپیریزن کے ساتھ کور کرتا ہے۔ |

`ns/op` 基準 کے ذریعے ماپا گیا فی انسٹرکشن وال کلاک میڈین 集計 کرتا ہے؛ `gas/op` `iroha_core::gas::meter_instruction` کے متعلقہ شیڈول اخراجات کا حسابی اوسط ہے؛ `ns/gas` نو انسٹرکشن سیٹ کے مجموعی نینو سیکنڈز کو مجموعی گیس سے تقسیم کرتا ہے۔

*نوٹ.* موجودہ arm64 ホスト ڈیفالٹ طور پر 基準 `raw.csv` خلاصے 放射する نہیں کرتا؛承認チェックリスト 承認チェックリストアーティファクトの世界`target/criterion/` `--save-baseline` بعد بھی غائب ہو تو Linux ホスト پر رن جمع کریں یا کنسول آؤٹ پٹ کوバンドル シリアル化 シリアル化 サポートحوالہ کے طور پر، تازہ ترین رن کا arm64 کنسول لاگ `docs/source/confidential_assets_calibration_neon_20251018.log` میں موجود ہے۔

特別な日 (`cargo bench -p iroha_core --bench isi_gas_calibration`):

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

شیڈول کالم `gas::tests::calibration_bench_gas_snapshot` کے ذریعے نافذ ہوتا ہے (نو انسٹرکشن سیٹ میں کل 1,413 گیس) اور الرر پیچز میٹرنگ کو بدل دیں مگر کیلیبریشن فکسچرز اپ ڈیٹ نہ ہوں تو ٹیسٹ فیل فو جائے گا۔