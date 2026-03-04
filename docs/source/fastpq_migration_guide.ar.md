---
lang: ar
direction: rtl
source: docs/source/fastpq_migration_guide.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 99e435a831d793035e71915ca567d3e61cb28b89627e0cf0ebdec72aa57a981d
source_last_modified: "2026-01-04T10:50:53.613193+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

#! دليل ترحيل إنتاج FASTPQ

يصف دليل التشغيل هذا كيفية التحقق من صحة مُثبت FASTPQ لإنتاج Stage6.
تمت إزالة الواجهة الخلفية للعنصر النائب الحتمي كجزء من خطة الترحيل هذه.
إنه يكمل الخطة المرحلية في `docs/source/fastpq_plan.md` ويفترض أنك تتبع بالفعل
حالة مساحة العمل في `status.md`.

## الجمهور والنطاق
- يقوم مشغلو أداة التحقق بطرح مُثبت الإنتاج في بيئات التشغيل المرحلي أو الشبكة الرئيسية.
- إطلاق المهندسين الذين يقومون بإنشاء ثنائيات أو حاويات سيتم شحنها مع الواجهة الخلفية للإنتاج.
- تقوم فرق SRE/المراقبة بتوصيل إشارات وتنبيهات القياس عن بعد الجديدة.

خارج النطاق: Kotodama تأليف العقد وتغييرات IVM ABI (راجع `docs/source/nexus.md` للتعرف على
نموذج التنفيذ).

## مصفوفة الميزات
| المسار | ميزات الشحن لتمكين | النتيجة | متى تستخدم |
| ---- | ----------------------- | ------ | ----------- |
| مُثبِّت الإنتاج (افتراضي) | _لا شيء_ | الواجهة الخلفية Stage6 FASTPQ مع مخطط FFT/LDE وخط أنابيب DEEP-FRI. الافتراضي لجميع ثنائيات الإنتاج. |
| تسريع GPU الاختياري | `fastpq_prover/fastpq-gpu` | لتمكين CUDA/Metal kernels مع احتياطي وحدة المعالجة المركزية التلقائي. المضيفين مع المسرعات المدعومة. |

## إجراءات البناء
1. ** بناء وحدة المعالجة المركزية فقط **
   ```bash
   cargo build --release -p irohad
   cargo build --release -p iroha_cli
   ```
   يتم تجميع الواجهة الخلفية للإنتاج بشكل افتراضي؛ لا توجد ميزات إضافية مطلوبة.

2. **إصدار يدعم وحدة معالجة الرسومات (اختياري)**
   ```bash
   export FASTPQ_GPU=auto        # honour GPU detection at build-time helpers
   cargo build --release -p irohad --features fastpq_prover/fastpq-gpu
   ```
   يتطلب دعم وحدة معالجة الرسومات مجموعة أدوات SM80+ CUDA مع توفر `nvcc` أثناء الإنشاء.

3. **الاختبارات الذاتية**
   ```bash
   cargo test -p fastpq_prover
   ```
   قم بتشغيل هذا مرة واحدة لكل إصدار إصدار لتأكيد مسار Stage6 قبل التعبئة.

### إعداد سلسلة الأدوات المعدنية (macOS)
1. قم بتثبيت أدوات سطر الأوامر المعدنية قبل الإنشاء: `xcode-select --install` (إذا كانت أدوات CLI مفقودة) و`xcodebuild -downloadComponent MetalToolchain` لجلب سلسلة أدوات GPU. يستدعي البرنامج النصي للإنشاء `xcrun metal`/`xcrun metallib` مباشرةً وسيفشل بسرعة في حالة غياب الثنائيات.
2. للتحقق من صحة المسار قبل CI، يمكنك عكس البرنامج النصي للبناء محليًا:
   ```bash
   export OUT_DIR=$PWD/target/metal && mkdir -p "$OUT_DIR"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"
   xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"
   xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"
   export FASTPQ_METAL_LIB="$OUT_DIR/fastpq.metallib"
   ```
   عندما ينجح هذا، يصدر البناء `FASTPQ_METAL_LIB=<path>`؛ يقرأ وقت التشغيل هذه القيمة لتحميل metallib بشكل حتمي.
3. قم بتعيين `FASTPQ_SKIP_GPU_BUILD=1` عند التجميع المتقاطع بدون سلسلة الأدوات المعدنية؛ يطبع الإصدار تحذيرًا ويظل المخطط على مسار وحدة المعالجة المركزية.
4. تعود العقد إلى وحدة المعالجة المركزية تلقائيًا في حالة عدم توفر المعدن (إطار عمل مفقود، أو وحدة معالجة رسومات غير مدعومة، أو `FASTPQ_METAL_LIB` فارغ)؛ يقوم البرنامج النصي للبناء بمسح env var ويقوم المخطط بتسجيل الرجوع إلى إصدار سابق.### القائمة المرجعية للإصدار (المرحلة السادسة)
احتفظ بتذكرة إصدار FASTPQ محظورة حتى يتم إكمال كل عنصر أدناه وإرفاقه.

