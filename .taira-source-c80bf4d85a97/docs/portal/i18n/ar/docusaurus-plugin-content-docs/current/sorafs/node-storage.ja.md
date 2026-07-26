---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/sorafs/node-storage.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 54d5fd7f77b39007f99a6d80ea15a373dd35a159cb8ff9021f2c1003b87f86f2
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---


---
id: node-storage
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/node-storage.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note المصدر المعتمد
تعكس هذه الصفحة `docs/source/sorafs/sorafs_node_storage.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة وثائق Sphinx القديمة.
:::

## تصميم تخزين عقدة SoraFS (مسودة)

توضح هذه المذكرة كيف يمكن لعقدة Iroha (Torii) الاشتراك في طبقة توفر بيانات
SoraFS وتخصيص جزء من القرص المحلي لتخزين وخدمة القطع. وهي تكمل مواصفة discovery
`sorafs_node_client_protocol.md` وأعمال fixtures لـ SF-1b عبر تفصيل معمارية جانب
التخزين وضوابط الموارد وتوصيلات الإعداد التي يجب أن تصل إلى العقدة ومسارات
بوابة Torii. توجد التدريبات العملية للمشغلين في
[Runbook عمليات العقدة](./node-operations).

### الأهداف

- السماح لأي مُحقق أو عملية Iroha مساعدة بتعريض قرص فائض كمزوّد SoraFS دون التأثير
  على مسؤوليات دفتر الأستاذ الأساسية.
- إبقاء وحدة التخزين حتمية ومدفوعة بـ Norito: manifests وخطط القطع وجذور
  Proof-of-Retrievability (PoR) وإعلانات المزوّد هي مصدر الحقيقة.
- فرض حصص يحددها المشغل حتى لا تستنزف العقدة مواردها بقبول عدد كبير من طلبات pin أو fetch.
- إرجاع الصحة/التليمترية (عينات PoR، زمن جلب القطع، ضغط القرص) إلى الحوكمة والعملاء.

### المعمارية عالية المستوى

```
┌──────────────────────────────────────────────────────────────────────┐
│                         Iroha/Torii Node                             │
│                                                                      │
│  ┌──────────────┐      ┌────────────────────┐                        │
│  │  Torii APIs  │◀────▶│   SoraFS Gateway   │◀───────────────┐       │
│  └──────────────┘      │ (Norito endpoints) │                │       │
│                        └────────┬───────────┘                │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Pin Registry   │◀───── manifests   │       │
│                        │ (State / DB)    │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Chunk Storage  │◀──── chunk plans  │       │
│                        │  (ChunkStore)   │                   │       │
│                        └────────┬────────┘                   │       │
│                                 │                            │       │
│                        ┌────────▼────────┐                   │       │
│                        │  Disk Quota/IO  │─Pin/serve chunks─▶│ Fetch │
│                        │  Scheduler      │                   │ Clients│
│                        └─────────────────┘                   │       │
│                                                                      │
└──────────────────────────────────────────────────────────────────────┘
```

الوحدات الرئيسية:

- **Gateway**: تعرض نقاط نهاية Norito HTTP لمقترحات pin وطلبات fetch للقطع وأخذ عينات PoR والتليمترية. تتحقق من حمولات Norito وتوجه الطلبات إلى مخزن القطع. تعيد استخدام مكدس HTTP الخاص بـ Torii لتجنب خدمة جديدة.
- **Pin Registry**: حالة تثبيت manifests المُسجلة في `iroha_data_model::sorafs` و`iroha_core`. عند قبول manifest يسجل السجل digest للـ manifest وdigest لخطة القطع وجذر PoR وأعلام قدرات المزوّد.
- **Chunk Storage**: تنفيذ `ChunkStore` على القرص يستقبل manifests موقعة، ويبني خطط القطع باستخدام `ChunkProfile::DEFAULT`، ويخزن القطع في تخطيط حتمي. ترتبط كل قطعة ببصمة محتوى وبيانات PoR وصفية كي يمكن إعادة التحقق دون قراءة الملف بالكامل.
- **Quota/Scheduler**: يفرض حدود المشغل (أقصى بايتات قرص، أقصى pins معلقة، أقصى عمليات fetch متوازية، TTL للقطع) وينسق IO حتى لا تتأثر مهام دفتر الأستاذ. كما يخدم scheduler إثباتات PoR وطلبات أخذ العينات ضمن ميزانية CPU محددة.

### الإعداد

أضف قسما جديدا إلى `iroha_config`:

```toml
[sorafs.storage]
enabled = false
data_dir = "/var/lib/iroha/sorafs"
max_capacity_bytes = "100 GiB"
max_parallel_fetches = 32
max_pins = 10_000
por_sample_interval_secs = 600
alias = "tenant.alpha"            # وسم اختياري مقروء
adverts:
  stake_pointer = "stake.pool.v1:0x1234"
  availability = "hot"
  max_latency_ms = 500
  topics = ["sorafs.sf1.primary:global"]
