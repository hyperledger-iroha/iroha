---
lang: fr
direction: ltr
source: docs/portal/docs/da/replication-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note المصدر القياسي
C'est `docs/source/da/replication_policy.md`. ابق النسختين متزامنتين حتى يتم
سحب الوثائق القديمة.
:::

# سياسة تكرار توفر البيانات (DA-4)

_الحالة : قيد التنفيذ -- المالكون : Core Protocol WG / Storage Team / SRE_

يطبق خط انابيب ingest الخاص بـ DA اهداف احتفاظ حتمية لكل فئة blob مذكورة في
`roadmap.md` (pour DA-4). يرفض Torii الاحتفاظ باغلفة الاحتفاظ التي يزودها
المتصل اذا لم تطابق السياسة المكونة، ما يضمن ان كل عقدة مدقق/تخزين تحتفظ بعدد
الحقب والنسخ المطلوبة دون الاعتماد على نية المرسل.

## السياسة الافتراضية

| فئة blob | احتفاظ chaud | احتفاظ froid | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|--------------|------------|-------------|----------------|-------------|-------------|
| `taikai_segment` | 24 heures | 14 janvier | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 étapes | 7 janvier | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 heures | 180 janvier | 3 | `cold` | `da.governance` |
| _Par défaut (كل الفئات الاخرى)_ | 6 étapes | 30 janvier | 3 | `warm` | `da.default` |

تدمج هذه القيم في `torii.da_ingest.replication_policy` وتطبق على جميع
طلبات `/v1/da/ingest`. يعيد Torii كتابة manifestes مع ملف الاحتفاظ المفروض ويصدر
Vous pouvez utiliser des SDK pour créer des liens vers des SDK
المتقادمة.

### فئات توفر Taikaiتعلن manifeste توجيه Taikai (`taikai.trm`) ou `availability_class`
(`hot`, `warm`, et `cold`). يفرض Torii السياسة المطابقة قبل التقسيم بحيث يمكن
للمشغلين توسيع عدد النسخ لكل stream دون تعديل الجدول العام. الافتراضيات:

| فئة التوفر | احتفاظ chaud | احتفاظ froid | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|------------|------------|-------------|----------------|-------------|-------------|
| `hot` | 24 heures | 14 janvier | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 étapes | 30 janvier | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 pièce | 180 janvier | 3 | `cold` | `da.taikai.archive` |

Il s'agit de la référence `hot`. قم
بتجاوز الافتراضيات عبر
`torii.da_ingest.replication_policy.taikai_availability` اذا كانت شبكتك تستخدم
اهدافا مختلفة.

## الاعداد

تعيش السياسة تحت `torii.da_ingest.replication_policy` وتعرض قالب *default* مع
مصفوفة remplace لكل فئة. معرفات الفئة غير حساسة لحالة الاحرف وتقبل
`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, et `custom:<u16>`
للامتدادات المعتمدة حوكما. Il s'agit de `hot`, `warm`, et `cold`.

```toml
[torii.da_ingest.replication_policy.default_retention]
hot_retention_secs = 21600          # 6 h
cold_retention_secs = 2592000       # 30 d
required_replicas = 3
storage_class = "warm"
governance_tag = "da.default"

[[torii.da_ingest.replication_policy.overrides]]
class = "taikai_segment"
[torii.da_ingest.replication_policy.overrides.retention]
hot_retention_secs = 86400          # 24 h
cold_retention_secs = 1209600       # 14 d
required_replicas = 5
storage_class = "hot"
governance_tag = "da.taikai.live"
```

اترك الكتلة كما هي للعمل بالقيم الافتراضية اعلاه. لتشديد فئة، حدّث remplacement
المطابق؛ Il s'agit d'un `default_retention`.

يمكن تجاوز فئات توفر Taikai بشكل مستقل عبر
`torii.da_ingest.replication_policy.taikai_availability` :

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "cold"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 3600          # 1 h
cold_retention_secs = 15552000     # 180 d
required_replicas = 3
storage_class = "cold"
governance_tag = "da.taikai.archive"
```

## دلالات الانفاذ- يستبدل Torii `RetentionPolicy` الذي يقدمه المستخدم بالملف المفروض قبل التقسيم
  او اصدار manifeste.
- ترفض manifeste المبنية مسبقا التي تعلن ملف احتفاظ غير مطابق بـ
  `400 schema mismatch` حتى لا تتمكن العملاء المتقادمة من اضعاف العقد.
- يتم تسجيل كل حدث override (`blob_class`, السياسة المرسلة مقابل المتوقعة)
  لاظهار المتصلين غير الملتزمين اثناء déploiement.

راجع [خطة ingest لتوفر البيانات](ingest-plan.md) (قائمة التحقق) للبوابة المحدثة
التي تغطي انفاذ الاحتفاظ.

## سير عمل اعادة التكرار (متابعة DA-4)

انفاذ الاحتفاظ هو الخطوة الاولى فقط. يجب على المشغلين ايضا اثبات ان manifeste
La situation actuelle est proche de celle de SoraFS pour le client
تكرار blobs غير المتوافقة تلقائيا.

1. **راقب الانحراف.** يصدر Torii
   `overriding DA retention policy to match configured network baseline` Français
   يرسل المتصل قيما قديمة للاحتفاظ. قرن هذا السجل مع قياسات
   `torii_sorafs_replication_*` لاكتشاف نقص النسخ او اعادة نشر متاخرة.
2. **فرق النية مقابل النسخ الحية.** استخدم مساعد التدقيق الجديد :

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   يحمل الامر `torii.da_ingest.replication_policy` pour les appareils ménagers
   Il s'agit d'un manifeste (JSON et Norito) et de charges utiles
   `ReplicationOrderV1` عبر digest للـ manifeste. يلخص الشرطان التاليان:- `policy_mismatch` - ملف الاحتفاظ في manifest يختلف عن السياسة المفروضة
     (Là يجب ان يحدث ذلك الا اذا كان Torii مكونا بشكل خاطئ).
   - `replica_shortfall` - امر التكرار الحي يطلب نسخا اقل من
     `RetentionPolicy.required_replicas` او يقدم تعيينات اقل من الهدف.

   حالة خروج غير صفرية تعني نقصا نشطا حتى تتمكن اتـمتة CI/on-call من التنبيه
   فورا. Utiliser JSON pour `docs/examples/da_manifest_review_template.md`
   لتصويت البرلمان.
3. **اطلق اعادة التكرار.** عندما يبلغ التدقيق عن نقص، اصدر `ReplicationOrderV1`
   جديدا عبر ادوات الحوكمة الموصوفة في
   [Marché de capacité de stockage SoraFS](../sorafs/storage-capacity-marketplace.md)
   واعِد تشغيل التدقيق حتى تتقارب مجموعة النسخ. للتجاوزات الطارئة، اربط مخرجات
   CLI avec `iroha app da prove-availability` pour les SRE dans le résumé
   Et PDP.

توجد تغطية الانحدار في `integration_tests/tests/da/replication_policy.rs`؛ تقوم
حزمة بارسال سياسة احتفاظ غير متطابقة الى `/v1/da/ingest` وتتحقق من ان
manifeste المسترجع يعرض الملف المفروض بدلا من نية المتصل.