1. **مقاييس إثبات الثانية الفرعية** — افحص `fastpq_metal_bench_*.json` الذي تم التقاطه حديثًا و
   قم بتأكيد الإدخال `benchmarks.operations` حيث `operation = "lde"` (والنسخة المتطابقة
   نموذج `report.operations`) يُبلغ عن `gpu_mean_ms ≤ 950` لحمل العمل المكون من 20000 صف (32768 مبطن
   الصفوف). تتطلب اللقطات خارج السقف إعادة التشغيل قبل أن يتم التوقيع على قائمة التحقق.
2. **البيان الموقع** — تشغيل
   `cargo xtask fastpq-bench-manifest --bench metal=<json> --bench cuda=<json> --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key <path> --out artifacts/fastpq_bench_manifest.json`
   لذا فإن تذكرة الإفراج تحمل كلا من البيان وتوقيعه المنفصل
   (`artifacts/fastpq_bench_manifest.sig`). يتحقق المراجعون من زوج الملخص/التوقيع من قبل
   الترويج لإصدار.[xtask/src/fastpq.rs:128] 【xtask/src/main.rs:845】 بيان المصفوفة (مُصمم
   عبر `scripts/fastpq/capture_matrix.sh`) يقوم بالفعل بتشفير أرضية الصف المكونة من 20 ألفًا و
   تصحيح الانحدار.
3. **مرفقات الأدلة** — قم بتحميل معيار JSON للمعادن، أو سجل stdout (أو تتبع الأدوات)،
   مخرجات بيان CUDA/Metal، والتوقيع المنفصل عن تذكرة الإصدار. إدخال القائمة المرجعية
   يجب أن ترتبط بجميع المصنوعات اليدوية بالإضافة إلى بصمة المفتاح العام المستخدمة للتوقيع على عمليات التدقيق النهائية
   يمكنه إعادة تشغيل خطوة التحقق.[artifacts/fastpq_benchmarks/README.md:65]### سير عمل التحقق من صحة المعادن
1. بعد إنشاء تمكين وحدة معالجة الرسومات، قم بتأكيد نقاط `FASTPQ_METAL_LIB` عند `.metallib` (`echo $FASTPQ_METAL_LIB`) حتى يتمكن وقت التشغيل من تحميله بشكل حتمي.
2. قم بتشغيل مجموعة التكافؤ مع تشغيل ممرات GPU:\
   `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq_prover/fastpq-gpu --release`. ستقوم الواجهة الخلفية بتمرين النوى المعدنية وتسجيل احتياطي حتمي لوحدة المعالجة المركزية في حالة فشل الاكتشاف.
