---
lang: ar
direction: rtl
source: docs/portal/docs/da/replication-policy.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر القياسي
تعكس `docs/source/da/replication_policy.md`. ابق النسختين متزامنتين حتى يتم
سحب الوثائق القديمة.
:::

#تعديل بيانات البيانات (DA-4)

_الحالة: قيد التنفيذ -- المالكون: Core Protocol WG / Storage Team / SRE_

يطبق خط انابيب ingest الخاص بـ DA اهداف احتفاظ حتمية لكل فئة blob مذكورة في
`roadmap.md` (مسار DA-4). أرفض Toriiالإصلاحات التي يزودها بها
المتصل اذا لم يتطابق مع التحديد، ما يضمن ان كل عقدة مكتمل/تخزين نهائيا
الحقوق والنسخ المطلوبة دون الاعتماد على نية المرسل.

## البرمجة

| فئة النقطة | الحفاظ على الساخن | الاحتفاظ بالبرد | النسخ المطلوبة | فئة التخزين | وصل التورم |
|----------|------------|------------|----------------|-------------|-------------|
| `taikai_segment` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 أيام | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 يوما | 3 | `cold` | `da.governance` |
| _الافتراضي (كل الفئات الاخرى)_ | 6 ساعات | 30 يوما | 3 | `warm` | `da.default` |

دمجت هذه القيم في `torii.da_ingest.replication_policy` وتطبق على الجميع
طلبات `/v2/da/ingest`. أعيد كتابة Torii لبيانات الإصلاحات المرفوضة والصدر
تحذيرا عندما يحدد المتصل قيما غير متطابقة حتى يبدأوا من كشف SDKs
المتقادمة.

###وتوفر Taikai

إعلان بيان توجيه تايكاي (`taikai.trm`) عن `availability_class`
(`hot`, `warm`, او `cold`). يفترض Torii السياسة المطابقة قبل التقسيم يمكن
للمشغلين عدد النسخ لكل تيار دون تعديل الجدول العام. افتراضيات:

| فئة التوفر | الحفاظ على الساخن | الاحتفاظ بالبرد | النسخ المطلوبة | فئة التخزين | وصل التورم |
|------------|------------|------------|----------------|-------------|------------|
| `hot` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 يوما | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 يوما | 3 | `cold` | `da.taikai.archive` |

التلميحات الأصلية تعود الى `hot` حتى يتم البث المباشر لباقوى طفل. قم
تجاوز الانترنت عبر
`torii.da_ingest.replication_policy.taikai_availability` اذا كانت شبكتك تستخدم
اهداف مختلفة.

## الاعداد

تعيش السياسة تحت `torii.da_ingest.replication_policy` وتتعرض للقالب *default* مع
تتجاوز المصفوفة لكل فئة. معرفات فئة غير حساسة لحالة الاحرف وتقبل
`taikai_segment`, `nexus_lane_sidecar`, `governance_artifact`, او `custom:<u16>`
للامتدادات المعتمدة حوكما. لماذا تقبل التمويل `hot`, `warm`, او `cold`.

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

اترك كما يمكنك العمل بالقيم الافتراضية لاه. لتشديد الفئة، تعديل
المطابق؛ ولتغيير الاساس لفئات جديدة، متعددة `default_retention`.

يمكن تجاوز وجود Taikai بشكل متكامل عبر
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

- يستبدل Torii `RetentionPolicy` الذي يقدمه المستخدم بالملف المفروض من قبل التقسيم
  اصدرت بيانا.
- عدم رفض البيانات البنية المسبقة التي تعلن عن الاحتفاظ بها بشكل غير مطابق لـ
  `400 schema mismatch` حتى لا يبدأ العملاء المتقادمة من العروض الرائعة.
- يتم تسجيل كل حدث تجاوز (`blob_class`، السياسة المرسلة مقابل الخاصة)
  لا مظاهر المتصلين غير الملتزمين أثناء الطرح.

راجع خطى استيعاب لتوفر البيانات](ingest-plan.md) (قائمة التحقق) للبوابة المحدثة
التي تغطي اعمارها.

## سير عمل تكرار التكرار (متابعة DA-4)

الاعمار هو الثاني الاول فقط. يجب على الجميع القيام بذلك أيضًا اثبت ان البيانات
أعطوا واوامر التكرار استمر متسقة مع السياسة ويجب حتى SoraFS من تحسين
النقط غير المتوافقة بشكل واضح.

1. **راقب الانحراف.** يصدر Torii
   `overriding DA retention policy to match configured network baseline` عندما
   المرسل المتصل قيما قديمة للاحتفاظ بها. قرن هذا السجل بقياسات
   `torii_sorafs_replication_*` لتصحيح عيوب النسخ او إعادة نشر لاحقة.
2. **الفرق الفنية مقابل المنحة.** استخدام مساعد التدقيق الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   توغل الأمر `torii.da_ingest.replication_policy` من أعدادات الحملة،
   ويفك تشفير كل البيان (JSON او Norito)، ويتوافق مع اختياريا الحمولات
   `ReplicationOrderV1` عبر الملخص للـ البيان. يلخص اللغتين التاليةان:

   - `policy_mismatch` - ملف الإصلاح في البيان يختلف عن السياسة المتنوعة
     (لا يجب ان يحدث ذلك الا اذا كان Torii خلقا بشكل خاطئ).
   - `replica_shortfall` - أمر التكرار الحي يطلب النسخة أقل من
     `RetentionPolicy.required_replicas` او يقدم تعيينات اقل من الهدف.

   حالة خروج غير صفرية تعني نقصا مباشرا حتى بدء التشغيل اتـمتة CI/on-call من التنبيه
   مرة أخرى. ارفق تقرير JSON بزمة `docs/examples/da_manifest_review_template.md`
   لتصويت التصويت.
3. **اطلاق إعادة التكرار.** عندما لاحظت العيوب، صدر `ReplicationOrderV1`
   جديدا عبر ادوات التورية الموصوفة في
   [سوق سعة التخزين SoraFS](../sorafs/storage-capacity-marketplace.md)
   واعِد تشغيل التدقيق حتى تتقارب مجموعة النسخ. للتجاوزات، اربط مخرجات
   CLI مع `iroha app da prove-availability` حتى يتجه SREs من الرجوع لنفس الملخص
   ملحوظة: حزب الديمقراطيين الاشتراكيين.

لا تزال هناك تغطية لكرة القدم في `integration_tests/tests/da/replication_policy.rs`؛ تقوم
حزمة بارسال الرغبة في عدم الاحتفاظ متطابقة الى `/v2/da/ingest` والتحقق من ان
البيان المسترجع المقدم الملف المفروض بدلا من نية المتصل.