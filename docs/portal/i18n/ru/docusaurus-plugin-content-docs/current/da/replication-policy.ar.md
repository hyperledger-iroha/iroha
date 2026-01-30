---
lang: ru
direction: ltr
source: docs/portal/docs/da/replication-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

:::note المصدر القياسي
يعكس `docs/source/da/replication_policy.md`. ابق النسختين متزامنتين حتى يتم
سحب الوثائق القديمة.
:::

# سياسة تكرار توفر البيانات (DA-4)

_الحالة: قيد التنفيذ -- المالكون: Core Protocol WG / Storage Team / SRE_

يطبق خط انابيب ingest الخاص بـ DA اهداف احتفاظ حتمية لكل فئة blob مذكورة في
`roadmap.md` (مسار DA-4). يرفض Torii الاحتفاظ باغلفة الاحتفاظ التي يزودها
المتصل اذا لم تطابق السياسة المكونة، ما يضمن ان كل عقدة مدقق/تخزين تحتفظ بعدد
الحقب والنسخ المطلوبة دون الاعتماد على نية المرسل.

## السياسة الافتراضية

| فئة blob | احتفاظ hot | احتفاظ cold | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|----------|------------|-------------|----------------|-------------|-------------|
| `taikai_segment` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 ايام | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 يوما | 3 | `cold` | `da.governance` |
| _Default (كل الفئات الاخرى)_ | 6 ساعات | 30 يوما | 3 | `warm` | `da.default` |

تدمج هذه القيم في `torii.da_ingest.replication_policy` وتطبق على جميع
طلبات `/v1/da/ingest`. يعيد Torii كتابة manifests مع ملف الاحتفاظ المفروض ويصدر
تحذيرا عندما يوفر المتصلون قيما غير متطابقة حتى يتمكن المشغلون من كشف SDKs
المتقادمة.

### فئات توفر Taikai

تعلن manifests توجيه Taikai (`taikai.trm`) عن `availability_class`
(`hot`, `warm`, او `cold`). يفرض Torii السياسة المطابقة قبل التقسيم بحيث يمكن
للمشغلين توسيع عدد النسخ لكل stream دون تعديل الجدول العام. الافتراضيات:

| فئة التوفر | احتفاظ hot | احتفاظ cold | النسخ المطلوبة | فئة التخزين | وسم الحوكمة |
|------------|------------|-------------|----------------|-------------|-------------|
| `hot` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 يوما | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 يوما | 3 | `cold` | `da.taikai.archive` |

التلميحات المفقودة تعود الى `hot` حتى تحتفظ البثوث الحية باقوى سياسة. قم
بتجاوز الافتراضيات عبر
`torii.da_ingest.replication_policy.taikai_availability` اذا كانت شبكتك تستخدم
اهدافا مختلفة.

## الاعداد

تعيش السياسة تحت `torii.da_ingest.replication_policy` وتعرض قالب *default* مع
مصفوفة overrides لكل فئة. معرفات الفئة غير حساسة لحالة الاحرف وتقبل
`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, او `custom:<u16>`
للامتدادات المعتمدة حوكما. فئات التخزين تقبل `hot`, `warm`, او `cold`.

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

اترك الكتلة كما هي للعمل بالقيم الافتراضية اعلاه. لتشديد فئة، حدّث override
المطابق؛ ولتغيير الاساس لفئات جديدة، عدّل `default_retention`.

يمكن تجاوز فئات توفر Taikai بشكل مستقل عبر
`torii.da_ingest.replication_policy.taikai_availability`:

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

## دلالات الانفاذ

- يستبدل Torii `RetentionPolicy` الذي يقدمه المستخدم بالملف المفروض قبل التقسيم
  او اصدار manifest.
- ترفض manifests المبنية مسبقا التي تعلن ملف احتفاظ غير مطابق بـ
  `400 schema mismatch` حتى لا تتمكن العملاء المتقادمة من اضعاف العقد.
- يتم تسجيل كل حدث override (`blob_class`, السياسة المرسلة مقابل المتوقعة)
  لاظهار المتصلين غير الملتزمين اثناء rollout.

راجع [خطة ingest لتوفر البيانات](ingest-plan.md) (قائمة التحقق) للبوابة المحدثة
التي تغطي انفاذ الاحتفاظ.

## سير عمل اعادة التكرار (متابعة DA-4)

انفاذ الاحتفاظ هو الخطوة الاولى فقط. يجب على المشغلين ايضا اثبات ان manifests
الحية واوامر التكرار تبقى متسقة مع السياسة المكونة حتى يتمكن SoraFS من اعادة
تكرار blobs غير المتوافقة تلقائيا.

1. **راقب الانحراف.** يصدر Torii
   `overriding DA retention policy to match configured network baseline` عندما
   يرسل المتصل قيما قديمة للاحتفاظ. قرن هذا السجل مع قياسات
   `torii_sorafs_replication_*` لاكتشاف نقص النسخ او اعادة نشر متاخرة.
2. **فرق النية مقابل النسخ الحية.** استخدم مساعد التدقيق الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   يحمل الامر `torii.da_ingest.replication_policy` من الاعدادات المقدمة،
   ويفك تشفير كل manifest (JSON او Norito)، ويطابق اختياريا payloads
   `ReplicationOrderV1` عبر digest للـ manifest. يلخص الشرطان التاليان:

   - `policy_mismatch` - ملف الاحتفاظ في manifest يختلف عن السياسة المفروضة
     (لا يجب ان يحدث ذلك الا اذا كان Torii مكونا بشكل خاطئ).
   - `replica_shortfall` - امر التكرار الحي يطلب نسخا اقل من
     `RetentionPolicy.required_replicas` او يقدم تعيينات اقل من الهدف.

   حالة خروج غير صفرية تعني نقصا نشطا حتى تتمكن اتـمتة CI/on-call من التنبيه
   فورا. ارفق تقرير JSON بحزمة `docs/examples/da_manifest_review_template.md`
   لتصويت البرلمان.
3. **اطلق اعادة التكرار.** عندما يبلغ التدقيق عن نقص، اصدر `ReplicationOrderV1`
   جديدا عبر ادوات الحوكمة الموصوفة في
   [SoraFS storage capacity marketplace](../sorafs/storage-capacity-marketplace.md)
   واعِد تشغيل التدقيق حتى تتقارب مجموعة النسخ. للتجاوزات الطارئة، اربط مخرجات
   CLI مع `iroha app da prove-availability` حتى يتمكن SREs من الرجوع لنفس digest
   ودليل PDP.

توجد تغطية الانحدار في `integration_tests/tests/da/replication_policy.rs`؛ تقوم
الحزمة بارسال سياسة احتفاظ غير متطابقة الى `/v1/da/ingest` وتتحقق من ان
manifest المسترجع يعرض الملف المفروض بدلا من نية المتصل.
