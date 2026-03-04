---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/node-plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : plan de nœud
titre : خطة تنفيذ عقدة SoraFS
sidebar_label : خطة تنفيذ العقدة
description: تحويل خارطة طريق تخزين SF-3 إلى عمل هندسي قابل للتنفيذ مع معالم ومهام وتغطية اختبارات.
---

:::note المصدر المعتمد
Il s'agit de la référence `docs/source/sorafs/sorafs_node_plan.md`. احرص على إبقاء النسختين متزامنتين إلى أن يتم إيقاف وثائق Sphinx القديمة.
:::

Le SF-3 est un boîtier de caisse pour le `sorafs-node` et un Iroha/Torii pour le mettre en valeur. SoraFS. استخدم هذه الخطة بجانب [دليل تخزين العقدة](node-storage.md)، et [سياسة قبول الموفّرين](provider-admission-policy.md)، و[خارطة طريق سوق سعة التخزين](storage-capacity-marketplace.md) عند ترتيب التسليمات.

## النطاق المستهدف (المرحلة M1)

1. **تكامل مخزن القطع.** تغليف `sorafs_car::ChunkStore` pour le manifeste et le PoR في مجلد البيانات المهيأ.
2. ** نقاط نهاية البوابة.** توفير نقاط نهاية HTTP pour Norito pour la broche et pour le PoR وتليمترية التخزين ضمن عملية Torii.
3. **توصيل الإعدادات.** إضافة بنية إعداد `SoraFsStorage` (مفتاح التفعيل، السعة، المجلدات، حدود التوازي) Il s'agit de `iroha_config` et `iroha_core` et `iroha_torii`.
4. **الحصص/الجدولة.** فرض حدود القرص/التوازي التي يحددها المشغل ووضع الطلبات في طوابير مع contre-pression.
5. **التليمترية.** إصدار مقاييس/سجلات لنجاح pin وزمن جلب القطع واستغلال السعة ونتائج عينات PoR.

## تفصيل العمل

### A. بنية الـ crate والوحدات| المهمة | المالك | الملاحظات |
|------|--------|---------------|
| Utilisez `crates/sorafs_node` pour les éléments : `config` et `store` et `gateway` et `scheduler` et `telemetry`. | فريق التخزين | إعادة تصدير الأنواع القابلة لإعادة الاستخدام لدمجها مع Torii. |
| Utiliser `StorageConfig` comme `SoraFsStorage` (utilisateur → réel → valeurs par défaut). | فريق التخزين / Config WG | Utilisez les paramètres Norito/`iroha_config`. |
| Utilisez `NodeHandle` pour Torii pour les broches/récupérations. | فريق التخزين | تغليف تفاصيل التخزين والتوصيلات غير المتزامنة. |

### B. مخزن قطع دائم

| المهمة | المالك | الملاحظات |
|------|--------|---------------|
| Vous devez utiliser le manifeste `sorafs_car::ChunkStore` pour le manifeste (`sled`/`sqlite`). | فريق التخزين | Nom du produit : `<data_dir>/<manifest_cid>/chunk_{idx}.bin`. |
| La version PoR est disponible (avec 64 KiB/4 KiB) par `ChunkStore::sample_leaves`. | فريق التخزين | يدعم إعادة التشغيل؛ يفشل بسرعة عند التلف. |
| تنفيذ إعادة فحص السلامة عند البدء (إعادة تجزئة manifeste et pins غير المكتملة). | فريق التخزين | يمنع بدء Torii حتى اكتمال إعادة الفحص. |

### C. نقاط نهاية البوابة| نقطة النهاية | السلوك | المهام |
|-------------|---------|-------|
| `POST /sorafs/pin` | Le `PinProposalV1` est un manifeste et un manifeste du CID. | Il s'agit également d'un chunker et d'un morceau de papier. |
| `GET /sorafs/chunks/{cid}` + gamme de produits | تقديم بايتات القطع مع ترويسات `Content-Chunker` واحترام مواصفة نطاق القدرات. | استخدام المجدول مع ميزانيات البث (ربطها بقدرات النطاق SF-2d). |
| `POST /sorafs/por/sample` | تنفيذ أخذ عينات PoR لملف manifest وإرجاع حزمة إثبات. | Vous pouvez utiliser les fichiers Norito JSON. |
| `GET /sorafs/telemetry` | ملخصات: السعة ونجاح PoR وعدّادات أخطاء fetch. | توفير البيانات للوحة المراقبة/المشغلين. |

تقوم الوصلات في وقت التشغيل بتمرير تفاعلات PoR عبر `sorafs_node::por`, حيث يسجل المتتبع كل `PorChallengeV1` و`PorProofV1` و`AuditVerdictV1` pour le téléphone portable `CapacityMeter` pour le téléphone portable Torii Exemple.【crates/sorafs_node/src/scheduler.rs#L147】

