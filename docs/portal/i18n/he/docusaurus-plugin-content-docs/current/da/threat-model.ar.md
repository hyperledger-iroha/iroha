---
lang: he
direction: rtl
source: docs/portal/docs/da/threat-model.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
يعكس `docs/source/da/threat_model.md`. ابق النسختين متزامنتين حتى يتم سحب
الوثائق القديمة.
:::

# نموذج تهديدات توفر البيانات في Sora Nexus

_اخر مراجعة: 2026-01-19 -- المراجعة القادمة المجدولة: 2026-04-19_

وتيرة الصيانة: Data Availability Working Group (<=90 يوما). يجب ان تظهر كل
مراجعة في `status.md` مع روابط لتذاكر التخفيف النشطة واثار المحاكاة.

## الهدف والنطاق

برنامج Data Availability (DA) يبقي بث Taikai و blobs مسارات Nexus وادوات
الحوكمة قابلة للاسترجاع تحت اخطاء بيزنطية، وشبكات، ومشغلين. هذا النموذج
يثبت عمل الهندسة لDA-1 (البنية ونموذج التهديدات) ويعد خط اساس لمهام DA اللاحقة
(DA-2 حتى DA-10).

المكونات ضمن النطاق:
- امتداد ingest في Torii وكتابة metadata Norito.
- اشجار تخزين blobs المدعومة بSoraFS (طبقات hot/cold) وسياسات التكرار.
- تعهدات كتل Nexus (wire formats, proofs, light-client APIs).
- hooks enforcement لPDP/PoTR الخاصة بحمولات DA.
- تدفقات المشغل (pinning, eviction, slashing) وخطوط الرصد.
- موافقات الحوكمة التي تقبل او تزيل مشغلي DA والمحتوى.

خارج النطاق لهذا المستند:
- نمذجة الاقتصاد الكاملة (ضمن مسار DA-7).
- بروتوكولات SoraFS الاساسية المغطاة في نموذج تهديدات SoraFS.
- ارغونوميا SDK للعميل خارج اعتبارات سطح التهديد.

## نظرة عامة معمارية

1. **الارسال:** يرسل العملاء blobs عبر API ingest لDA في Torii. تقوم العقدة
   بتجزئة blobs، وترميز manifests Norito (نوع blob، lane، epoch، اعلام codec)،
   وتخزين chunks في طبقة SoraFS الساخنة.
2. **الاعلان:** تنتشر نوايا pin وتلميحات التكرار الى مزودي التخزين عبر
   registry (SoraFS marketplace) مع وسوم سياسة تحدد اهداف الاحتفاظ hot/cold.
3. **الالتزام:** يدرج sequencers في Nexus تعهدات blobs (CID + جذور KZG اختيارية)
   في الكتلة القياسية. يعتمد light clients على hash الالتزام والmetadata
   المعلنة للتحقق من availability.
4. **التكرار:** تسحب عقد التخزين الحصص/chunks المخصصة، وتلبي تحديات PDP/PoTR،
   وتقوم بترقية البيانات بين طبقات hot وcold حسب السياسة.
5. **الجلب:** يسترجع المستهلكون البيانات عبر SoraFS او بوابات DA-aware مع
   التحقق من proofs ورفع طلبات اصلاح عند اختفاء النسخ.
6. **الحوكمة:** يوافق البرلمان ولجنة الاشراف على DA على المشغلين وجداول rent
   وتصعيدات enforcement. تحفظ ادوات الحوكمة عبر نفس مسار DA لضمان شفافية
   العملية.

## الاصول والمالكون

مقياس الاثر: **حرج** يكسر سلامة/حيوية الدفتر؛ **عال** يحجب backfill DA او
العملاء؛ **متوسط** يخفض الجودة لكنه قابل للاسترجاع؛ **منخفض** اثر محدود.