```

- `enabled`: مفتاح مشاركة. عند false تعيد البوابة 503 لنقاط نهاية التخزين ولا تعلن العقدة نفسها في discovery.
- `data_dir`: المجلد الجذري لبيانات القطع وأشجار PoR وتليمترية fetch. الافتراضي `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: حد صارم لبيانات القطع المثبتة. ترفض مهمة خلفية pins الجديدة عند بلوغ الحد.
- `max_parallel_fetches`: سقف التوازي الذي يفرضه scheduler لتحقيق توازن بين IO القرص وحمل المُحقق.
- `max_pins`: أقصى عدد من pins للـ manifest قبل تطبيق الإخلاء/الضغط الخلفي.
- `por_sample_interval_secs`: وتيرة مهام أخذ عينات PoR التلقائية. كل مهمة تأخذ `N` ورقة (قابلة للضبط لكل manifest) وتصدر أحداث تليمترية. يمكن للحوكمة توسيع `N` بشكل حتمي عبر مفتاح metadata `profile.sample_multiplier` (عدد صحيح `1-4`). يمكن أن تكون القيمة رقما/نصا واحدا أو كائنا مع overrides لكل ملف تعريف، مثل `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: بنية يستخدمها مولد adverts لملء حقول `ProviderAdvertV1` (stake pointer، إشارات QoS، topics). إذا تم حذفها تستخدم العقدة القيم الافتراضية من سجل الحوكمة.

توصيلات الإعداد:

- `[sorafs.storage]` معرف في `iroha_config` كـ `SorafsStorage` ويتم تحميله من ملف إعداد العقدة.
- تقوم `iroha_core` و`iroha_torii` بتمرير إعداد التخزين إلى builder الخاص بالبوابة ومخزن القطع عند البدء.
- توجد overrides للتطوير/الاختبار (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`)، لكن نشر الإنتاج يجب أن يعتمد على ملف الإعداد.

### أدوات CLI

بينما ما زالت واجهة Torii HTTP قيد التوصيل، يشحن crate `sorafs_node` واجهة CLI خفيفة حتى يتمكن المشغلون من أتمتة تمارين الإدخال/التصدير ضد الواجهة الخلفية الدائمة.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` يتوقع ملف manifest `.to` مشفر بـ Norito مع بايتات payload المطابقة. يعيد بناء خطة القطع من ملف تعريف chunking للـ manifest، ويفرض تطابق digest، ويحفظ ملفات القطع، ويصدر اختياريا كتلة JSON باسم `chunk_fetch_specs` حتى تتمكن الأدوات اللاحقة من التحقق من التخطيط.
- `export` يقبل معرف manifest ويكتب manifest/payload المخزن إلى القرص (مع خطة JSON اختيارية) لضمان قابلية إعادة إنتاج fixtures عبر البيئات.

يطبع الأمران ملخص Norito JSON إلى stdout، مما يسهل استخدامه في scripts. تغطي اختبارات التكامل الـ CLI لضمان أن manifests وpayloads تعمل round-trip بشكل صحيح قبل وصول واجهات Torii.【crates/sorafs_node/tests/cli.rs:1】

> تكافؤ HTTP
>
> تعرض بوابة Torii الآن مساعدات قراءة فقط مدعومة بنفس `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — يعيد manifest Norito المخزن (base64) مع digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — يعيد خطة القطع الحتمية JSON (`chunk_fetch_specs`) لأدوات downstream.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> تعكس هذه النقاط خرج CLI بحيث يمكن للخطوط التحويل من scripts محلية إلى فحوصات HTTP دون تغيير المحللات.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### دورة حياة العقدة

1. **البدء**:
   - عند تفعيل التخزين تهيئ العقدة مخزن القطع بالمجلد والسعة المهيأة. يشمل ذلك التحقق أو إنشاء قاعدة بيانات PoR للـ manifest وإعادة تشغيل manifests المثبتة لتسخين الكاش.
   - تسجيل مسارات بوابة SoraFS (نقاط نهاية Norito JSON POST/GET لـ pin وfetch وأخذ عينات PoR والتليمترية).
   - تشغيل عامل أخذ عينات PoR ومراقب الحصص.
2. **Discovery / Adverts**:
   - توليد مستندات `ProviderAdvertV1` باستخدام السعة/الصحة الحالية، وتوقيعها بالمفتاح المعتمد من المجلس، ونشرها عبر قناة discovery. استخدم قائمة `profile_aliases` لإبقاء المقابض القياسية والقديمة متاحة.
