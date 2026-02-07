---
lang: ur
direction: rtl
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

٪ Iroha 3 SLO کنٹرول

Iroha 3 ریلیز لائن میں اہم Nexus راستوں کے لئے واضح SLOS اٹھایا گیا ہے:

- فائنلٹی سلاٹ دورانیہ (NX - 18 کیڈینس)
- پروف کی توثیق (سرٹیفکیٹ ، جے ڈی جی کی تصدیق ، برج ثبوت)
- پروف اینڈپوائنٹ ہینڈلنگ (توثیق میں تاخیر کے ذریعے ایکسم پاتھ پراکسی)
- فیس اور اسٹیکنگ راستے (ادائیگی کرنے والے/کفیل اور بانڈ/سلیش فلوز)

## بجٹ

بجٹ `benchmarks/i3/slo_budgets.json` میں رہتے ہیں اور براہ راست بینچ کا نقشہ بنائیں
I3 سویٹ میں منظرنامے۔ مقاصد فی - کال P99 اہداف ہیں:

- فیس/اسٹیکنگ: 50 ملی میٹر فی کال (`fee_payer` ، `fee_sponsor` ، `staking_bond` ، `staking_slash`)
- سٹرٹ / جے ڈی جی / پل کی تصدیق کریں: 80ms (`commit_cert_verify` ، `jdg_attestation_verify` ،
  `bridge_proof_verify`)
- سٹرٹ اسمبلی کا ارتکاب کریں: 80ms (`commit_cert_assembly`)
- رسائی شیڈولر: 50 ملی میٹر (`access_scheduler`)
- پروف اینڈپوائنٹ پراکسی: 120 ایم ایس (`torii_proof_endpoint`)

برن ریٹ کے اشارے (`burn_rate_fast`/`burn_rate_slow`) 14.4/6.0 کو انکوڈ کریں
پیجنگ بمقابلہ ٹکٹ الرٹس کے لئے ملٹی ونڈو تناسب۔

## کنٹرول

`cargo xtask i3-slo-harness` کے ذریعے کنٹرول چلائیں:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

نتائج:

- `bench_report.json|csv|md` - خام i3 بینچ سویٹ کے نتائج (گٹ ہیش + منظرنامے)
- `slo_report.json|md`- پاس/فیل/بجٹ تناسب کے ساتھ ہر ہدف کے ساتھ ایس ایل او تشخیص

کنٹرول بجٹ فائل کو کھاتا ہے اور `benchmarks/i3/slo_thresholds.json` کو نافذ کرتا ہے
بینچ کے دوران جب کوئی ہدف دباؤ ڈالتا ہے تو تیزی سے ناکام ہوجاتا ہے۔

## ٹیلی میٹری اور ڈیش بورڈز

- فائنلٹی: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- ثبوت کی توثیق: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

Grafana اسٹارٹر پینل `dashboards/grafana/i3_slo.json` میں رہتے ہیں۔ Prometheus
`dashboards/alerts/i3_slo_burn.yml` میں برن ریٹ کے انتباہات فراہم کیے گئے ہیں
بیکڈ (فائنلٹی 2s ، پروف کی تصدیق 80 ایم ایس ، پروف اینڈپوائنٹ پراکسی کی تصدیق کریں
120 ملی میٹر)۔

## آپریشنل نوٹ

- راتوں میں استعمال چلائیں۔ `artifacts/i3_slo/<stamp>/slo_report.md` شائع کریں
  گورننس شواہد کے لئے بینچ نوادرات کے ساتھ ساتھ۔
- اگر بجٹ ناکام ہوجاتا ہے تو ، منظر نامے کی نشاندہی کرنے کے لئے بینچ مارک ڈاون کا استعمال کریں ، پھر ڈرل کریں
  مماثل Grafana پینل/براہ راست میٹرکس سے وابستہ ہونے کے لئے الرٹ۔
- پروف اختتامی نقطہ SLOs ہر روٹ سے بچنے کے لئے تصدیق میں تاخیر کو پراکسی کے طور پر استعمال کرتے ہیں
  کارڈنلٹی بلو اپ ؛ بینچ مارک ہدف (120 ایم ایس) برقرار رکھنے/ڈاس سے میل کھاتا ہے
  پروف API پر محافظ۔