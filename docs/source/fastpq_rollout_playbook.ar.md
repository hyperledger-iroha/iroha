---
lang: ar
direction: rtl
source: docs/source/fastpq_rollout_playbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 3a0c22a213e04a6a8fef94ded6ec0017531737ffd4b9418ec94286bb6759ff8a
source_last_modified: "2026-01-08T09:26:20.579700+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# دليل تشغيل FASTPQ (المرحلة 7-3)

يطبق دليل التشغيل هذا متطلبات خريطة الطريق Stage7-3: كل ترقية للأسطول
التي تمكن تنفيذ FASTPQ GPU يجب أن ترفق بيان معياري قابل للتكرار،
أدلة Grafana المقترنة، وتمرين التراجع الموثق. إنه يكمل
`docs/source/fastpq_plan.md` (الأهداف/الهندسة المعمارية) و
`docs/source/fastpq_migration_guide.md` (خطوات الترقية على مستوى العقدة) من خلال التركيز
في قائمة التحقق من الطرح التي تواجه المشغل.

## النطاق والأدوار

- **هندسة الإصدار / SRE:** التقاط المعايير الخاصة، وتوقيع البيان، و
  تصدير لوحة المعلومات قبل الموافقة على الطرح.
- **Ops Guild:** تدير عمليات إطلاق مرحلية، وتسجل تدريبات التراجع، وتخزن
  حزمة المصنوعات اليدوية ضمن `artifacts/fastpq_rollouts/<timestamp>/`.
- **الحوكمة/الامتثال:** تتحقق من أن الأدلة تصاحب كل تغيير
  الطلب قبل تبديل الإعداد الافتراضي FASTPQ للأسطول.

## متطلبات حزمة الأدلة

