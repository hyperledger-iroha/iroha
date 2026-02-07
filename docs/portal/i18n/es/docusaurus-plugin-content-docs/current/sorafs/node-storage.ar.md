---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: almacenamiento de nodo
título: تصميم تخزين عقدة SoraFS
sidebar_label: تصميم تخزين العقدة
descripción: معمارية التخزين والحصص وخطافات دورة الحياة لعُقد Torii المستضيفة لبيانات SoraFS.
---

:::nota المصدر المعتمد
Utilice el botón `docs/source/sorafs/sorafs_node_storage.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة وثائق Sphinx القديمة.
:::

## تصميم تخزين عقدة SoraFS (مسودة)

توضح هذه المذكرة كيف يمكن لعقدة Iroha (Torii) الاشتراك في طبقة توفر بيانات
SoraFS Asegúrese de que el dispositivo esté encendido y apagado. وهي تكمل مواصفة descubrimiento
`sorafs_node_client_protocol.md` Accesorios para el SF-1b para el hogar
Otros productos y servicios
Número Torii. توجد التدريبات العملية للمشغلين في
[Runbook عمليات العقدة](./node-operations).

### الأهداف

- السماح لأي مُحقق أو عملية Iroha مساعدة بتعريض قرص فائض كمزوّد SoraFS دون التأثير
  على مسؤوليات دفتر الأستاذ الأساسية.
- إبقاء وحدة التخزين حتمية ومدفوعة بـ Norito: manifiestos وخطط القطع وجذور
  Prueba de recuperabilidad (PoR) وإعلانات المزوّد هي مصدر الحقيقة.
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

الوحدات الرئيسية:- **Puerta de enlace**: تعرض نقاط نهاية Norito HTTP لمقترحات pin y fetch للقطع y أخذ عينات PoR y التليمترية. تحقق من حمولات Norito وتوجه الطلبات إلى مخزن القطع. Utilice el código HTTP Torii para ejecutar el comando.
- **Registro de PIN**: muestra los manifiestos de los archivos `iroha_data_model::sorafs` e `iroha_core`. عند قبول manifest يسجل السجل digest للـ manifest وdigest لخطة القطع وجذر PoR وأعلام قدرات المزوّد.
- **Almacenamiento de fragmentos**: تنفيذ `ChunkStore` على القرص يستقبل manifests موقعة، ويبني خطط القطع باستخدام `ChunkProfile::DEFAULT`، ويخزن القطع في تخطيط حتمي. ترتبط كل قطعة ببصمة محتوى وبيانات PoR وصفية كي يمكن إعادة التحقق دون قراءة الملف بالكامل.
- **Cuota/Programador**: يفرض حدود المشغل (أقصى بايتات قرص، أقصى pins معلقة, أقصى عمليات buscar متوازية, TTL للقطع) وينسق IO حتى لا تتأثر مهام دفتر الأستاذ. El programador del programador necesita PoR y el procesador de CPU.

### الإعداد

Aquí está el artículo `iroha_config`:

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
```- `enabled`: مفتاح مشاركة. عند false تعيد البوابة 503 لنقاط نهاية التخزين ولا تعلن العقدة نفسها في descubrimiento.
- `data_dir`: المجلد الجذري لبيانات القطع وأشجار PoR وتليمترية fetch. Nombre `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes`: حد صارم لبيانات القطع المثبتة. ترفض مهمة خلفية pins الجديدة عند بلوغ الحد.
- `max_parallel_fetches`: سقف التوازي الذي يفرضه planificador لتحقيق توازن بين IO القرص وحمل المُحقق.
- `max_pins`: أقصى عدد من pins للـ manifest قبل تطبيق الإخلاء/الضغط الخلفي.
- `por_sample_interval_secs`: وتيرة مهام أخذ عينات PoR التلقائية. كل مهمة تأخذ `N` ورقة (قابلة للضبط لكل manifest) وتصدر أحداث تليمترية. Esta es la versión `N` que contiene los metadatos `profile.sample_multiplier` (la versión `1-4`). يمكن أن تكون القيمة رقما/نصا واحدا أو كائنا مع anula la لكل ملف تعريف، مثل `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts`: بنية يستخدمها مولد ads لملء حقول `ProviderAdvertV1` (puntero de participación, إشارات QoS, temas). إذا تم حذفها تستخدم العقدة القيم الافتراضية من سجل الحوكمة.

