---
lang: ar
direction: rtl
source: docs/source/crypto/gost_performance.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7fab384ae80e1993b1e54d6addc82fd3dc652fb6e3958bea6a04e057a1805b57
source_last_modified: "2026-01-03T18:07:57.084090+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# سير عمل أداء GOST

توثق هذه المذكرة كيفية تتبعنا وتطبيق مظروف الأداء الخاص بـ
TC26 GOST التوقيع الخلفي.

## التشغيل محليا

```bash
make gost-bench                     # run benches + tolerance check
make gost-bench GOST_BENCH_ARGS="--tolerance 0.30"  # override guard
make gost-dudect                    # run the constant-time timing guard
./scripts/update_gost_baseline.sh   # bench + rebaseline helper
```

خلف الكواليس يسمي كلا الهدفين `scripts/gost_bench.sh`، والذي:

1. ينفذ `cargo bench -p iroha_crypto --bench gost_sign --features gost -- --noplot`.
2. يقوم بتشغيل `gost_perf_check` مقابل `target/criterion`، والتحقق من المتوسطات مقابل `target/criterion`.
   تم تسجيل الوصول إلى خط الأساس (`crates/iroha_crypto/benches/gost_perf_baseline.json`).
3. يُدخل ملخص تخفيض السعر في `$GITHUB_STEP_SUMMARY` عندما يكون متاحًا.

لتحديث خط الأساس بعد الموافقة على الانحدار/التحسين، قم بتشغيل:

```bash
make gost-bench-update
```

أو مباشرة:

```bash
./scripts/gost_bench.sh --write-baseline \
  --baseline crates/iroha_crypto/benches/gost_perf_baseline.json
```

يقوم `scripts/update_gost_baseline.sh` بتشغيل المدقق + المدقق، والكتابة فوق JSON الأساسي، والطباعة
الوسيطات الجديدة. قم دائمًا بتنفيذ JSON المحدث جنبًا إلى جنب مع سجل القرار في
`crates/iroha_crypto/docs/gost_backend.md`.

### المتوسطات المرجعية الحالية

| الخوارزمية | الوسيط (ميكروثانية) |
|----------------------|------------|
| ed25519 | 69.67 |
| gost256_paramset_a | 1136.96 |
| gost256_paramset_b | 1129.05 |
| gost256_paramset_c | 1133.25 |
| gost512_paramset_a | 8944.39 |
| gost512_paramset_b | 8963.60 |
| secp256k1 | 160.53 |

## سي آي

يستخدم `.github/workflows/gost-perf.yml` نفس البرنامج النصي ويقوم أيضًا بتشغيل حارس توقيت dudect.
يفشل CI عندما يتجاوز المتوسط المقاس خط الأساس بأكثر من التسامح المكوّن
(20% بشكل افتراضي) أو عندما يكتشف واقي التوقيت وجود تسرب، لذلك يتم اكتشاف الانحدارات تلقائيًا.

## ملخص الإخراج

يقوم `gost_perf_check` بطباعة جدول المقارنة محليًا وإلحاق نفس المحتوى به
`$GITHUB_STEP_SUMMARY`، لذا فإن سجلات وظائف CI وملخصات التشغيل تشترك في نفس الأرقام.