3. **تدفق pin**:
   - تستقبل البوابة manifest موقعا (يشمل خطة القطع وجذر PoR وتواقيع المجلس). تتحقق من قائمة aliases (`sorafs.sf1@1.0.0` مطلوب) وتؤكد أن خطة القطع تطابق بيانات manifest الوصفية.
   - تحقق من الحصص. إذا كانت حدود السعة/pin ستتجاوز، فاستجب بخطأ سياسة (Norito منظم).
   - بث بيانات القطع إلى `ChunkStore` مع التحقق من digests أثناء الإدخال. حدّث أشجار PoR وخزّن metadata للـ manifest في registry.
4. **تدفق fetch**:
   - تقديم طلبات نطاق القطع من القرص. يفرض scheduler `max_parallel_fetches` ويعيد `429` عند التشبع.
   - إصدار تليمترية منظمة (Norito JSON) تتضمن زمن الاستجابة والبايتات المخدومة وعدادات الأخطاء للمراقبة اللاحقة.
5. **أخذ عينات PoR**:
   - يختار العامل manifests بنسبة للوزن (مثل البايتات المخزنة) ويجري أخذ عينات حتميا باستخدام شجرة PoR الخاصة بمخزن القطع.
   - حفظ النتائج لأغراض تدقيق الحوكمة وإدراج الملخصات في adverts الخاصة بالمزوّد/نقاط التليمترية.
6. **الإخلاء/تطبيق الحصص**:
   - عند بلوغ السعة ترفض العقدة pins الجديدة افتراضيا. يمكن للمشغلين تكوين سياسات إخلاء (مثل TTL وLRU) عند توافق نموذج الحوكمة؛ حاليا يفترض التصميم حصصا صارمة وعمليات unpin يطلقها المشغل.

### تكامل إعلان السعة والجدولة

- تعيد Torii تمرير تحديثات `CapacityDeclarationRecord` من `/v1/sorafs/capacity/declare` إلى `CapacityManager` المضمن، بحيث يبني كل عقدة عرضا في الذاكرة لتخصيصات chunker/lane الملتزم بها. يكشف المدير لقطات read-only للتليمترية (`GET /v1/sorafs/capacity/state`) ويفرض حجوزات لكل ملف تعريف أو lane قبل قبول أوامر جديدة.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- يقبل endpoint `/v1/sorafs/capacity/schedule` حمولات `ReplicationOrderV1` الصادرة عن الحوكمة. عندما يستهدف الأمر المزوّد المحلي، يتحقق المدير من التكرار، ويفحص سعة chunker/lane، ويحجز الشريحة، ويعيد `ReplicationPlan` يصف السعة المتبقية لتتمكن أدوات orchestration من متابعة الإدخال. يتم الإقرار بالأوامر الخاصة بمزوّدين آخرين باستجابة `ignored` لتسهيل سير العمل متعدد المشغلين.【crates/iroha_torii/src/routing.rs:4845】
- تقوم hooks الإكمال (مثل ما يحدث بعد نجاح الإدخال) باستدعاء `POST /v1/sorafs/capacity/complete` لإطلاق الحجوزات عبر `CapacityManager::complete_order`. يتضمن الرد لقطة `ReplicationRelease` (الإجماليات المتبقية وبقايا chunker/lane) حتى تتمكن أدوات orchestration من جدولة الأمر التالي دون polling. سيُوصل هذا بخط أنابيب مخزن القطع عند اكتمال منطق الإدخال.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- يمكن تعديل `TelemetryAccumulator` المضمن عبر `NodeHandle::update_telemetry`، مما يسمح لعمال الخلفية بتسجيل عينات PoR/uptime وفي النهاية اشتقاق حمولات `CapacityTelemetryV1` القياسية دون لمس internals الـ scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】

### التكاملات والعمل المستقبلي

- **الحوكمة**: توسيع `sorafs_pin_registry_tracker.md` بتليمترية التخزين (معدل نجاح PoR، استغلال القرص). يمكن لسياسات القبول اشتراط سعة دنيا أو حد أدنى لنجاح PoR قبل قبول adverts.
- **SDKs للعملاء**: تعريض إعداد التخزين الجديد (حدود القرص، alias) لتمكين أدوات الإدارة من bootstrap العقد برمجيا.
- **التليمترية**: دمج مع مكدس المقاييس الحالي (Prometheus / OpenTelemetry) حتى تظهر مقاييس التخزين في لوحات المراقبة.
- **الأمان**: تشغيل وحدة التخزين داخل مجموعة مهام async مخصصة مع back-pressure، والنظر في sandboxing لقراءات القطع عبر io_uring أو مجموعات tokio المحدودة لمنع العملاء الخبيثين من استنزاف الموارد.

يحافظ هذا التصميم على اختيارية وحدة التخزين وحتميتها، ويمنح المشغلين المفاتيح اللازمة للمشاركة في طبقة توفر بيانات SoraFS. سيتطلب التنفيذ تغييرات عبر `iroha_config` و`iroha_core` و`iroha_torii` وبوابة Norito، بالإضافة إلى أدوات إعلانات المزوّد.
