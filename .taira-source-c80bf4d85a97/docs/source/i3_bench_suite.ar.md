---
lang: ar
direction: rtl
source: docs/source/i3_bench_suite.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a3158cd70a42104bacaafc520fdcc10e20e3bc347d895be448fcb10da4f668bd
source_last_modified: "2026-01-03T18:08:01.692664+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# Iroha جناح 3 مقاعد

مجموعة مقاعد البدلاء Iroha 3 تضاهي المسارات الساخنة التي نعتمد عليها أثناء عملية التوقيع والرسوم
الشحن والتحقق من الإثبات والجدولة ونقاط نهاية الإثبات. يتم تشغيله باعتباره
أمر `xtask` مع التركيبات الحتمية (البذور الثابتة، المادة الرئيسية الثابتة،
وحمولات الطلب المستقرة) بحيث تكون النتائج قابلة للتكرار عبر الأجهزة المضيفة.

## تشغيل المجموعة

```bash
cargo xtask i3-bench-suite \
  --iterations 64 \
  --sample-count 5 \
  --json-out benchmarks/i3/latest.json \
  --csv-out benchmarks/i3/latest.csv \
  --markdown-out benchmarks/i3/latest.md \
  --threshold benchmarks/i3/thresholds.json \
  --allow-overwrite
```

الأعلام:

- يتحكم `--iterations` في التكرارات لكل عينة سيناريو (الافتراضي: 64).
- يكرر `--sample-count` كل سيناريو لحساب المتوسط ​​(الافتراضي: 5).
- `--json-out|--csv-out|--markdown-out` اختر عناصر الإخراج (جميعها اختيارية).
- `--threshold` يقارن المتوسطات بحدود خط الأساس (المجموعة `--no-threshold`)
  لتخطي).
- `--flamegraph-hint` يعلق على تقرير Markdown مع `cargo flamegraph`
  أمر لملف تعريف السيناريو.

يوجد غراء CI في `ci/i3_bench_suite.sh` ويتم تعيينه افتراضيًا على المسارات المذكورة أعلاه؛ مجموعة
`I3_BENCH_ITERATIONS`/`I3_BENCH_SAMPLES` لضبط وقت التشغيل في الصحف الليلية.

## السيناريوهات

- `fee_payer` / `fee_sponsor` / `fee_insufficient` — الخصم مقابل الجهة الدافعة
  ورفض النقص.
- `staking_bond` / `staking_slash` — قائمة انتظار السند/فك الارتباط مع وبدون
  التقطيع.
- `commit_cert_verify` / `jdg_attestation_verify` / `bridge_proof_verify` —
  التحقق من التوقيع عبر شهادات الالتزام وشهادات JDG والجسر
  حمولات إثبات.
- `commit_cert_assembly` - تجميع الملخص لشهادات الالتزام.
- `access_scheduler` — جدولة مجموعة الوصول المدركة للتعارض.
- `torii_proof_endpoint` - تحليل نقطة النهاية لإثبات أكسوم + التحقق ذهابًا وإيابًا.

يسجل كل سيناريو متوسط النانو ثانية لكل تكرار، وإنتاجية، و
عداد التخصيص الحتمي للانحدارات السريعة. العتبات تعيش في
`benchmarks/i3/thresholds.json`; حدود عثرة هناك عندما تتغير الأجهزة و
ارتكاب قطعة أثرية جديدة جنبا إلى جنب مع التقرير.

## استكشاف الأخطاء وإصلاحها

- قم بتثبيت تردد وحدة المعالجة المركزية/الحاكم عند جمع الأدلة لتجنب الانحدارات الصاخبة.
- استخدم `--no-threshold` لعمليات التشغيل الاستكشافية، ثم أعد التمكين بمجرد الوصول إلى خط الأساس
  منتعش.
- لتكوين ملف تعريف لسيناريو واحد، قم بتعيين `--iterations 1` وأعد التشغيل ضمنه
  `cargo flamegraph -p xtask -- i3-bench-suite --iterations 128 --sample-count 1 --no-threshold --flamegraph-hint`.