3. قم بالتقاط عينة مرجعية للوحات المعلومات:\
   حدد موقع مكتبة Metal المترجمة (`fd -g 'fastpq.metallib' target/release/build | head -n1`)،
   قم بتصديره عبر `FASTPQ_METAL_LIB`، وقم بتشغيل\
  `cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`.
  يقوم الآن ملف تعريف `fastpq-lane-balanced` الأساسي بتوسيع كل عملية التقاط إلى 32,768 صفًا (2¹⁵)، لذا يحمل JSON كلاً من `rows` و`padded_rows` جنبًا إلى جنب مع زمن انتقال Metal LDE؛ أعد تشغيل الالتقاط إذا دفعت `zero_fill` أو إعدادات قائمة الانتظار وحدة معالجة الرسومات LDE إلى ما هو أبعد من هدف 950 مللي ثانية (<1 ثانية) على مضيفي سلسلة AppleM. أرشفة JSON/log الناتج إلى جانب أدلة الإصدار الأخرى؛ يقوم سير عمل macOS الليلي بتنفيذ نفس التشغيل وتحميل عناصره للمقارنة.
  عندما تحتاج إلى القياس عن بعد لـ Poseidon فقط (على سبيل المثال، لتسجيل تتبع الآلات)، أضف `--operation poseidon_hash_columns` إلى الأمر أعلاه؛ ستظل المنصة تحترم `FASTPQ_GPU=gpu`، وتنبعث منها `metal_dispatch_queue.poseidon`، وتتضمن كتلة `poseidon_profiles` الجديدة، لذا فإن حزمة الإصدار توثق عنق الزجاجة Poseidon بشكل صريح.
  تتضمن الأدلة الآن `zero_fill.{bytes,ms,queue_delta}` بالإضافة إلى `kernel_profiles` (لكل نواة
  الإشغال، وGB/s المقدرة، وإحصائيات المدة) بحيث يمكن رسم بياني لكفاءة وحدة معالجة الرسومات بدونها
  إعادة معالجة الآثار الأولية، وكتلة `twiddle_cache` (الزيارات/الإخفاقات + `before_ms`/`after_ms`) التي
  يثبت أن عمليات التحميل المخزنة مؤقتًا سارية المفعول. يقوم `--trace-dir` بإعادة تشغيل الحزام الموجود أسفله
  `xcrun xctrace record` و
  يخزن ملف `.trace` ذو الطابع الزمني إلى جانب JSON؛ لا يزال بإمكانك تقديم مفصل
  `--trace-output` (مع `--trace-template` / `--trace-seconds` الاختياري) عند الالتقاط إلى
  الموقع/القالب المخصص. يسجل JSON `metal_trace_{template,seconds,output}` للتدقيق.بعد كل التقاط، قم بتشغيل `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json` بحيث يحمل المنشور بيانات تعريف المضيف (بما في ذلك الآن `metadata.metal_trace`) لحزمة اللوحة/التنبيه Grafana (`dashboards/grafana/fastpq_acceleration.json`، `dashboards/alerts/fastpq_acceleration_rules.yml`). يحمل التقرير الآن كائن `speedup` لكل عملية (`speedup.ratio`، `speedup.delta_ms`)، ورافعات المجمّع `zero_fill_hotspots` (البايت، ووقت الاستجابة، وGB/s المشتقة، وعدادات دلتا قائمة الانتظار المعدنية)، وتسوية `kernel_profiles` إلى `benchmarks.kernel_summary`، يحافظ على كتلة `twiddle_cache` سليمة، وينسخ كتلة/ملخص `post_tile_dispatches` الجديد حتى يتمكن المراجعون من إثبات تشغيل kernel متعدد التمريرات أثناء الالتقاط، ويلخص الآن دليل Poseidon microbench في `benchmarks.poseidon_microbench` حتى تتمكن لوحات المعلومات من اقتباس زمن الوصول العددي مقابل الافتراضي دون إعادة تحليل التقرير الخام. تقرأ بوابة البيان نفس الكتلة وترفض حزم أدلة وحدة معالجة الرسومات التي تحذفها، مما يجبر المشغلين على تحديث اللقطات عندما يتم تخطي مسار ما بعد التجانب أو تم تكوينها بشكل غير صحيح.[الصناديق/fastpq_prover/src/bin/fastpq_metal_bench.rs:1048][scripts/fastpq/wrap_benchmark.py:714][scripts/fastpq/wrap_benchmark.py:732][xtask/src/fastpq.rs:280]
  تشترك نواة Poseidon2 المعدنية في نفس المقابض: `FASTPQ_METAL_POSEIDON_LANES` (32–256، قوى اثنين) و`FASTPQ_METAL_POSEIDON_BATCH` (1–32 حالة لكل حارة) تتيح لك تثبيت عرض الإطلاق والعمل لكل حارة دون إعادة البناء؛ يقوم المضيف بربط هذه القيم عبر `PoseidonArgs` قبل كل إرسال. افتراضيًا، يقوم وقت التشغيل بفحص `MTLDevice::{is_low_power,is_headless,location}` لانحياز وحدات معالجة الرسومات المنفصلة نحو عمليات الإطلاق ذات طبقات VRAM (`256×24` عند الإبلاغ عن ≥48GiB، و`256×20` عند 32GiB، و`256×16` بخلاف ذلك) بينما تظل SoCs منخفضة الطاقة على `256×8` (والأجزاء الأقدم من الممرات 128/64 تلتزم بـ 8/6 حالات لكل حارة)، لذلك لا يحتاج معظم المشغلين أبدًا إلى ضبط متغيرات env يدويًا. `FASTPQ_METAL_POSEIDON_MICRO_MODE={default,scalar}` ويصدر كتلة `poseidon_microbench` التي تسجل كلاً من ملفات تعريف الإطلاق بالإضافة إلى السرعة المقاسة مقابل المسار العددي بحيث يمكن لحزم الإصدار أن تثبت أن النواة الجديدة تقلص فعليًا `poseidon_hash_columns`، وتتضمن كتلة `poseidon_pipeline` لذا فإن دليل Stage7 يلتقط عمق القطعة/مقابض التداخل جنبًا إلى جنب مع طبقات الإشغال الجديدة. اترك البيئة غير مضبوطة للتشغيل العادي؛ يدير الحزام إعادة التنفيذ تلقائيًا، ويسجل حالات الفشل إذا تعذر تشغيل الالتقاط الفرعي، ويخرج فورًا عند تعيين `FASTPQ_GPU=gpu` ولكن لا تتوفر واجهة خلفية لوحدة معالجة الرسومات، لذا لا تتسلل عمليات احتياطية صامتة لوحدة المعالجة المركزية إلى الأداء أبدًا المصنوعات اليدوية.[الصناديق/fastpq_prover/src/bin/fastpq_metal_bench.rs:1691][الصناديق/fastpq_prover/src/bin/fastpq_metal_bench.rs:1988]يرفض المجمع عمليات التقاط Poseidon التي تفتقد دلتا `metal_dispatch_queue.poseidon` أو عدادات `column_staging` المشتركة أو كتل الأدلة `poseidon_profiles`/`poseidon_microbench` لذلك يجب على المشغلين تحديث أي التقاط يفشل في إثبات التداخل المرحلي أو العدد العددي مقابل الافتراضي speedup.[scripts/fastpq/wrap_benchmark.py:732] عندما تحتاج إلى JSON مستقل للوحات المعلومات أو دلتا CI، قم بتشغيل `python3 scripts/fastpq/export_poseidon_microbench.py --bundle <bundle>`؛ يقبل المساعد كلاً من العناصر الملتفة والتقاطات `fastpq_metal_bench*.json` الأولية، وينبعث منها `benchmarks/poseidon/poseidon_microbench_<timestamp>.json` مع التوقيت الافتراضي/العددي، وضبط البيانات الوصفية، والتسريع المسجل.[scripts/fastpq/export_poseidon_microbench.py:1]
  قم بإنهاء التشغيل عن طريق تنفيذ `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json` بحيث تفرض قائمة التحقق من إصدار Stage6 سقف `<1 s` LDE وتصدر حزمة بيان/ملخص موقعة تأتي مع تذكرة الإصدار.[xtask/src/fastpq.rs:1] 【artifacts/fastpq_benchmarks/README.md:65】