ملاحظات تنفيذية:

- استخدم مكدس Axum الخاص بـ Torii avec `norito::json`.
- أضف مخططات Norito للاستجابات (`PinResultV1` و`FetchErrorV1` وبنى التليمترية).- ✅ أصبح المسار `/v1/sorafs/por/ingestion/{manifest_digest_hex}` يعرض عمق الـ backlog وأقدم epoch/deadline وأحدث طوابع النجاح/الفشل لكل مزود، عبر `sorafs_node::NodeHandle::por_ingestion_status`, et Torii, `torii_sorafs_por_ingest_backlog`/`torii_sorafs_por_ingest_failures_total` للّوحات.【crates/sorafs_node/src/lib.rs:510】【crates/iroha_torii/src/sorafs/api.rs:1883】【crates/iroha_torii/src/routing.rs:7244】【crates/iroha_telemetry/src/metrics.rs:5390】

### D. المجدول وفرض الحصص

| المهمة | التفاصيل |
|------|----------|
| حصة القرص | تتبع البايتات على القرص؛ رفض pins الجديدة عند تجاوز `max_capacity_bytes`. توفير نقاط ربط لسياسات الإخلاء المستقبلية. |
| توازي récupérer | شبهور عام (`max_parallel_fetches`) ميزانيات لكل مزود من حدود نطاق SF-2d. |
| épingles à cheveux | تحديد عدد مهام الإدخال المعلقة؛ توفير نقاط حالة Norito لعمق الطابور. |
| et PoR | عامل خلفي يعمل وفق `por_sample_interval_secs`. |

### E. التليمترية والسجلات

Nom (Prometheus):

-`sorafs_pin_success_total`, `sorafs_pin_failure_total`
- `sorafs_chunk_fetch_duration_seconds` (pour `result`)
-`torii_sorafs_storage_bytes_used`, `torii_sorafs_storage_bytes_capacity`
- `torii_sorafs_storage_pin_queue_depth`, `torii_sorafs_storage_fetch_inflight`
-`torii_sorafs_storage_fetch_bytes_per_sec`
-`torii_sorafs_storage_por_inflight`
-`torii_sorafs_storage_por_samples_success_total`, `torii_sorafs_storage_por_samples_failed_total`

السجلات / الأحداث:

- تليمترية Norito منظمة لعمليات الحوكمة (`StorageTelemetryV1`).
- تنبيهات عند تجاوز الاستغلال 90% أو عندما تتخطى سلسلة إخفاقات PoR العتبة.

### F. استراتيجية الاختبارات1. **اختبارات وحدات.** ديمومة مخزن القطع، حسابات الحصة، ثوابت المجدول (انظر `crates/sorafs_node/src/scheduler.rs`).
2. **اختبارات تكامل** (`crates/sorafs_node/tests`). دورة pin → fetch، الاستعادة بعد إعادة التشغيل، رفض الحصص، والتحقق من إثباتات أخذ عينات PoR.
3. **اختبارات تكامل Torii.** تشغيل Torii مع تفعيل التخزين وتجربة نقاط النهاية HTTP عبر `assert_cmd`.
4. **خارطة طريق الفوضى.** تدريبات مستقبلية تحاكي نفاد القرص، بطء IO، وإزالة الموفّرين.

## التبعيات

- سياسة قبول SF-2b — التأكد من أن العقد تتحقق من أظرف القبول قبل الإعلان.
- سوق السعة SF-2c — ربط التليمترية بإعلانات السعة.
- Annonce publicitaire pour SF-2d — استهلاك قدرة النطاق + ميزانيات البث عند توفرها.

## معايير إغلاق المرحلة

- `cargo run -p sorafs_node --example pin_fetch` يعمل مع luminaires محلية.
- Utilisez Torii et `--features sorafs-storage` pour obtenir des informations supplémentaires.
- تحديث الوثائق ([دليل تخزين العقدة](node-storage.md)) مع افتراضيات الإعداد وأمثلة CLI؛ Il s'agit du runbook للمشغلين.
- La mise en scène et la mise en scène des PoR.

## مخرجات الوثائق والعمليات

- تحديث [مرجع تخزين العقدة](node-storage.md) مع افتراضيات الإعداد، استخدام CLI، وخطوات الاستكشاف.
- إبقاء [runbook عمليات العقدة](node-operations.md) متوافقا مع التنفيذ مع تطور SF-3.
- نشر مراجع API لنقاط النهاية `/sorafs/*` داخل بوابة المطورين وربطها بملف OpenAPI عند وصول معالجات Torii.