| الاصل | الوصف | السلامة | الاتاحة | السرية | المالك |
| --- | --- | --- | --- | --- | --- |
| Blobs DA (chunks + manifests) | Blobs Taikai و lane وادوات الحوكمة المخزنة في SoraFS | حرج | حرج | متوسط | DA WG / Storage Team |
| Manifests Norito DA | Metadata مصنفة تصف blobs | حرج | عال | متوسط | Core Protocol WG |
| تعهدات الكتل | CIDs + جذور KZG داخل كتل Nexus | حرج | عال | منخفض | Core Protocol WG |
| جداول PDP/PoTR | وتيرة enforcement لنسخ DA | عال | عال | منخفض | Storage Team |
| سجل المشغلين | مزودو التخزين الموافق عليهم والسياسات | عال | عال | منخفض | Governance Council |
| سجلات rent والحوافز | قيود الدفتر لعائدات DA والعقوبات | عال | متوسط | منخفض | Treasury WG |
| لوحات الرصد | SLOs DA وعمق التكرار والتنبيهات | متوسط | عال | منخفض | SRE / Observability |
| نوايا الاصلاح | طلبات لاعادة ترطيب chunks المفقودة | متوسط | متوسط | منخفض | Storage Team |

## الخصوم والقدرات

| الفاعل | القدرات | الدوافع | الملاحظات |
| --- | --- | --- | --- |
| عميل خبيث | ارسال blobs تالفة، اعادة تشغيل manifests قديمة، محاولة DoS على ingest. | تعطيل بث Taikai، حقن بيانات غير صحيحة. | لا مفاتيح مميزة. |
| عقدة تخزين بيزنطية | اسقاط نسخ مخصصة، تزوير proofs PDP/PoTR، تواطؤ. | تقليص احتفاظ DA، تجنب rent، احتجاز البيانات. | يحمل بيانات اعتماد مشغل صحيحة. |
| sequencer مخترق | حذف تعهدات، ازدواجية كتل، اعادة ترتيب metadata. | اخفاء ارسال DA، خلق عدم اتساق. | محدود باغلبية الاجماع. |
| مشغل داخلي | اساءة استخدام الوصول للحوكمة، العبث بسياسات الاحتفاظ، تسريب بيانات اعتماد. | مكسب اقتصادي، تخريب. | وصول الى بنية hot/cold. |
| خصم شبكة | تقسيم العقد، تاخير التكرار، حقن حركة MITM. | خفض الاتاحة، تدهور SLOs. | لا يكسر TLS لكنه يمكنه اسقاط/ابطاء الروابط. |
| مهاجم الرصد | العبث باللوحات/التنبيهات، اخفاء الحوادث. | اخفاء انقطاعات DA. | يتطلب وصولا لخط التليمترية. |

## حدود الثقة

- **حد الدخول:** العميل الى امتداد DA في Torii. يتطلب auth على مستوى الطلب،
  تحديد المعدل، والتحقق من payload.
- **حد التكرار:** عقد التخزين تتبادل chunks وproofs. العقد متصادقة لكنها قد
  تتصرف بشكل بيزنطي.
- **حد الدفتر:** بيانات الكتلة الملتزمة مقابل التخزين خارج السلسلة. الاجماع
  يحمي السلامة، لكن الاتاحة تتطلب enforcement خارج السلسلة.
- **حد الحوكمة:** قرارات Council/Parliament لاعتماد المشغلين والميزانيات
  والعقوبات. الاختراق هنا يؤثر مباشرة على نشر DA.
- **حد الرصد:** جمع metrics/logs المصدرة الى لوحات/تنبيهات. العبث يخفي
  الانقطاعات او الهجمات.

## سيناريوهات التهديد والضوابط

### هجمات مسار ingest

**السيناريو:** عميل خبيث يرسل payloads Norito تالفة او blobs كبيرة جدا لاستنزاف
الموارد او تمرير metadata غير صحيحة.

**الضوابط**
- تحقق صارم من schema Norito مع تفاوض الاصدار؛ رفض الاعلام غير المعروفة.
- تحديد المعدل والمصادقة على endpoint ingest في Torii.
- حدود chunk size وترميز حتمي يفرضه chunker SoraFS.
- خط القبول لا يحفظ manifests الا بعد تطابق checksum السلامة.
- Replay cache حتمي (`ReplayCache`) يتتبع نوافذ `(lane, epoch, sequence)`،
  يحفظ high-water marks على القرص، ويرفض التكرار/اعادة التشغيل القديمة؛
  harnesses property وfuzz تغطي fingerprints مختلفة وارسال خارج الترتيب.
  [crates/iroha_core/src/da/replay_cache.rs:1]
  [fuzz/da_replay_cache.rs:1] [crates/iroha_torii/src/da/ingest.rs:1]

