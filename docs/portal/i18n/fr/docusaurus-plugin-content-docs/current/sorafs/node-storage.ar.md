---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-storage.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : stockage de nœud
titre : تصميم تخزين عقدة SoraFS
sidebar_label : تصميم تخزين العقدة
description: معمارية التخزين والحصص وخطافات دورة الحياة لعُقد Torii المستضيفة لبيانات SoraFS.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/sorafs_node_storage.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف مجموعة وثائق Sphinx القديمة.
:::

## تصميم تخزين عقدة SoraFS (مسودة)

توضح هذه المذكرة كيف يمكن لعقدة Iroha (Torii) الاشتراك في طبقة توفر بيانات
SoraFS est situé à proximité de la porte d'entrée. وهي تكمل مواصفة découverte
`sorafs_node_client_protocol.md` luminaires pour SF-1b pour montage mural
التخزين وضوابط الموارد وتوصيلات الإعداد التي يجب أن تصل إلى العقدة ومسارات
Pour Torii. توجد التدريبات العملية للمشغلين في
[Runbook عمليات العقدة](./node-operations).

### الأهداف

- السماح لأي مُحقق أو عملية Iroha مساعدة بتعريض قرص فائض كمزوّد SoraFS دون التأثير
  على مسؤوليات دفتر الأستاذ الأساسية.
- إبقاء وحدة التخزين حتمية ومدفوعة byـ Norito: manifeste et وخطط القطع وجذور
  Preuve de récupérabilité (PoR) et preuve de récupérabilité (PoR).
- Vous pouvez utiliser la broche pour récupérer la broche et récupérer la broche.
- إرجاع الصحة/التليمترية (عينات PoR, زمن جلب القطع، ضغط القرص) إلى الحوكمة والعملاء.

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

Description de la situation :- **Passerelle** : Utilisez la broche Norito HTTP pour récupérer la broche et récupérer le PoR et le PoR. تتحقق من حمولات Norito وتوجه الطلبات إلى مخزن القطع. Vous devez utiliser le protocole HTTP pour Torii pour votre connexion Internet.
- **Pin Registry** : les manifestes se manifestent par `iroha_data_model::sorafs` et `iroha_core`. عند قبول manifest يسجل السجل digest للـ manifest وdigest لخطة القطع وجذر PoR وأعلام قدرات المزوّد.
- **Chunk Storage** : تنفيذ `ChunkStore` pour les manifestes et `ChunkProfile::DEFAULT`, ويخزن القطع في تخطيط حتمي. ترتبط قطعة ببصمة محتوى وبيانات PoR وصفية كي يمكن إعادة التحقق دون قراءة الملف بالكامل.
- **Quota/Planificateur** : يفرض حدود المشغل (أقصى بايتات قرص، أقصى pins معلقة، أقصى عمليات fetch متوازية، TTL للقطع) وينسق IO حتى لا تتأثر مهام دفتر الأستاذ. Il s'agit d'un planificateur PoR et d'un planificateur qui utilise le CPU.

### الإعداد

