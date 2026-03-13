---
lang: ar
direction: rtl
source: docs/portal/docs/da/replication-policy.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فوينتي كانونيكا
ريفليجا `docs/source/da/replication_policy.md`. Mantenga ambas الإصدارات en
:::

# سياسة نسخ البيانات المتوفرة (DA-4)

_الحالة: التقدم - المسؤولون: فريق عمل البروتوكول الأساسي / فريق التخزين / SRE_

خط أنابيب الإدخال DA الآن ينطبق على أهداف الاحتفاظ المحددة
تم وصف كل فئة من وحدات النقطة في `roadmap.md` (مسار العمل DA-4). Torii rechaza
احتفظ بمغلفات الاحتفاظ المخصصة للمتصل بحيث لا تتزامن مع ذلك
السياسة التكوينية، تضمن كل عقدة من التحقق/التخزين المحتفظ به
العدد المطلوب من العصور والنسخ المتماثلة لا يعتمد على نية الباعث.

## السياسة من أجل الخلل

| كلاس دي بلوب | الاحتفاظ الساخنة | الاحتفاظ بالبرد | النسخ المتماثلة المطلوبة | كلاس دي تخزين | الوسم دي حاكمة |
|--------------|--------------|----------------|-----|----------------------------------------|---|
| `taikai_segment` | 24 ساعة | 14 دياس | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 دياس | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 دياس | 3 | `cold` | `da.governance` |
| _الافتراضي (todas las demas classes)_ | 6 ساعات | 30 دياس | 3 | `warm` | `da.default` |

يتم تعزيز هذه القيم في `torii.da_ingest.replication_policy` ويتم تطبيقها
جميع الطلبات `/v2/da/ingest`. Torii إعادة كتابة البيانات باستخدام ملف التعريف
قوة الاحتفاظ وبث إعلان عندما يكتسب المتصلون قيمًا
لا توجد مصادفات حتى يتمكن المشغلون من اكتشاف حزم SDK غير النشطة.

### فئات التوفر Taikai

تعلن بيانات Enrutamiento Taikai (`taikai.trm`) عن عدم
`availability_class` (`hot`، `warm`، أو `cold`). Torii يتم تطبيق السياسة
قبل القطع، يمكن للمشغلين أن يصعدوا
محتوى النسخ المتماثلة للبث بدون تحرير اللوحة العالمية. الافتراضيات:

| فئة التوفر | الاحتفاظ الساخنة | الاحتفاظ بالبرد | النسخ المتماثلة المطلوبة | كلاس دي تخزين | الوسم دي حاكمة |
|-------------------------|--------------|----------------|-----|----------------------------------------|---|
| `hot` | 24 ساعة | 14 دياس | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 دياس | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 دياس | 3 | `cold` | `da.taikai.archive` |

تستخدم المسارات الخاطئة `hot` بسبب الخلل في عمليات الإرسال الحية
استعادة السياسة أكثر قوة. Sobrescriba los defaults عبر
`torii.da_ingest.replication_policy.taikai_availability` إذا كانت أهدافك حمراء
مختلفة.

## التكوين

السياسة الحية bajo `torii.da_ingest.replication_policy` وعرض قالب واحد
*افتراضي* هو عبارة عن مجموعة تجاوزات لكل فئة. معرفات الفئة رقم
الابن المعقول a mayus/minus y aceptan `taikai_segment`، `nexus_lane_sidecar`،
`governance_artifact`، أو `custom:<u16>` للتمديدات المناسبة للإدارة.
تقبل فئات التخزين `hot`، أو `warm`، أو `cold`.

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

Deje الكتلة سليمة لاستخدام القوائم الافتراضية القادمة. بارا متحمل
فئة واحدة هي التجاوز الفعلي؛ لتغيير القاعدة الجديدة
الفئات، تحرير `default_retention`.

يمكن تسجيل فئات التوفر في Taikai بشكل مستقل
عبر `torii.da_ingest.replication_policy.taikai_availability`:

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

## تطبيق دلالات الدلالة

- Torii يتم استبدال `RetentionPolicy` بالمستخدم بالملف
  الدفع قبل التقطيع أو إصدار البيانات.
- البيانات التي تم إنشاؤها مسبقًا والتي تعلن عن ملف تخزين منفصل بحد ذاته
  قم بالحجز مع `400 schema mismatch` حتى لا يتمكن العملاء القدامى من
  العقد المنهك.
- كل حدث تجاوز التسجيل (`blob_class`، إرسال سياسي مقابل انتظار)
  لعدم تطابق المتصلين أثناء الطرح.

الإصدار [خطة استيعاب توفر البيانات](ingest-plan.md) (قائمة التحقق من الصحة)
لتحديث البوابة لمنع إنفاذ الاحتفاظ.

## تدفق إعادة النسخ (تابع DA-4)

إن فرض الاحتفاظ هو الخطوة الأولى فقط. المشغلون موجودون أيضًا
تأكد من الحفاظ على البيانات الحية وأوامر النسخ
تم تخصيصها مع السياسة التي تم تكوينها حتى تتمكن SoraFS من إعادة نسخ النقط
فوراً دي شكل من أشكال الولاء التلقائي.

1. **اليقظة الانجراف.** Torii تنبعث
   `overriding DA retention policy to match configured network baseline`
   يبعث المتصل بقيم الاحتفاظ غير الفعلية. قم بشراء هذا السجل
   القياس عن بعد `torii_sorafs_replication_*` للكشف عن النسخ المتماثلة الخاطئة
   o يعيد نشر الديمورادوس.
2. **الاختلاف في النية مقابل النسخ المتماثلة في الجسم الحي.** استخدم مساعد الاستماع الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   أمر الشحن `torii.da_ingest.replication_policy` من التكوين
   Provista، وفك تشفير كل بيان (JSON o Norito)، والتشغيل اختياريًا
   الحمولات `ReplicationOrderV1` لملخص البيان. استئناف ماركا دوس
   الظروف:

   - `policy_mismatch` - ملف الاحتفاظ بالبيان المتباين
     قوة سياسية (هذا لا ينبغي أن يحدث إطلاقًا Torii هذا سيء)
     تكوين).
   - `replica_shortfall` - أمر النسخ المتماثل في الحياة يطلب أقل عدد ممكن من النسخ المتماثلة
     ما `RetentionPolicy.required_replicas` أو يجب عليك تخصيص أقل ما لديك
     objetivo.

   تشير حالة الخروج التي لا تشير إلى وجود نشاط خاطئ للأتمتة
   CI/on-call يمكن أن تكون هناك صفحة فورية. أضف تقرير JSON إلى الحزمة
   `docs/examples/da_manifest_review_template.md` لتصويتات البرلمان.
3. **إلغاء إعادة النسخ.** عند قيام الجمهور بالإبلاغ عن خطأ، يصدر واحدًا
   جديد `ReplicationOrderV1` عبر أدوات الإدارة الموصوفة
   [سوق سعة التخزين SoraFS](../sorafs/storage-capacity-marketplace.md)
   ثم قم بتشغيل الاستماع حتى يتم تجميع مجموعة النسخ المتماثلة. الفقرة
   تجاوزات الطوارئ، وتمكين خروج CLI مع `iroha app da prove-availability`
   حتى تتمكن SREs من الرجوع إلى نفس الملخص وأدلة PDP.

La cobertura de regression vive en
`integration_tests/tests/da/replication_policy.rs`; لا سويت Envia una Politica de
الاحتفاظ ليس بالصدفة مع `/v2/da/ingest` والتحقق من الحصول على البيان
قم بإظهار الملف الشخصي في مكان نية المتصل.