يجب أن يحتوي كل إرسال للطرح على العناصر التالية. إرفاق كافة الملفات
إلى تذكرة الإصدار/الترقية واحتفظ بالحزمة
`artifacts/fastpq_rollouts/<YYYYMMDD>/<fleet>/<lane>/`.| قطعة أثرية | الغرض | كيفية الإنتاج |
|----------|--------|----------------|
| `fastpq_bench_manifest.json` | إثبات أن حمل العمل الأساسي المكون من 20000 صف يظل ضمن سقف `<1 s` LDE ويسجل التجزئة لكل معيار مضمن.| التقط عمليات تشغيل Metal/CUDA، ولفها، ثم قم بتشغيلها:`cargo xtask fastpq-bench-manifest \``  --bench metal=artifacts/fastpq_benchmarks/<metal>.json \``  --bench cuda=artifacts/fastpq_benchmarks/<cuda>.json \``  --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json \``  --signing-key secrets/fastpq_bench.ed25519 \``  --out artifacts/fastpq_rollouts/<stamp>/fastpq_bench_manifest.json` |
| المعايير المجمعة (`fastpq_metal_bench_*.json`، `fastpq_cuda_bench_*.json`) | التقط بيانات تعريف المضيف، وأدلة استخدام الصف، ونقاط الاتصال الخالية من التعبئة، وملخصات Poseidon microbench، وإحصائيات kernel المستخدمة بواسطة لوحات المعلومات/التنبيهات.| قم بتشغيل `fastpq_metal_bench` / `fastpq_cuda_bench`، ثم قم بلف JSON الأولي:`python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 \``  --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json \``  --poseidon-metrics artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom \``  fastpq_metal_bench.json artifacts/fastpq_benchmarks/<metal>.json --sign-output`كرر ذلك لالتقاط CUDA (نقطة `--row-usage` و`--poseidon-metrics` في ملفات الشاهد/الكشط ذات الصلة). يقوم المساعد بتضمين عينات `fastpq_poseidon_pipeline_total`/`fastpq_execution_mode_total` التي تمت تصفيتها، لذا فإن دليل WP2-E.6 متطابق عبر Metal وCUDA. استخدم `scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>` عندما تحتاج إلى ملخص Poseidon microbench مستقل (يدعم المدخلات المجمعة أو الأولية). |
|  |  | **متطلبات تسمية Stage7:** يفشل `wrap_benchmark.py` الآن ما لم يحتوي القسم `metadata.labels` الناتج على كل من `device_class` و`gpu_kind`. عندما يتعذر على الاكتشاف التلقائي استنتاجها (على سبيل المثال، عند الالتفاف على عقدة CI منفصلة)، قم بتمرير تجاوزات صريحة مثل `--label device_class=xeon-rtx-sm80 --label gpu_kind=discrete`. |
|  |  | **قياس التسريع عن بعد:** يلتقط المجمع أيضًا `cargo xtask acceleration-state --format json` بشكل افتراضي، ويكتب `<bundle>.accel.json` و`<bundle>.accel.prom` بجوار المعيار الملتف (تجاوز بإشارات `--accel-*` أو `--skip-acceleration-state`). تستخدم مصفوفة الالتقاط هذه الملفات لإنشاء `acceleration_matrix.{json,md}` للوحات معلومات الأسطول. |
| تصدير Grafana | إثبات اعتماد القياس عن بعد والتعليقات التوضيحية التنبيهية لنافذة الطرح.| تصدير لوحة المعلومات `fastpq-acceleration`:`curl -s -H "Authorization: Bearer $GRAFANA_TOKEN" \``  "$GRAFANA_URL/api/dashboards/uid/fastpq-acceleration" \``  | jq '.dashboard' \``  > artifacts/fastpq_rollouts/<stamp>/grafana_fastpq_acceleration.json`ضع تعليقًا توضيحيًا على اللوحة مع أوقات بدء/إيقاف الطرح قبل التصدير. يمكن أن يقوم مسار الإصدار بذلك تلقائيًا عبر `scripts/run_release_pipeline.py --export-fastpq-grafana --grafana-url <URL>` (الرمز المميز الذي يتم توفيره عبر `GRAFANA_TOKEN`). |
| لقطة تنبيه | يلتقط قواعد التنبيه التي تحمي عملية الطرح.| انسخ `dashboards/alerts/fastpq_acceleration_rules.yml` (وتركيبات `tests/`) إلى الحزمة حتى يتمكن المراجعون من إعادة تشغيل `promtool test rules …`. |
| سجل الحفر التراجعي | يوضح أن المشغلين تدربوا على الإجراء الاحتياطي الإجباري لوحدة المعالجة المركزية وإقرارات القياس عن بعد.| استخدم الإجراء الموجود في [تدريبات الاستعادة](#rollback-drills) وقم بتخزين سجلات وحدة التحكم (`rollback_drill.log`) بالإضافة إلى كشط Prometheus الناتج (`metrics_rollback.prom`). || `row_usage/fastpq_row_usage_<date>.json` | يقوم بتسجيل تخصيص صف ExecWitness FASTPQ الذي يتتبعه TF-5 في CI ولوحات المعلومات.| قم بتنزيل شاهد جديد من Torii، وقم بفك تشفيره عبر `iroha_cli audit witness --decode exec.witness` (أضف `--fastpq-parameter fastpq-lane-balanced` بشكل اختياري لتأكيد مجموعة المعلمات المتوقعة؛ وتصدر دفعات FASTPQ بشكل افتراضي)، وانسخ `row_usage` JSON إلى `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/row_usage/`. احتفظ بطابع زمني لأسماء الملفات حتى يتمكن المراجعون من ربطها ببطاقة الطرح، وتشغيل `python3 scripts/fastpq/validate_row_usage_snapshot.py row_usage/*.json` (أو `make check-fastpq-rollout`) بحيث تتحقق بوابة Stage7-3 من أن كل دفعة تعلن عن أعداد المحدد و`transfer_ratio = transfer_rows / total_rows` الثابت قبل إرفاق الدليل. |

> **نصيحة:** يقوم `artifacts/fastpq_rollouts/README.md` بتوثيق التسمية المفضلة
> المخطط (`<stamp>/<fleet>/<lane>`) وملفات الأدلة المطلوبة. ال
> يجب أن يقوم المجلد `<stamp>` بتشفير `YYYYMMDDThhmmZ` حتى تظل المصنوعات اليدوية قابلة للفرز
> دون استشارة التذاكر.

## قائمة مراجعة إنشاء الأدلة1. **التقاط معايير GPU.**
   - تشغيل عبء العمل الأساسي (20000 صف منطقي، 32768 صفًا مبطنًا) عبر
     `cargo run -p fastpq_prover --bin fastpq_metal_bench -- --rows 20000 --pretty`.
   - قم بلف النتيجة باستخدام `scripts/fastpq/wrap_benchmark.py` باستخدام `--row-usage <decoded witness>` بحيث تحمل الحزمة دليل الجهاز إلى جانب القياس عن بعد لوحدة معالجة الرسومات. قم بتمرير `--require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 --sign-output` بحيث يفشل المجمّع بسرعة إذا تجاوز أي من المسرّع الهدف أو إذا كان القياس عن بعد لقائمة انتظار/ملف تعريف Poseidon مفقودًا، ولإنشاء التوقيع المنفصل.
   - كرر ذلك على مضيف CUDA بحيث يحتوي البيان على عائلتي GPU.
   - لا **لا** تنزع `benchmarks.metal_dispatch_queue` أو
     كتل `benchmarks.zero_fill_hotspots` من ملف JSON المغلف. بوابة سي.آي
     (`ci/check_fastpq_rollout.sh`) يقرأ الآن تلك الحقول ويفشل عند قائمة الانتظار
     تنخفض المساحة الرئيسية إلى أقل من فتحة واحدة أو عندما تقوم أي نقطة اتصال LDE بالإبلاغ عن `mean_ms>
     0.40 مللي ثانية، مما يؤدي إلى فرض حماية القياس عن بعد Stage7 تلقائيًا.
2. **إنشاء البيان.** استخدم `cargo xtask fastpq-bench-manifest …` كـ
   يظهر في الجدول. قم بتخزين `fastpq_bench_manifest.json` في حزمة الطرح.
3. **تصدير Grafana.**
   - قم بتعليق لوحة `FASTPQ Acceleration Overview` بنافذة الطرح،
     الارتباط بمعرفات لوحة Grafana ذات الصلة.
   - قم بتصدير لوحة المعلومات JSON عبر Grafana API (الأمر أعلاه) وقم بتضمينها
     قسم `annotations` حتى يتمكن المراجعون من مطابقة منحنيات الاعتماد مع
     طرح مرحلي.
4. **تنبيهات اللقطات.** انسخ قواعد التنبيه الدقيقة (`dashboards/alerts/…`) المستخدمة
   عن طريق الطرح في الحزمة. إذا تم تجاوز قواعد Prometheus، فقم بتضمينها
   فرق التجاوز
5. **Prometheus/OTEL سكراب.** التقط `fastpq_execution_mode_total{device_class="<matrix>"}` من كل منهما
   المضيف (قبل وبعد المسرح) بالإضافة إلى عداد OTEL
   `fastpq.execution_mode_resolutions_total` والمقترن
   إدخالات السجل `telemetry::fastpq.execution_mode`. وهذه القطع الأثرية تثبت ذلك
   يعد اعتماد وحدة معالجة الرسومات (GPU) مستقرًا ولا تزال عمليات القياس الاحتياطية القسرية لوحدة المعالجة المركزية (CPU) تنبعث منها القياس عن بعد.
6. ** أرشفة القياس عن بعد لاستخدام الصف. ** بعد فك تشفير ExecWitness، قم بتشغيل
   في البداية، قم بإسقاط JSON الناتج ضمن `row_usage/` في الحزمة. سي.آي
   يقوم المساعد (`ci/check_fastpq_row_usage.sh`) بمقارنة هذه اللقطات مع ملف
   خطوط الأساس الأساسية، و`ci/check_fastpq_rollout.sh` تتطلب الآن كل
   حزمة لشحن ملف `row_usage` واحد على الأقل للاحتفاظ بأدلة TF-5 مرفقة
   إلى تذكرة الإفراج.

## تدفق الطرح المرحلي

استخدم ثلاث مراحل حتمية لكل أسطول. التقدم فقط بعد الخروج
استيفاء المعايير في كل مرحلة وتوثيقها في حزمة الأدلة.| المرحلة | النطاق | معايير الخروج | المرفقات |
|-------|-------|--------------|------------|
| طيار (ف1) | 1 مستوى تحكم + 1 عقدة مستوى بيانات لكل منطقة | `fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}` ≥90% لمدة 48 ساعة، وعدم حدوث أي حوادث لـ Alertmanager، وتمرير عملية التراجع. | حزمة من كلا المضيفين (JSONs البدلاء، تصدير Grafana مع تعليق توضيحي تجريبي، وسجلات التراجع). |
| المنحدر (ف2) | ≥50% من المدققين بالإضافة إلى ممر أرشيفي واحد على الأقل لكل مجموعة | يستمر تنفيذ وحدة معالجة الرسومات لمدة 5 أيام، ولا يزيد ارتفاع مستوى التخفيض مرة واحدة عن 10 دقائق، وتثبت عدادات Prometheus التنبيه بالتراجعات خلال 60 ثانية. | تم تحديث تصدير Grafana لعرض التعليق التوضيحي المنحدر، واختلافات الكشط Prometheus، ولقطة شاشة/سجل Alertmanager. |
| الافتراضي (P3) | العقد المتبقية؛ تم وضع علامة FASTPQ على الإعداد الافتراضي في `iroha_config` | بيان البدلاء الموقع + تصدير Grafana الذي يشير إلى منحنى الاعتماد النهائي، وتمرين التراجع الموثق الذي يوضح تبديل التكوين. | البيان النهائي، Grafana JSON، سجل التراجع، مرجع التذكرة لمراجعة تغيير التكوين. |

قم بتوثيق كل خطوة ترويجية في تذكرة الطرح واربطها مباشرة بـ
التعليقات التوضيحية `grafana_fastpq_acceleration.json` حتى يتمكن المراجعون من ربط
الجدول الزمني مع الأدلة.

## تدريبات التراجع

يجب أن تتضمن كل مرحلة من مراحل الطرح تمرينًا للتراجع:

1. اختر عقدة واحدة لكل مجموعة وسجل المقاييس الحالية:
   ```bash
   curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
2. فرض وضع وحدة المعالجة المركزية (CPU) لمدة 10 دقائق باستخدام مقبض التكوين
   (`zk.fastpq.execution_mode = "cpu"`) أو تجاوز البيئة:
   ```bash
   FASTPQ_GPU=cpu irohad --config <path> --genesis-manifest-json <path>
   ```
3. قم بتأكيد سجل الرجوع إلى إصدار أقدم
   (`telemetry::fastpq.execution_mode resolved="cpu" requested="gpu"`) وكشط
   نقطة النهاية Prometheus مرة أخرى لإظهار زيادات العداد.
4. قم باستعادة وضع GPU، وتحقق من أن `telemetry::fastpq.execution_mode` يقدم تقاريره الآن
   `resolved="metal"` (أو `resolved="cuda"/"opencl"` للممرات غير المعدنية)،
   تأكد من أن كشط Prometheus يحتوي على عينات من وحدة المعالجة المركزية ووحدة معالجة الرسومات
   `fastpq_execution_mode_total{backend=…}`، وقم بتسجيل الوقت المنقضي
   الكشف/التنظيف.
5. قم بتخزين نصوص Shell والمقاييس وإقرارات المشغل كما يلي
   `rollback_drill.log` و`metrics_rollback.prom` في حزمة الطرح. هذه
   يجب أن توضح الملفات دورة الرجوع إلى إصدار أقدم + الاستعادة الكاملة لأن
   يفشل `ci/check_fastpq_rollout.sh` الآن عندما يفتقر السجل إلى وحدة معالجة الرسومات
   يحذف سطر الاسترداد أو لقطة المقاييس إما عدادات وحدة المعالجة المركزية (CPU) أو وحدة معالجة الرسومات (GPU).

تثبت هذه السجلات أن كل مجموعة يمكن أن تتحلل بأمان وأن فرق SRE
تعرف على كيفية التراجع بشكل حتمي في حالة تراجع برامج تشغيل وحدة معالجة الرسومات أو النوى.

## الدليل الاحتياطي للوضع المختلط (WP2-E.6)

عندما يحتاج المضيف إلى GPU FFT/LDE ولكن تجزئة وحدة المعالجة المركزية Poseidon (لكل Stage7 <900ms
المتطلبات)، قم بتجميع العناصر التالية جنبًا إلى جنب مع سجلات التراجع القياسية:1. ** فرق التكوين. ** تحقق (أو أرفق) تجاوز المضيف المحلي الذي تم تعيينه
   `zk.fastpq.poseidon_mode = "cpu"` (`FASTPQ_POSEIDON_MODE=cpu`) أثناء المغادرة
   `zk.fastpq.execution_mode` لم يمسها أحد. اسم التصحيح
   `artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/poseidon_fallback.patch`.
2. **مكشطة بوسيدون المضادة.**
   ```bash
   curl -s http://<host>:8180/metrics \
     | rg 'fastpq_poseidon_pipeline_total{.*device_class="<label>"' \
     > artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>/metrics_poseidon.prom
   ```
   يجب أن يُظهر الالتقاط تزايد `path="cpu_forced"` بخطوة القفل باستخدام ملف
   عداد GPU FFT/LDE لفئة الجهاز تلك. خذ كشطًا ثانيًا بعد العودة
   العودة إلى وضع GPU حتى يتمكن المراجعون من رؤية استئناف الصف `path="gpu"`.

   قم بتمرير الملف الناتج إلى `wrap_benchmark.py --poseidon-metrics …` بحيث يسجل المعيار المغلف نفس العدادات داخل القسم `poseidon_metrics` الخاص به؛ يؤدي هذا إلى الحفاظ على عمليات طرح Metal وCUDA في نفس سير العمل ويجعل الدليل الاحتياطي قابلاً للتدقيق دون فتح ملفات كشط منفصلة.
3. **مقتطف من السجل.** انسخ إدخالات `telemetry::fastpq.poseidon` التي تثبت
   تم قلب المحلل إلى وحدة المعالجة المركزية (`cpu_forced`) إلى
   `poseidon_fallback.log`، مع الاحتفاظ بالطوابع الزمنية حتى يمكن إنشاء المخططات الزمنية لـ Alertmanager
   يرتبط مع تغيير التكوين.

تقوم CI بفرض عمليات التحقق من قائمة الانتظار/الملء الصفري اليوم؛ بمجرد وصول البوابة ذات الوضع المختلط،
سوف يصر `ci/check_fastpq_rollout.sh` أيضًا على أن أي حزمة تحتوي على
يقوم `poseidon_fallback.patch` بإرسال لقطة `metrics_poseidon.prom` المطابقة.
يؤدي اتباع سير العمل هذا إلى إبقاء السياسة الاحتياطية WP2-E.6 قابلة للتدقيق ومرتبطة بها
نفس جامعي الأدلة الذين استخدموا أثناء الطرح الافتراضي.

## إعداد التقارير والأتمتة

- قم بإرفاق الدليل `artifacts/fastpq_rollouts/<stamp>/` بالكامل إلى ملف
  قم بتحرير التذكرة وقم بالإشارة إليها من `status.md` بمجرد إغلاق الطرح.
- تشغيل `dashboards/alerts/tests/fastpq_acceleration_rules.test.yml` (عبر
  `promtool`) داخل CI لضمان حزم التنبيهات المجمعة مع استمرار الطرح
  ترجمة.
- التحقق من صحة الحزمة باستخدام `ci/check_fastpq_rollout.sh` (أو
  `make check-fastpq-rollout`) وتمرير `FASTPQ_ROLLOUT_BUNDLE=<path>` عندما
  تريد استهداف طرح واحد. يستدعي CI نفس البرنامج النصي عبر
  `.github/workflows/fastpq-rollout.yml`، لذلك تفشل المصنوعات اليدوية المفقودة بسرعة قبل ملف
  يمكن إغلاق تذكرة الإصدار. يمكن لخط أنابيب الإصدار أرشفة الحزم التي تم التحقق من صحتها
  بجانب البيانات الموقعة بالمرور
  `--fastpq-rollout-bundle artifacts/fastpq_rollouts/<stamp>/<fleet>/<lane>` إلى
  `scripts/run_release_pipeline.py`; يعيد المساعد
  `ci/check_fastpq_rollout.sh` (ما لم يتم تعيين `--skip-fastpq-rollout-check`) و
  نسخ شجرة الدليل إلى `artifacts/releases/<version>/fastpq_rollouts/…`.
  كجزء من هذه البوابة، يفرض البرنامج النصي عمق قائمة الانتظار Stage7 والتعبئة الصفرية
  الميزانيات من خلال قراءة `benchmarks.metal_dispatch_queue` و
  `benchmarks.zero_fill_hotspots` من كل `metal` مقعد JSON.

باتباع كتاب اللعب هذا، يمكننا إثبات التبني الحتمي، وتقديم أ
حزمة أدلة واحدة لكل عملية طرح، وحافظ على مراجعة تدريبات التراجع جنبًا إلى جنب
يظهر المعيار الموقع.