4. تحقق من القياس عن بعد قبل بدء التشغيل: قم بلف نقطة النهاية Prometheus (`fastpq_execution_mode_total{device_class="<matrix>", backend="metal"}`) وافحص سجلات `telemetry::fastpq.execution_mode` بحثًا عن `resolved="cpu"` غير المتوقع الإدخالات.[الصناديق/iroha_telemetry/src/metrics.rs:8887] 【الصناديق/fastpq_prover/src/backend.rs:174】
5. قم بتوثيق المسار الاحتياطي لوحدة المعالجة المركزية عن طريق فرضه عمدًا (`FASTPQ_GPU=cpu` أو `zk.fastpq.execution_mode = "cpu"`) بحيث تظل قواعد تشغيل SRE متوافقة مع السلوك الحتمي.6. الضبط الاختياري: افتراضيًا يختار المضيف 16 مسارًا للتتبعات القصيرة، و32 مسارًا للتتبعات المتوسطة، و64/128 مرة واحدة `log_len ≥ 10/14`، ويهبط عند 256 عندما `log_len ≥ 18`، ويحتفظ الآن بلوحة الذاكرة المشتركة في خمس مراحل للآثار الصغيرة، وأربع مرة واحدة `log_len ≥ 12`، و مراحل 14/12/16 لـ `log_len ≥ 18/20/22` قبل بدء العمل إلى نواة ما بعد التبليط. قم بتصدير `FASTPQ_METAL_FFT_LANES` (قوة اثنين بين 8 و256) و/أو `FASTPQ_METAL_FFT_TILE_STAGES` (1–16) قبل تنفيذ الخطوات المذكورة أعلاه لتجاوز تلك الاستدلالات. يتم اشتقاق أحجام دفعة أعمدة FFT/IFFT وLDE من عرض مجموعة الخيوط التي تم حلها (≈2048 سلسلة منطقية لكل إرسال، متوج بـ 32 عمودًا، وتتصاعد الآن خلال 32 → 16 → 8 → 4 → 2 → 1 مع نمو المجال) بينما لا يزال مسار LDE يفرض حدود النطاق الخاصة به؛ قم بتعيين `FASTPQ_METAL_FFT_COLUMNS` (1–32) لتثبيت حجم دفعة FFT محدد و`FASTPQ_METAL_LDE_COLUMNS` (1–32) لتطبيق نفس التجاوز على مرسل LDE عندما تحتاج إلى مقارنات بت مقابل بت عبر الأجهزة المضيفة. يعكس عمق تجانب LDE استدلالات FFT أيضًا - تعمل التتبعات باستخدام `log₂ ≥ 18/20/22` فقط على تشغيل 12/10/8 مراحل الذاكرة المشتركة قبل تسليم الفراشات العريضة إلى نواة ما بعد التبليط - ويمكنك تجاوز هذا الحد عبر `FASTPQ_METAL_LDE_TILE_STAGES` (1–32). يقوم وقت التشغيل بتمرير جميع القيم من خلال وسيطات Metal kernel، ويثبت التجاوزات غير المدعومة، ويسجل القيم التي تم حلها بحيث تظل التجارب قابلة للتكرار دون إعادة بناء metallib؛ يعرض معيار JSON كلاً من الضبط الذي تم حله وميزانية التعبئة الصفرية للمضيف (`zero_fill.{bytes,ms,queue_delta}`) التي تم التقاطها عبر إحصائيات LDE بحيث يتم ربط دلتا قائمة الانتظار مباشرة بكل التقاط، ويضيف الآن كتلة `column_staging` (دفعات مسطحة، ومسطحة، وانتظار_ms، ونسبة انتظار) حتى يتمكن المراجعون من التحقق من تداخل المضيف/الجهاز المقدم من خلال خط الأنابيب المزدوج المخزن. عندما ترفض وحدة معالجة الرسومات الإبلاغ عن القياس عن بعد ذي التعبئة الصفرية، يقوم الحزام الآن بتجميع توقيت محدد من المخزن المؤقت من جانب المضيف ومسحه وإدخاله في كتلة `zero_fill` بحيث لا يتم شحن الأدلة مطلقًا بدون الحقل.[الصناديق/fastpq_prover/src/metal_config.rs:15][الصناديق/fastpq_prover/src/metal.rs:742][الصناديق/fastpq_prover/src/bin/fastpq_met al_bench.rs:575 】 【الصناديق/fastpq_prover/src/bin/fastpq_metal_bench.rs:1609】 【الصناديق/fastpq_prover/src/bin/fastpq_metal_bench.rs:1860】7. يتم إرسال قوائم الانتظار المتعددة تلقائيًا على أجهزة Mac المنفصلة: عندما يُرجع `Device::is_low_power()` خطأً أو يُبلغ الجهاز المعدني عن موقع فتحة/خارجي، يقوم المضيف بإنشاء مثيلين `MTLCommandQueue`، ويتم الترحيل فقط بمجرد أن يحمل حمل العمل ≥16 عمودًا (يتم قياسه بواسطة الموسع)، ويقوم بتدوير دفعات الأعمدة عبر قوائم الانتظار بحيث تحافظ الآثار الطويلة على انشغال كلا ممري GPU دون الحتمية المساومة. قم بتجاوز السياسة باستخدام `FASTPQ_METAL_QUEUE_FANOUT` (قوائم الانتظار من 1 إلى 4) و`FASTPQ_METAL_COLUMN_THRESHOLD` (الحد الأدنى لإجمالي الأعمدة قبل التوزيع الموسع) عندما تحتاج إلى عمليات التقاط قابلة للتكرار عبر الأجهزة؛ تجبر اختبارات التكافؤ هذه التجاوزات بحيث تظل أجهزة Mac متعددة وحدات معالجة الرسومات مغطاة ويتم تسجيل عتبة/خروج المروحة التي تم حلها بجوار القياس عن بعد لعمق قائمة الانتظار.### أدلة للأرشفة
| قطعة أثرية | التقاط | ملاحظات |
|----------|--------|-------|
| حزمة `.metallib` | `xcrun metal -std=metal3.0 -O3 -c metal/kernels/ntt_stage.metal -o "$OUT_DIR/ntt_stage.air"` و`xcrun metal -std=metal3.0 -O3 -c metal/kernels/poseidon2.metal -o "$OUT_DIR/poseidon2.air"` متبوعًا بـ `xcrun metallib "$OUT_DIR/ntt_stage.air" "$OUT_DIR/poseidon2.air" -o "$OUT_DIR/fastpq.metallib"` و`export FASTPQ_METAL_LIB=$OUT_DIR/fastpq.metallib`. | يثبت أن Metal CLI/toolchain قد تم تركيبه وإنتاج مكتبة حتمية لهذا الالتزام.
| لقطة بيئية | `echo $FASTPQ_METAL_LIB` بعد الإنشاء؛ حافظ على المسار المطلق مع تذكرة الإصدار الخاصة بك. | الإخراج الفارغ يعني أنه تم تعطيل المعدن؛ تسجيل مستندات القيمة التي تظل ممرات GPU متاحة على قطعة الشحن الفنية.
| سجل تكافؤ GPU | `FASTPQ_GPU=gpu cargo test -p fastpq_prover --features fastpq-gpu --release` وأرشفة المقتطف الذي يحتوي على `backend="metal"` أو تحذير الرجوع إلى إصدار سابق. | يوضح أن النوى تعمل (أو تتراجع بشكل حتمي) قبل أن تقوم بترقية البناء.
| الناتج المعياري | `FASTPQ_METAL_LIB=$(fd -g 'fastpq.metallib' target/release/build | head -n1) cargo run -p fastpq_prover --features fastpq-gpu --bin fastpq_metal_bench --release -- --rows 20000 --iterations 5 --output fastpq_metal_bench.json --trace-dir traces`; قم باللف والتوقيع عبر `python3 scripts/fastpq/wrap_benchmark.py --require-lde-mean-ms 950 --require-poseidon-mean-ms 1000 fastpq_metal_bench.json artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --sign-output [--gpg-key <fingerprint>]`. | يسجل JSON الملفوف `speedup.ratio`، `speedup.delta_ms`، ضبط FFT، الصفوف المبطنة (32768)، `zero_fill`/`kernel_profiles` المسطح، `kernel_summary` المسطح، الذي تم التحقق منه كتل `metal_dispatch_queue.poseidon`/`poseidon_profiles` (عند استخدام `--operation poseidon_hash_columns`)، وبيانات تعريف التتبع بحيث يظل متوسط GPU LDE ≥950 مللي ثانية ويبقى Poseidon <1s؛ احتفظ بكل من الحزمة وتوقيع `.json.asc` الذي تم إنشاؤه مع تذكرة الإصدار حتى تتمكن لوحات المعلومات والمدققون من التحقق من المنتج دون إعادة التشغيل أحمال العمل.[الصناديق/fastpq_prover/src/bin/fastpq_metal_bench.rs:697][scripts/fastpq/wrap_benchmark.py:714][scripts/fastpq/wrap_benchmark.py:732] |
| بيان مقاعد البدلاء | `cargo xtask fastpq-bench-manifest --bench metal=artifacts/fastpq_benchmarks/fastpq_metal_bench_<date>_macos14_arm64.json --bench cuda=artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json --matrix artifacts/fastpq_benchmarks/matrix/matrix_manifest.json --signing-key secrets/fastpq_bench.ed25519 --out artifacts/fastpq_bench_manifest.json`. | التحقق من صحة كل من عناصر GPU، ويفشل إذا تجاوز متوسط ​​LDE الحد الأقصى `<1 s`، ويسجل هضم BLAKE3/SHA-256، ويصدر بيانًا موقعًا بحيث لا يمكن لقائمة التحقق من الإصدار أن تتقدم بدون مقاييس يمكن التحقق منها. 【xtask/src/fastpq.rs:1 】 【artifacts/fastpq_benchmarks/README.md:65】 |
| حزمة كودا | قم بتشغيل `FASTPQ_GPU=gpu cargo run -p fastpq_prover --bin fastpq_cuda_bench --release -- --rows 20000 --iterations 5 --column-count 16 --device 0 --row-usage artifacts/fastpq_benchmarks/fastpq_row_usage_2025-05-12.json` على مضيف المختبر SM80، وقم بلف/تسجيل JSON في `artifacts/fastpq_benchmarks/fastpq_cuda_bench_<date>_sm80.json` (استخدم `--label device_class=xeon-rtx-sm80` حتى تلتقط لوحات المعلومات الفئة الصحيحة)، وأضف المسار إلى `artifacts/fastpq_benchmarks/matrix/devices/xeon-rtx-sm80.txt`، واحتفظ بالزوج `.json`/`.asc` مع قطعة أثرية معدنية قبل تجديد البيان. يوضح `fastpq_cuda_bench_2025-11-12T090501Z_ubuntu24_x86_64.json` الذي تم تسجيله تنسيق الحزمة الدقيق الذي يتوقعه المدققون.
| إثبات القياس عن بعد | `curl -s http://<host>:8180/metrics | rg 'fastpq_execution_mode_total{device_class'` بالإضافة إلى سجل `telemetry::fastpq.execution_mode` المنبعث عند بدء التشغيل. | يؤكد أن Prometheus/OTEL يعرض `device_class="<matrix>", backend="metal"` (أو سجل الرجوع إلى إصدار سابق) قبل تمكين حركة المرور.| الحفر القسري لوحدة المعالجة المركزية | قم بتشغيل دفعة قصيرة باستخدام `FASTPQ_GPU=cpu` أو `zk.fastpq.execution_mode = "cpu"` والتقط سجل الرجوع إلى إصدار سابق. | يحافظ على محاذاة دفاتر تشغيل SRE مع المسار الاحتياطي الحتمي في حالة الحاجة إلى التراجع في منتصف الإصدار.
| التقاط التتبع (اختياري) | كرر اختبار التكافؤ باستخدام `FASTPQ_METAL_TRACE=1 FASTPQ_GPU=gpu …` واحفظ تتبع الإرسال المنبعث. | يحافظ على دليل الإشغال/مجموعة مؤشرات الترابط لمراجعات التوصيف اللاحقة دون إعادة تشغيل المعايير.

تشير ملفات `fastpq_plan.*` متعددة اللغات إلى قائمة المراجعة هذه، لذا يتبع مشغلو التدريج والإنتاج نفس مسار الأدلة.[docs/source/fastpq_plan.md:1]

## بنيات قابلة للتكرار
استخدم سير عمل الحاوية المثبتة لإنتاج منتجات Stage6 القابلة للتكرار:

```bash
scripts/fastpq/repro_build.sh --mode cpu                     # CPU-only toolchain
scripts/fastpq/repro_build.sh --mode gpu --output artifacts/fastpq-repro-gpu
scripts/fastpq/repro_build.sh --container-runtime podman     # Explicit runtime override
```

يقوم البرنامج النصي المساعد بإنشاء صورة سلسلة الأدوات `rust:1.88.0-slim-bookworm` (و`nvidia/cuda:12.2.2-devel-ubuntu22.04` لـ GPU)، وتشغيل البنية داخل الحاوية، وكتابة `manifest.json`، و`sha256s.txt`، والثنائيات المجمعة إلى الإخراج المستهدف الدليل.[scripts/fastpq/repro_build.sh:1] 【scripts/fastpq/run_inside_repro_build.sh:1】[scripts/fastpq/docker/Dockerfile.gpu:1]

تجاوزات البيئة:
- `FASTPQ_RUST_IMAGE`، `FASTPQ_RUST_TOOLCHAIN` - تثبيت قاعدة/علامة الصدأ الواضحة.
- `FASTPQ_CUDA_IMAGE` – قم بتبديل قاعدة CUDA عند إنتاج عناصر GPU.
- `FASTPQ_CONTAINER_RUNTIME` - فرض وقت تشغيل محدد؛ الافتراضي `auto` يحاول `FASTPQ_CONTAINER_RUNTIME_FALLBACKS`.
- `FASTPQ_CONTAINER_RUNTIME_FALLBACKS` - ترتيب تفضيل مفصول بفواصل للاكتشاف التلقائي لوقت التشغيل (الإعداد الافتراضي هو `docker,podman,nerdctl`).

## تحديثات التكوين
1. قم بتعيين وضع تنفيذ وقت التشغيل في TOML الخاص بك:
   ```toml
   [zk.fastpq]
   execution_mode = "auto"   # or "cpu"/"gpu"
   ```
   يتم تحليل القيمة من خلال `FastpqExecutionMode` وربطها في الواجهة الخلفية عند بدء التشغيل.

2. قم بالتجاوز عند الإطلاق إذا لزم الأمر:
   ```bash
   irohad --fastpq-execution-mode gpu ...
   ```
   يتجاوز سطر الأوامر (CLI) تغيير التكوين الذي تم حله قبل تشغيل العقدة.

3. يمكن للمطورين فرض الاكتشاف مؤقتًا دون لمس التكوينات عن طريق التصدير
   `FASTPQ_GPU={auto,cpu,gpu}` قبل إطلاق الملف الثنائي؛ يتم تسجيل التجاوز وخط الأنابيب
   لا يزال يظهر الوضع الذي تم حله.

## قائمة التحقق
1. **سجلات بدء التشغيل**
   - توقع `FASTPQ execution mode resolved` من الهدف `telemetry::fastpq.execution_mode` مع
     التسميات `requested` و`resolved` و`backend`.[crates/fastpq_prover/src/backend.rs:208]
   - عند الكشف التلقائي عن وحدة معالجة الرسومات، يُبلغ السجل الثانوي من `fastpq::planner` عن المسار الأخير.
   - سطح المضيف المعدني `backend="metal"` عندما يتم تحميل metallib بنجاح؛ في حالة فشل التجميع أو التحميل، يُصدر البرنامج النصي للإنشاء تحذيرًا، ويمسح `FASTPQ_METAL_LIB`، ويسجل المخطط `GPU acceleration unavailable` قبل الاستمرار وحدة المعالجة المركزية.[الصناديق/fastpq_prover/build.rs:29][الصناديق/fastpq_prover/src/backend.rs:174] 【الصناديق/fastpq_prover/src/backend.rs:195】[الصناديق/fastpq_prover/src/metal.rs:43]2. **مقاييس Prometheus**
   ```bash
   curl -s http://localhost:8180/metrics | rg 'fastpq_execution_mode_total{device_class'
   ```
   تتم زيادة العداد عبر `record_fastpq_execution_mode` (المسمى الآن بواسطة
   `{device_class,chip_family,gpu_kind}`) عندما تحل العقدة تنفيذها
   الوضع.[الصناديق/iroha_telemetry/src/metrics.rs:8887]
   - تأكيد التغطية المعدنية
     `fastpq_execution_mode_total{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>", backend="metal"}`
     الزيادات بجانب لوحات معلومات النشر الخاصة بك. 【crates/iroha_telemetry/src/metrics.rs:5397】
   - تعرض عقد macOS المجمعة مع `irohad --features fastpq-gpu` بالإضافة إلى ذلك
     `fastpq_metal_queue_ratio{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",queue="global",metric="busy"}`
     و
     `fastpq_metal_queue_depth{device_class="<matrix>",chip_family="<family>",gpu_kind="<kind>",metric="limit"}` لذا فإن لوحات معلومات Stage7
     يمكنه تتبع دورة العمل ومساحة قائمة الانتظار من الخدوش المباشرة Prometheus.

3. **تصدير أجهزة القياس عن بعد**
   - تقوم شركة OTEL بإصدار `fastpq.execution_mode_resolutions_total` بنفس التسميات؛ تأكد من الخاص بك
     تراقب لوحات المعلومات أو التنبيهات حدوث `resolved="cpu"` غير المتوقع عندما تكون وحدات معالجة الرسومات نشطة.

4. **السلامة تثبت/تحقق**
   - تشغيل دفعة صغيرة من خلال `iroha_cli` أو أداة التكامل وتأكيد إثباتات التحقق على
     الأقران المترجمة مع نفس المعلمات.

## استكشاف الأخطاء وإصلاحها
- **يظل الوضع الذي تم حله على وحدة المعالجة المركزية (CPU) على الأجهزة المضيفة لوحدة معالجة الرسومات** — تأكد من إنشاء الملف الثنائي باستخدامه
  `fastpq_prover/fastpq-gpu`، مكتبات CUDA موجودة في مسار أداة التحميل، ولا يتم فرض `FASTPQ_GPU`
  `cpu`.
- **المعدن غير متوفر في Apple Silicon** - تحقق من تثبيت أدوات CLI (`xcode-select --install`)، وأعد تشغيل `xcodebuild -downloadComponent MetalToolchain`، وتأكد من أن الإصدار أنتج مسار `FASTPQ_METAL_LIB` غير فارغ؛ تؤدي القيمة الفارغة أو المفقودة إلى تعطيل الواجهة الخلفية حسب التصميم.
- **أخطاء `Unknown parameter`** — تأكد من أن كلاً من المثبت والمدقق يستخدمان نفس الكتالوج الأساسي
  المنبعثة من `fastpq_isi`؛ تظهر عدم التطابقات على أنها `Error::UnknownParameter`.[crates/fastpq_prover/src/proof.rs:133]
- **احتياط غير متوقع لوحدة المعالجة المركزية** - افحص `cargo tree -p fastpq_prover --features` و
  تأكد من وجود `fastpq_prover/fastpq-gpu` في إصدارات GPU؛ تحقق من وجود مكتبات `nvcc`/CUDA في مسار البحث.
- **عداد القياس عن بعد مفقود** — تحقق من بدء العقدة باستخدام `--features telemetry` (افتراضي)
  ويتضمن تصدير OTEL (في حالة تمكينه) خط الأنابيب المتري.

## الإجراء الاحتياطي
تمت إزالة الواجهة الخلفية للعنصر النائب الحتمي. إذا كان الانحدار يتطلب التراجع،
أعد نشر عناصر الإصدار الجيدة المعروفة سابقًا وتحقق منها قبل إعادة إصدار Stage6
الثنائيات. قم بتوثيق قرار إدارة التغيير وتأكد من اكتمال التقدم للأمام فقط بعد
ومن المفهوم الانحدار.

3. مراقبة القياس عن بعد للتأكد من أن `fastpq_execution_mode_total{device_class="<matrix>", backend="none"}` يعكس ما هو متوقع
   تنفيذ العنصر النائب.

## خط الأساس للأجهزة
| الملف الشخصي | وحدة المعالجة المركزية | GPU | ملاحظات |
| ------- | --- | --- | ----- |
| المرجع (المرحلة السادسة) | AMD EPYC7B12 (32 نواة)، 256 جيجا بايت رام | نفيديا A10040GB (CUDA12.2) | يجب أن تكتمل الدُفعات الاصطناعية التي يبلغ عددها 20000 صف بـ ≥1000 مللي ثانية. 【docs/source/fastpq_plan.md:131】 |
| وحدة المعالجة المركزية فقط | ≥32 نواة مادية، AVX2 | – | توقع ~0.9–1.2 ثانية لـ 20000 صف؛ احتفظ بـ `execution_mode = "cpu"` للحتمية. |## اختبارات الانحدار
-`cargo test -p fastpq_prover --release`
- `cargo test -p fastpq_prover --release --features fastpq_prover/fastpq-gpu` (على مضيفي GPU)
- فحص التركيبات الذهبية الاختيارية:
  ```bash
  cargo test -p fastpq_prover --test backend_regression --release -- --ignored
  ```

قم بتوثيق أي انحرافات عن قائمة التحقق هذه في دليل تشغيل العمليات الخاص بك وقم بتحديث `status.md` بعد
اكتملت نافذة الهجرة.