أضف قسما جديدا إلى `iroha_config` :

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
```- `enabled` : مفتاح مشاركة. عند false تعيد البوابة 503 لنقاط نهاية التخزين ولا تعلن العقدة نفسها في découverte.
- `data_dir` : Récupération de PoR et récupération de PoR. Article `<iroha.data_dir>/sorafs`.
- `max_capacity_bytes` : حد صارم لبيانات القطع المثبتة. ترفض مهمة خلفية pins الجديدة عند بلوغ الحد.
- `max_parallel_fetches` : Planificateur de planification pour les IO et les applications.
- `max_pins` : Il s'agit d'une broche pour le manifeste dans le cadre d'un manifeste.
- `por_sample_interval_secs` : وتيرة مهام أخذ عينات PoR التلقائية. Il s'agit du fichier `N` (pour le manifeste) et du manifeste. Vous pouvez utiliser `N` pour utiliser les métadonnées `profile.sample_multiplier` (voir `1-4`). Il s'agit d'un système de remplacement/de conversion et de remplacement de remplacement par `{"default":2,"sorafs.sf2@1.0.0":3}`.
- `adverts` : les publicités sont disponibles pour `ProviderAdvertV1` (pointeur de mise, pour les sujets QoS). Il s'agit d'un élément important de votre système d'exploitation.

توصيلات الإعداد:

- `[sorafs.storage]` est compatible avec `iroha_config` et `SorafsStorage` et est également compatible avec votre appareil.
- تقوم `iroha_core` و`iroha_torii` بتمرير إعداد التخزين إلى builder الخاص بالبوابة ومخزن القطع عند البدء.
- توجد remplace للتطوير/الاختبار (`SORAFS_STORAGE_*`, `SORAFS_STORAGE_PIN_*`), لكن نشر الإنتاج يجب أن يعتمد على ملف الإعداد.

### أدوات CLIConnectez-vous avec Torii HTTP pour votre crate `sorafs_node` et CLI avec votre compte من أتمتة تمارين الإدخال/التصدير ضد الواجهة الخلفية الدائمة.【crates/sorafs_node/src/bin/sorafs-node.rs:1】

```bash
cargo run -p sorafs_node --bin sorafs-node ingest \
  --data-dir ./storage/sorafs \
  --manifest ./fixtures/manifest.to \
  --payload ./fixtures/payload.bin \
  --plan-json-out ./plan.json