**الثغرات المتبقية**
- يجب ان يمر Torii ingest replay cache في مسار القبول ويحفظ مؤشرات sequence عبر
  اعادة التشغيل.
- مخططات Norito DA تملك الان fuzz harness مخصصا (`fuzz/da_ingest_schema.rs`) لفحص
  ثوابت encode/decode؛ يجب ان تنبه لوحات التغطية عند التراجع.

### احتجاز التكرار

**السيناريو:** مشغلو التخزين البيزنطيون يقبلون مهام pin لكنهم يسقطون chunks،
ويمررون تحديات PDP/PoTR عبر ردود مزورة او تواطؤ.

**الضوابط**
- جدول تحديات PDP/PoTR يمتد لحمولات DA مع تغطية لكل epoch.
- تكرار متعدد المصادر مع عتبات quorum؛ orchestrator يكتشف shards المفقودة
  ويطلق اصلاحات.
- slashing حوكمي مرتبط بفشل proofs والنسخ المفقودة.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) تقارن receipts
  بcommitments DA (SignedBlockWire, `.norito`, او JSON)، تبعث bundle JSON كدليل
  للحوكمة، وتفشل عند فقدان/عدم تطابق tickets لكي يطلق Alertmanager تنبيهات.

**الثغرات المتبقية**
- harness المحاكاة في `integration_tests/src/da/pdp_potr.rs` (مغطى عبر
  `integration_tests/tests/da/pdp_potr_simulation.rs`) يختبر سيناريوهات تواطؤ
  وتقسيم، ويتحقق ان جدول PDP/PoTR يكتشف السلوك البيزنطي بشكل حتمي. استمر في
  توسيعه مع DA-5 لتغطية اسطح proof الجديدة.
- سياسة eviction لطبقة cold تحتاج سجل تدقيق موقع لمنع الاسقاطات الخفية.

### العبث بالتعهدات

**السيناريو:** sequencer مخترق ينشر كتل تحذف او تغير تعهدات DA، ما يسبب فشل
fetch او عدم اتساق لدى light clients.

**الضوابط**
- الاجماع يطابق مقترحات الكتل مع قوائم ارسال DA؛ النظراء يرفضون المقترحات
  التي تفتقد تعهدات مطلوبة.
- light clients يتحققون من inclusion proofs قبل اظهار handles للfetch.
- سجل تدقيق يقارن receipts الارسال بتعهدات الكتل.
- مهمة مصالحة تلقائية (`cargo xtask da-commitment-reconcile`) تقارن receipts
  بcommitments DA (SignedBlockWire, `.norito`, او JSON)، تبعث bundle JSON كدليل
  للحوكمة، وتفشل عند فقدان/عدم تطابق tickets لكي يطلق Alertmanager تنبيهات.

**الثغرات المتبقية**
- مغطاة بمهمة المصالحة + hook Alertmanager؛ حزم الحوكمة تستهلك bundle JSON
  كافتراضي.

### تقسيم الشبكة والرقابة

**السيناريو:** خصم يقسم شبكة التكرار، مانعا العقد من الحصول على chunks
المخصصة او الرد على تحديات PDP/PoTR.

**الضوابط**
- متطلبات مزودين multi-region تضمن مسارات شبكة متنوعة.
- نوافذ التحدي تتضمن jitter وfallback الى قنوات اصلاح خارج النطاق.
- لوحات الرصد تراقب عمق التكرار ونجاح التحديات وزمن fetch مع عتبات تنبيه.

**الثغرات المتبقية**
- محاكاة التقسيم لاحداث Taikai المباشرة ما زالت مفقودة؛ نحتاج soak tests.
- سياسة حجز سعة اصلاح لم توثق بعد.

### اساءة داخلية

**السيناريو:** مشغل لديه وصول للregistry يتلاعب بسياسات الاحتفاظ، ويمنح
whitelist لمزودين خبيثين، او يخفي التنبيهات.

