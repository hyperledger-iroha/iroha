---
lang: ar
direction: rtl
source: docs/source/i3_slo_harness.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df3e3ac15baf47a6c53001acabcac7987a2386c2b772b1d8625eb60598f95a60
source_last_modified: "2026-01-03T18:08:01.691568+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

% Iroha 3 SLO

يحمل خط الإصدار Iroha 3 SLOs واضحة لمسارات Nexus الهامة:

- مدة الفتحة النهائية (إيقاع NX‑18)
- التحقق من الأدلة (شهادات الالتزام، شهادات JDG، إثباتات الجسر)
- التعامل مع نقطة النهاية (وكيل مسار أكسوم عبر زمن الوصول للتحقق)
- مسارات الرسوم والرهن العقاري (تدفقات الدافع/الراعي والسندات/القطع المائلة)

## الميزانيات

الميزانيات موجودة في `benchmarks/i3/slo_budgets.json` ويتم تعيينها مباشرةً على مقاعد البدلاء
السيناريوهات في مجموعة I3. الأهداف هي أهداف p99 لكل مكالمة:

- الرسوم/التحصيل: 50 مللي ثانية لكل مكالمة (`fee_payer`، `fee_sponsor`، `staking_bond`، `staking_slash`)
- التحقق من شهادة الالتزام / JDG / الجسر: 80 مللي ثانية (`commit_cert_verify`، `jdg_attestation_verify`،
  `bridge_proof_verify`)
- تجميع شهادة الالتزام: 80 مللي ثانية (`commit_cert_assembly`)
- جدولة الوصول: 50 مللي ثانية (`access_scheduler`)
- وكيل نقطة نهاية الإثبات: 120 مللي ثانية (`torii_proof_endpoint`)

تلميحات معدل النسخ (`burn_rate_fast`/`burn_rate_slow`) تقوم بتشفير 14.4/6.0
نسب النوافذ المتعددة للترحيل مقابل تنبيهات التذاكر.

## تسخير

قم بتشغيل الحزام عبر `cargo xtask i3-slo-harness`:

```bash
cargo xtask i3-slo-harness \
  --iterations 64 \
  --sample-count 5 \
  --out-dir artifacts/i3_slo/latest
```

النواتج:

- `bench_report.json|csv|md` - نتائج مجموعة مقاعد البدلاء I3 الأولية (تجزئة git + السيناريوهات)
- `slo_report.json|md` — تقييم SLO مع نسبة النجاح/الفشل/الميزانية لكل هدف

يستهلك الحزام ملف الميزانيات ويفرض `benchmarks/i3/slo_thresholds.json`
أثناء الجري على مقاعد البدلاء لتفشل بسرعة عندما يتراجع الهدف.

## القياس عن بعد ولوحات المعلومات

- النهاية: `histogram_quantile(0.99, rate(iroha_slot_duration_ms_bucket[5m]))`
- التحقق من الإثبات: `histogram_quantile(0.99, sum by (le) (rate(zk_verify_latency_ms_bucket{status="Verified"}[5m])))`

لوحات بدء التشغيل Grafana موجودة في `dashboards/grafana/i3_slo.json`. Prometheus
يتم توفير تنبيهات معدل النسخ في `dashboards/alerts/i3_slo_burn.yml` مع
الميزانيات المذكورة أعلاه مخبأة (النهائية 2، إثبات التحقق من 80 مللي ثانية، إثبات وكيل نقطة النهاية
120 مللي ثانية).

## ملاحظات عملية

- قم بتشغيل الحزام في ملابس النوم؛ نشر `artifacts/i3_slo/<stamp>/slo_report.md`
  إلى جانب المصنوعات اليدوية لأدلة الحكم.
- في حالة فشل الميزانية، استخدم قاعدة تخفيض السعر لتحديد السيناريو، ثم انتقل
  في لوحة/تنبيه Grafana المطابق للارتباط بالمقاييس المباشرة.
- تستخدم كائنات مستوى الخدمة (SLO) لنقطة نهاية الإثبات زمن الوصول للتحقق كوكيل لتجنب كل مسار
  تفجير الكاردينالية يتطابق الهدف المعياري (120 مللي ثانية) مع الاحتفاظ/DoS
  الدرابزين على API الإثبات.