```

- `ingest` contient le manifeste `.to` et Norito pour la charge utile. Vous pouvez également utiliser le chunking pour le manifeste, ainsi que le digest et les autres JSON est basé sur `chunk_fetch_specs`.
- `export` يقبل معرف manifest ويكتب manifeste/payload المخزن إلى القرص (مع خطة JSON اختيارية) لضمان قابلية إعادة إنتاج luminaires عبر البيئات.

Vous pouvez utiliser Norito JSON pour la sortie standard, ainsi que les scripts. Utilisez la CLI pour les manifestes et les charges utiles, ainsi que les allers-retours pour les manifestes et les charges utiles. Torii.【crates/sorafs_node/tests/cli.rs:1】> Utiliser HTTP
>
> تعرض بوابة Torii الآن مساعدات قراءة فقط مدعومة بنفس `NodeHandle`:
>
> - `GET /v2/sorafs/storage/manifest/{manifest_id_hex}` — Le manifeste Norito est (base64) avec digest/metadata.【crates/iroha_torii/src/sorafs/api.rs:1207】
> - `GET /v2/sorafs/storage/plan/{manifest_id_hex}` — JSON (`chunk_fetch_specs`) en aval.【crates/iroha_torii/src/sorafs/api.rs:1259】
>
> Utiliser la CLI pour les scripts et les scripts HTTP pour les applications HTTP المحللات.【crates/iroha_torii/src/sorafs/api.rs:1207】【crates/iroha_torii/src/sorafs/api.rs:1259】

### دورة حياة العقدة1. **البدء** :
   - عند تفعيل التخزين تهيئ العقدة مخزن القطع بالمجلد والسعة المهيأة. يشمل ذلك التحقق أو إنشاء قاعدة بيانات PoR للـ manifest وإعادة تشغيل manifeste المثبتة لتسخين الكاش.
   - Utilisez le code SoraFS (code Norito JSON POST/GET pour la récupération de broches et le PoR).
   - تشغيل عامل أخذ عينات PoR ومراقب الحصص.
2. **Découverte / Annonces** :
   - توليد مستندات `ProviderAdvertV1` باستخدام السعة/الصحة الحالية، وتوقيعها بالمفتاح المعتمد من المجلس، ونشرها عبر قناة découverte. استخدم قائمة `profile_aliases` لإبقاء المقابض القياسية والقديمة متاحة.
3. **PIN PIN** :
   - تستقبل البوابة manifeste موقعا (يشمل خطة القطع وجذر PoR وتواقيع المجلس). Vous pouvez utiliser les alias (`sorafs.sf1@1.0.0` مطلوب) et les alias sont également des manifestes.
   - تحقق من الحصص. Utilisez le connecteur/pin pour connecter le connecteur (Norito).
   - بيانات القطع إلى `ChunkStore` مع التحقق من digests أثناء الإدخال. Il s'agit du PoR et des métadonnées du manifeste dans le registre.
4. **Récupérer** :
   - تقديم طلبات نطاق القطع من القرص. Vous pouvez utiliser le planificateur `max_parallel_fetches` et `429`.
   - إصدار تليمترية منظمة (Norito JSON) تتضمن زمن الاستجابة والبايتات المخدومة وعدادات الأخطاء للمراقبة اللاحقة.
5. **أخذ عينات PoR** :
   - يختار العامل manifeste بنسبة للوزن (مثل البايتات المخزنة) et ويجري أخذ عينات حتميا باستخدام شجرة PoR الخاصة بمخزن القطع.- حفظ النتائج لأغراض تدقيق الحوكمة وإدراج الملخصات في adverts الخاصة بالمزوّد/نقاط التليمترية.
6. **الإخلاء/تطبيق الحصص** :
   - عند بلوغ السعة ترفض العقدة pins الجديدة افتراضيا. يمكن للمشغلين تكوين سياسات إخلاء (مثل TTL وLRU) عند توافق نموذج الحوكمة؛ حاليا يفترض التصميم حصصا صارمة وعمليات désépingler يطلقها المشغل.

### تكامل إعلان السعة والجدولة- تعيد Torii تمرير تحديثات `CapacityDeclarationRecord` من `/v2/sorafs/capacity/declare` إلى `CapacityManager` المضمن، بحيث يبني كل عقدة Il s'agit d'un chunker/lane similaire à celui-ci. يكشف المدير لقطات lecture seule للتليمترية (`GET /v2/sorafs/capacity/state`) ويفرض حجوزات لكل ملف تعريف أو lane قبل قبول أوامر Lire.【crates/sorafs_node/src/capacity.rs:1】【crates/sorafs_node/src/lib.rs:60】
- Le point de terminaison `/v2/sorafs/capacity/schedule` est associé à `ReplicationOrderV1`. Il s'agit d'un chunker/lane ou d'un chunker/lane `ReplicationPlan` يصف السعة المتبقية لتتمكن أدوات orchestration من متابعة الإدخال. يتم الإقرار بالأوامر الخاصة بمزوّدين آخرين باستجابة `ignored` لتسهيل سير العمل متعدد مشغلين.【crates/iroha_torii/src/routing.rs:4845】
- Crochets pour crochets (pour crochets) pour `POST /v2/sorafs/capacity/complete` pour `CapacityManager::complete_order`. L'orchestration `ReplicationRelease` (pour le chunker/lane) est utilisée pour l'orchestration pour le sondage. سيُوصل هذا بخط أنابيب مخزن القطع عند اكتمال منطق Fichier.【crates/iroha_torii/src/routing.rs:4885】【crates/sorafs_node/src/capacity.rs:90】
- يمكن تعديل `TelemetryAccumulator` المضمن عبر `NodeHandle::update_telemetry`, مما يسمح لعمال الخلفية بتسجيل عينات PoR/uptime وفي النهاية Utilisez le planificateur `CapacityTelemetryV1` pour les composants internes du planificateur.【crates/sorafs_node/src/lib.rs:142】【crates/sorafs_node/src/telemetry.rs:1】### التكاملات والعمل المستقبلي

- **الحوكمة** : توسيع `sorafs_pin_registry_tracker.md` بتليمترية التخزين (معدل نجاح PoR, استغلال القرص). Il y a des publicités sur les publicités PoR.
- **SDK pour ** : Utiliser le bootstrap pour le bootstrap (alias de l'utilisateur).
- **التليمترية** : دمج مع مكدس المقاييس الحالي (Prometheus / OpenTelemetry) حتى تظهر مقاييس التخزين في لوحات المراقبة.
- **الأمان** : La fonction async est utilisée pour la contre-pression et le sandboxing. io_uring est une entreprise de Tokio qui est en train de travailler avec elle.

يحافظ هذا التصميم على اختيارية وحدة التخزين وحتميتها، ويمنح المشغلين المفاتيح اللازمة للمشاركة في طبقة توفر بيانات SoraFS. سيتطلب التنفيذ تغييرات عبر `iroha_config` و`iroha_core` و`iroha_torii` وبوابة Norito، بالإضافة إلى أدوات إعلانات المزوّد.