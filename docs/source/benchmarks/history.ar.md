---
lang: ar
direction: rtl
source: docs/source/benchmarks/history.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3aad1366bd823bddaca32dc82573d41ec6572a6d9f969dc1e0c6146ea068e03e
source_last_modified: "2026-01-03T18:08:00.425813+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# سجل الالتقاط المعياري لوحدة معالجة الرسومات (FASTPQ WP5-B)

يتم إنشاء هذا الملف بواسطة `python3 scripts/fastpq/update_benchmark_history.py`.
إنه يلبي متطلبات FASTPQ Stage 7 WP5-B من خلال تتبع كل وحدة معالجة رسومات ملفوفة
قطعة أثرية قياسية، وبيان Poseidon microbench، وعمليات المسح المساعدة بالأسفل
`benchmarks/`. قم بتحديث اللقطات الأساسية وأعد تشغيل البرنامج النصي كلما تم إنشاء ملف جديد
تحتاج أراضي الحزمة أو القياس عن بعد إلى أدلة جديدة.

## النطاق وعملية التحديث

- إنتاج أو تغليف لقطات GPU جديدة (عبر `scripts/fastpq/wrap_benchmark.py`)،
  قم بإلحاقها بمصفوفة الالتقاط، ثم أعد تشغيل هذا المولد لتحديث ملف
  الجداول.
- عند وجود بيانات Poseidon microbench، قم بتصديرها باستخدام
  `scripts/fastpq/export_poseidon_microbench.py` وأعد إنشاء البيان باستخدام
  `scripts/fastpq/aggregate_poseidon_microbench.py`.
- سجل عمليات مسح عتبة Merkle عن طريق تخزين مخرجات JSON الخاصة بها تحتها
  `benchmarks/merkle_threshold/`; يسرد هذا المولد الملفات المعروفة حتى يقوم بالتدقيق
  يمكن أن يكون مرجعًا ترافقيًا لتوفر وحدة المعالجة المركزية (CPU) مقابل توفر وحدة معالجة الرسومات (GPU).

## معايير FASTPQ للمرحلة 7 لوحدة معالجة الرسومات

| حزمة | الخلفية | الوضع | الواجهة الخلفية لوحدة معالجة الرسومات | GPU متاح | فئة الجهاز | GPU | LDE مللي ثانية (CPU/GPU/SU) | بوسيدون مللي ثانية (CPU/GPU/SU) |
|-------|---------|------|--------------|-------------|-----|----------------------------|------|----------------|
| `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` | كودا | وحدة معالجة الرسومات | كودا-sm80 | نعم | زيون-RTX | نفيديا ار تي اكس 6000 ادا | 1512.9/880.7/1.72 | —/—/— |
| `fastpq_metal_bench_2025-11-07T123018Z_macos14_arm64.json` | معدن | وحدة معالجة الرسومات | لا شيء | نعم | أبل-M4 | معالج رسوميات أبل 40 نواة | 785.6/735.6/1.07 | 1803.8/1897.5/0.95 |
| `fastpq_metal_bench_20251108T192645Z_macos14_arm64.json` | معدن | وحدة معالجة الرسومات | معدن | نعم | أبل-m2-ألترا | ابل M2 الترا | 1581.1/1604.5/0.98 | 3589.9/3697.3/0.97 |
| `fastpq_metal_bench_20251108T225946_macos_arm64.json` | معدن | وحدة معالجة الرسومات | معدن | نعم | أبل-m2-ألترا | ابل M2 الترا | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_metal_bench_20251108T231910_macos_arm64_withtrace.json` | معدن | وحدة معالجة الرسومات | معدن | نعم | أبل-m2-ألترا | ابل M2 الترا | 1804.5/1666.4/1.08 | 3939.5/4083.3/0.96 |
| `fastpq_opencl_bench_2025-11-18T074455Z_ubuntu24_aarch64.json` | اوبنكل | وحدة معالجة الرسومات | اوبنكل | نعم | نيوفيرس-mi300 | ايه ام دي غريزة MI300A | 4518.5/688.9/6.56 | 2780.4/905.6/3.07 |

> الأعمدة: `Backend` مشتق من اسم الحزمة؛ `Mode`/`GPU backend`/`GPU available`
> يتم نسخها من كتلة `benchmarks` الملتفة لكشف احتياطيات وحدة المعالجة المركزية أو وحدة معالجة الرسومات المفقودة
> الاكتشاف (على سبيل المثال، `gpu_backend=none` بالرغم من `Mode=gpu`). SU = نسبة التسريع (CPU/GPU).

## لقطات بوسيدون Microbench

يقوم `benchmarks/poseidon/manifest.json` بتجميع بوسيدون الافتراضي مقابل العددي
يتم تصدير تشغيل microbench من كل حزمة معدنية. يتم تحديث الجدول أدناه بواسطة
البرنامج النصي للمولد، بحيث يمكن لمراجعات CI والحوكمة أن تختلف عن التسريعات التاريخية
دون تفريغ تقارير FASTPQ الملتفة.

| ملخص | حزمة | الطابع الزمني | مللي ثانية افتراضية | مللي ثانية | تسريع |
|---------|--------|----------|------------|-----------|---------|
| `benchmarks/poseidon/poseidon_microbench_debug.json` | `fastpq_metal_bench_debug.json` | 2025-11-09T06:11:01Z | 2167.7 | 2152.2 | 0.99 |
| `benchmarks/poseidon/poseidon_microbench_full.json` | `fastpq_metal_bench_full.json` | 2025-11-09T06:04:07Z | 1990.5 | 1994.5 | 1.00 |

## عمليات مسح عتبة ميركلتم جمع اللقطات المرجعية عبر
`cargo run --release -p ivm --features metal --example merkle_threshold -- --json`
العيش تحت `benchmarks/merkle_threshold/`. تظهر إدخالات القائمة ما إذا كان المضيف
الأجهزة المعدنية المكشوفة أثناء عملية المسح؛ يجب الإبلاغ عن اللقطات الممكّنة بواسطة GPU
`metal_available=true`.

- `benchmarks/merkle_threshold/macos14_arm64_cpu.json` — `metal_available=False`
- `benchmarks/merkle_threshold/macos14_arm64_metal.json` — `metal_available=False`
- `benchmarks/merkle_threshold/takemiyacStudio.lan_25.0.0_arm64.json` — `metal_available=True`

إن التقاط Apple Silicon (`takemiyacStudio.lan_25.0.0_arm64`) هو خط الأساس الأساسي لوحدة معالجة الرسومات المستخدمة في `docs/source/benchmarks.md`؛ تظل إدخالات macOS 14 بمثابة خطوط أساسية لوحدة المعالجة المركزية (CPU) فقط للبيئات التي لا يمكنها الكشف عن الأجهزة المعدنية.

## لقطات استخدام الصف

تثبت رموز فك الشاهد التي تم التقاطها عبر `scripts/fastpq/check_row_usage.py` عملية النقل
كفاءة صف الأداة. احتفظ بعناصر JSON ضمن `artifacts/fastpq_benchmarks/`
وسيقوم هذا المولد بتلخيص نسب التحويل المسجلة للمراجعين.

- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-02-01.json` - الدُفعات=2، متوسط نسبة النقل=0.629 (الحد الأدنى=0.625، الحد الأقصى=0.633)
- `artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` - الدُفعات=2، متوسط نسبة النقل=0.619 (الحد الأدنى=0.613، الحد الأقصى=0.625)