توصيلات الإعداد:

- `[sorafs.storage]` entre `iroha_config` y `SorafsStorage` y entre otros.
- تقوم `iroha_core` و`iroha_torii` بتمرير إعداد التخزين إلى builder الخاص بالبوابة ومخزن القطع عند البدء.
- Esta opción anula للتطوير/الاختبار (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), لكن نشر الإنتاج يجب أن يعتمد على ملف الإعداد.

### أدوات CLIUtilice el servidor Torii HTTP y el crate `sorafs_node` y el CLI para instalar el servidor. 【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` contiene el manifiesto `.to` contiene la carga útil Norito. Puede utilizar la fragmentación del manifiesto, el resumen y el resumen de archivos JSON. باسم `chunk_fetch_specs` حتى تمكن الأدوات اللاحقة من التحقق من التخطيط.
- `export` Manifiesto de manifiesto y carga útil Manifiesto/carga útil (manifiesto JSON) para accesorios de instalación البيئات.

Esta es la salida estándar JSON Norito que contiene scripts. تغطي اختبارات التكامل الـ CLI لضمان أن manifiestos y cargas útiles تعمل ida y vuelta بشكل صحيح قبل وصول واجهات Torii.【crates/sorafs_node/tests/cli.rs:1】> تكافؤ HTTP
>
> تعرض بوابة Torii الآن مساعدات قراءة فقط مدعومة بنفس `NodeHandle`:
>
> - `GET /v1/sorafs/storage/manifest/{manifest_id_hex}` — Manifestado Norito (base64) en resumen/metadatos.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v1/sorafs/storage/plan/{manifest_id_hex}` — يعيد خطة القطع الحتمية JSON (`chunk_fetch_specs`) en sentido descendente.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> تعكس هذه النقاط خرج CLI بحيث يمكن للخطوط التحويل من scripts محلية إلى فحوصات HTTP دون تغيير المحللات.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### دورة حياة العقدة1. **البدء**:
   - عند تفعيل التخزين تهيئ العقدة مخزن القطع بالمجلد والسعة المهيأة. يشمل ذلك التحقق أو إنشاء قاعدة بيانات PoR للـ manifest وإعادة تشغيل manifests المثبتة لتسخين الكاش.
   - Instale el archivo SoraFS (es decir, Norito JSON POST/GET para pin, búsqueda y PoR y التليمترية).
   - تشغيل عامل أخذ عينات PoR ومراقب الحصص.
2. **Descubrimiento/Anuncios**:
   - توليد مستندات `ProviderAdvertV1` باستخدام السعة/الصحة الحالية, وتوقيعها بالمفتاح المعتمد من المجلس، ونشرها عبر قناة descubrimiento. Utilice el cable `profile_aliases` para limpiar y limpiar el aparato.
3. **pin**:
   - تستقبل البوابة manifiesto موقعا (يشمل خطة القطع وجذر PoR وتواقيع المجلس). تتحقق من قائمة alias (`sorafs.sf1@1.0.0` مطلوب) y تؤكد أن خطة القطع تطابق بيانات manifiesto الوصفية.
   - تحقق من الحصص. إذا كانت حدود السعة/pin ستجاوز، فاستجب بخطأ سياسة (Norito منظم).
   - بث بيانات القطع إلى `ChunkStore` مع التحقق من resúmenes أثناء الإدخال. حدّث أشجار PoR y metadatos del manifiesto del registro.
4. **تدفق buscar**:
   - تقديم طلبات نطاق القطع من القرص. El programador `max_parallel_fetches` y el `429` están disponibles.
   - إصدار تليمترية منظمة (Norito JSON) تضمن زمن الاستجابة والبايتات المخدومة وعدادات الأخطاء للمراقبة اللاحقة.
5. **أخذ عينات PoR**:
   - يختار العامل manifiesta بنسبة للوزن (مثل البايتات المخزنة) ويجري أخذ عينات حتميا باستخدام شجرة PoR الخاصة بمخزن القطع.- حفظ النتائج لأغراض تدقيق الحوكمة وإدراج الملخصات في anuncios الخاصة بالمزوّد/نقاط التليمترية.
6. **الإخلاء/تطبيق الحصص**:
   - عند بلوغ السعة ترفض العقدة pines الجديدة افتراضيا. يمكن للمشغلين تكوين سياسات إخلاء (مثل TTL و LRU) عند توافق نموذج الحوكمة؛ حاليا يفترض التصميم حصصا صارمة y desanclar يطلقها المشغل.

### تكامل إعلان السعة والجدولة- تعيد Torii تمرير تحديثات `CapacityDeclarationRecord` من `/v1/sorafs/capacity/declare` إلى `CapacityManager` المضمن، بحيث يبني كل عقدة عرضا في الذاكرة لتخصيصات chunker/lane الملتزم بها. يكشف المدير لقطات للتليمترية (`GET /v1/sorafs/capacity/state`) ويفرض حجوزات لكل ملف تعريف أو lane قبل قبول أوامر جديدة.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- El punto final `/v1/sorafs/capacity/schedule` incluye `ReplicationOrderV1`. عندما يستهدف الأمر المزوّد المحلي، يتحقق المدير من التكرار، ويفحص سعة chunker/lane، ويحجز الشريحة، ويعيد `ReplicationPlan` يصف السعة المتبقية لتتمكن أدوات من متابعة الإدخال. يتم الإقرار بالأوامر الخاصة بمزوّدين آخرين باستجابة `ignored` لتسهيل سير العمل متعدد المشغلين.【crates/iroha_torii/src/routing.rs:4845】
- تقوم ganchos الإكمال (مثل ما يحدث بعد نجاح الإدخال) باستدعاء `POST /v1/sorafs/capacity/complete` لإطلاق الحجوزات عبر `CapacityManager::complete_order`. يتضمن الرد لقطة `ReplicationRelease` (الإجماليات المتبقية وبقايا fragmentador/carril) حتى تمكن أدوات orquestación من جدولة الأمر التالي دون sondeo. 【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- مكن تعديل `TelemetryAccumulator` المضمن عبر `NodeHandle::update_telemetry`, مما يسمح لعمال الخلفية عينات PoR/uptime y النهاية اشتقاق حمولات `CapacityTelemetryV1` القياسية دون لمس internals الـ Scheduler.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】### التكاملات والعمل المستقبلي

- **الحوكمة**: توسيع `sorafs_pin_registry_tracker.md` بتليمترية التخزين (معدل نجاح PoR، استغلال القرص). يمكن لسياسات القبول اشتراط سعة دنيا أو حد أدنى لنجاح PoR قبل قبول anuncios.
- **SDKs للعملاء**: تعريض إعداد التخزين الجديد (حدود القرص، alias) لتمكين أدوات الإدارة من bootstrap العقد برمجيا.
- **التليمترية**: دمج مع مكدس المقاييس الحالي (Prometheus / OpenTelemetry) حتى تظهر مقاييس التخزين في لوحات المراقبة.
- **الأمان**: تشغيل وحدة التخزين داخل مجموعة مهام async مخصصة مع back-pression, والنظر في sandboxing لقراءات القطع عبر io_uring أو مجموعات tokio المحدودة لمنع العملاء الخبيثين من استنزاف الموارد.

يحافظ هذا التصميم على اختيارية وحدة التخزين وحتميتها، ويمنح المشغلين المفاتيح اللازمة للمشاركة في طبقة توفر بيانات SoraFS. Los dispositivos de conexión `iroha_config`, `iroha_core`, `iroha_torii` y Norito, están conectados a la red. أدوات إعلانات المزوّد.