**الضوابط**
- اعمال الحوكمة تتطلب تواقيع متعددة الاطراف وسجلات Norito موثقة.
- تغييرات السياسة تصدر احداثا للرصد وسجلات ارشيف.
- خط الرصد يفرض سجلات Norito append-only مع hash chaining.
- اتمتة مراجعة الوصول ربع السنوية (`cargo xtask da-privilege-audit`) تتفقد
  ادلة manifest/replay (مع مسارات يحددها المشغلون)، وتعلم العناصر المفقودة/
  غير-دليل/قابلة للكتابة عالميا، وتنتج bundle JSON موقع للوحات الحوكمة.

**الثغرات المتبقية**
- ادلة العبث في اللوحات تحتاج snapshots موقعة.

## سجل المخاطر المتبقية

| الخطر | الاحتمال | الاثر | المالك | خطة التخفيف |
| --- | --- | --- | --- | --- |
| Replay لmanifests DA قبل وصول cache sequence في DA-2 | ممكن | متوسط | Core Protocol WG | تنفيذ sequence cache + تحقق nonce في DA-2؛ اضافة اختبارات تراجع. |
| تواطؤ PDP/PoTR عند اختراق >f عقد | غير محتمل | عال | Storage Team | اشتقاق جدول تحديات جديد مع sampling cross-provider؛ التحقق عبر harness المحاكاة. |
| فجوة تدقيق eviction في طبقة cold | ممكن | عال | SRE / Storage Team | ربط سجلات موقعة ووصولات on-chain لعمليات eviction؛ الرصد عبر اللوحات. |
| زمن اكتشاف حذف sequencer | ممكن | عال | Core Protocol WG | `cargo xtask da-commitment-reconcile` ليلي يقارن receipts بالcommitments (SignedBlockWire/`.norito`/JSON) ويطلق تنبيه حوكمي عند tickets مفقودة او غير متطابقة. |
| مقاومة التقسيم لبث Taikai المباشر | ممكن | حرج | Networking TL | تنفيذ تدريبات تقسيم؛ حجز سعة اصلاح؛ توثيق SOP للفشل. |
| انحراف امتيازات الحوكمة | غير محتمل | عال | Governance Council | `cargo xtask da-privilege-audit` ربع سنوي (dirs manifest/replay + مسارات اضافية) مع JSON موقع + gate لوحة؛ تثبيت artefacts التدقيق على السلسلة. |

## المتابعات المطلوبة

1. نشر مخططات Norito لingest DA ومتجهات امثلة (منقولة الى DA-2).
2. تمرير replay cache عبر Torii DA ingest وحفظ مؤشرات sequence عبر اعادة التشغيل.
3. **مكتمل (2026-02-05):** harness محاكاة PDP/PoTR يمارس تواطؤ + تقسيم مع نمذجة
   backlog QoS؛ راجع `integration_tests/src/da/pdp_potr.rs` (مع اختبارات في
   `integration_tests/tests/da/pdp_potr_simulation.rs`) للتنفيذ والملخصات
   الحتمية الملتقطة ادناه.
4. **مكتمل (2026-05-29):** `cargo xtask da-commitment-reconcile` يقارن receipts
   بالcommitments DA (SignedBlockWire/`.norito`/JSON)، ويصدر
   `artifacts/da/commitment_reconciliation.json`، ومربوط بAlertmanager/حزم
   الحوكمة لتنبيهات omission/tampering (`xtask/src/da.rs`).
5. **مكتمل (2026-05-29):** `cargo xtask da-privilege-audit` يتجول في spool
   manifest/replay (مع مسارات يحددها المشغلون)، ويحدد العناصر المفقودة/غير
   دليل/قابلة للكتابة عالميا، وينتج bundle JSON موقع للحوكمة
   (`artifacts/da/privilege_audit.json`), مغلقا فجوة اتمتة مراجعة الوصول.

**اين تتابع:**

- replay cache واستمرار المؤشرات تم انجازهما في DA-2. راجع التنفيذ في
  `crates/iroha_core/src/da/replay_cache.rs` (منطق cache) وتكامل Torii في
  `crates/iroha_torii/src/da/ingest.rs` الذي يمرر checks fingerprint عبر `/v1/da/ingest`.
- محاكاة streaming PDP/PoTR تمارس عبر harness proof-stream في
  `crates/sorafs_car/tests/sorafs_cli.rs`، وتغطي تدفقات طلب PoR/PDP/PoTR وسيناريوهات
  الفشل المشار اليها في نموذج التهديدات.
- نتائج capacity وrepair soak موجودة في
  `docs/source/sorafs/reports/sf2c_capacity_soak.md`، بينما مصفوفة soak Sumeragi
  الاوسع يتم تتبعها في `docs/source/sumeragi_soak_matrix.md` (مع نسخ محلية).
  هذه artefacts توثق التدريبات الطويلة المذكورة في سجل المخاطر.
- اتمتة المصالحة + privilege-audit موجودة في `docs/automation/da/README.md`
  والاوامر الجديدة `cargo xtask da-commitment-reconcile` /
  `cargo xtask da-privilege-audit`; استخدم المخارج الافتراضية تحت
  `artifacts/da/` عند ارفاق الادلة بحزم الحوكمة.

## ادلة المحاكاة ونمذجة QoS (2026-02)

لاغلاق متابعة DA-1 #3، قمنا بتكويد harness محاكاة PDP/PoTR حتمي تحت
`integration_tests/src/da/pdp_potr.rs` (مغطي بواسطة
`integration_tests/tests/da/pdp_potr_simulation.rs`). يقوم harness بتوزيع العقد
على ثلاث مناطق، ويحقن التقسيم/التواطؤ حسب احتمالات roadmap، ويتتبع تاخر PoTR،
ويغذي نموذج backlog للاصلاح يعكس ميزانية اصلاح طبقة hot. تشغيل السيناريو
الافتراضي (12 epochs، 18 تحديا PDP + نافذتان PoTR لكل epoch) انتج المقاييس
التالية:

<!-- BEGIN_DA_SIM_TABLE -->
<!-- AUTO-GENERATED by scripts/docs/render_da_threat_model_tables.py; do not edit manually. -->
| المقياس | القيمة | الملاحظات |
| --- | --- | --- |
| PDP failures detected | 48 / 49 (98.0%) | ما زالت partitions تطلق الاكتشاف؛ فشل واحد غير مكتشف ناتج عن jitter صادق. |
| PDP mean detection latency | 0.0 epochs | يتم اظهار الاعطال ضمن epoch الاصلي. |
| PoTR failures detected | 28 / 77 (36.4%) | يتم اطلاق الاكتشاف عند فقدان العقدة >=2 نوافذ PoTR، مما يترك معظم الاحداث في سجل المخاطر المتبقية. |
| PoTR mean detection latency | 2.0 epochs | يطابق عتبة تاخر مقدارها epochين مضمنة في تصعيد الارشفة. |
| Repair queue peak | 38 manifests | يرتفع backlog عندما تتكدس partitions اسرع من اربع اصلاحات متاحة لكل epoch. |
| Response latency p95 | 30,068 ms | يعكس نافذة تحدي 30 ثانية مع jitter +/-75 ms مطبق لاخذ عينات QoS. |
<!-- END_DA_SIM_TABLE -->

هذه المخرجات تقود الان نماذج لوحات DA وتفي بمعايير قبول "simulation harness +
QoS modelling" المشار اليها في roadmap.

الان توجد الاتمتة خلف
`cargo xtask da-threat-model-report [--out <path|->] [--seed <u64|0xhex>] [--config <path>]`,
والتي تستدعي harness المشترك وتصدر Norito JSON الى
`artifacts/da/threat_model_report.json` افتراضيا. تستخدم الوظائف الليلية هذا
الملف لتحديث المصفوفات في هذا المستند والتنبيه عند انحراف معدلات الاكتشاف
او طوابير الاصلاح او عينات QoS.

لتحديث الجدول اعلاه للوثائق، شغل `make docs-da-threat-model`، والذي يستدعي
`cargo xtask da-threat-model-report` ويعيد توليد
`docs/source/da/_generated/threat_model_report.json` ويعيد كتابة هذا القسم عبر
`scripts/docs/render_da_threat_model_tables.py`. يتم تحديث نسخة `docs/portal`
(`docs/portal/docs/da/threat-model.md`) في نفس التمرير لكي تبقى النسختان
متزامنتين.
