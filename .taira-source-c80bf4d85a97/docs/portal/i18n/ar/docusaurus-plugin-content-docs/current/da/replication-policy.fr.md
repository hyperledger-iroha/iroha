---
lang: ar
direction: rtl
source: docs/portal/docs/da/replication-policy.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة المصدر الكنسي
ريفليت `docs/source/da/replication_policy.md`. جارديز ليه نسختين أون
:::

# سياسة النسخ المتماثل لتوفر البيانات (DA-4)

_الحالة: في الدورة - المسؤولون: مجموعة عمل البروتوكول الأساسية / فريق التخزين / SRE_

يتم تطبيق خط أنابيب الإدخال DA للحفاظ على كائنات الاحتفاظ
محددات لكل فئة من النقطة التي تحددها في `roadmap.md` (مسار العمل
دا-4). Torii رفض استمرار حفظ المظاريف على قدم المساواة
المتصل الذي لا يتواصل مع السياسة التي تم تكوينها، يضمن ذلك
كل يوم مدقق/مخزون محتفظ بالاسم المطلوب للعصر وآخر
النسخ المتماثلة دون الاعتماد على نية الباعث.

## سياسة افتراضية

| كلاس دي بلوب | الاحتفاظ الساخنة | الاحتفاظ بالبرد | يتطلب النسخ المتماثلة | فئة المخزون | علامة الحكم |
|---------------|--------------|----------------|---|-----------------------------------|----|
| `taikai_segment` | 24 ساعة | 14 يوم | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 أيام | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 يوم | 3 | `cold` | `da.governance` |
| _الافتراضي (toutes les autresclass)_ | 6 ساعات | 30 يوم | 3 | `warm` | `da.default` |

هذه القيم هي عناصر متكاملة في `torii.da_ingest.replication_policy` وتطبيقاتها
جميع عمليات الإرسال `/v1/da/ingest`. Torii قم بإعادة كتابة البيانات مع
ملف تعريف الاحتفاظ يفرض ويصدر إخطارًا عند وصول المتصلين
القيم غير المتماسكة تسمح للمشغلين باكتشاف SDKs القديمة.

### فئات المتاح Taikai

تعلن بيانات توجيه Taikai (`taikai.trm`) عن `availability_class`
(`hot`، `warm`، أو `cold`). Torii ينطبق على السياسة المقابلة مقدمًا
التقطيع يتيح للمشغلين ضبط حسابات النسخ المتماثلة على قدم المساواة
تيار بلا محرر الجدول العالمي. الافتراضيات:

| فئة المتاح | الاحتفاظ الساخنة | الاحتفاظ بالبرد | يتطلب النسخ المتماثلة | فئة المخزون | علامة الحكم |
|-------------------------|--------------|----------------|---|-----------------------------------|----|
| `hot` | 24 ساعة | 14 يوم | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 يوم | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 يوم | 3 | `cold` | `da.taikai.archive` |

تعود التلميحات الصعبة إلى `hot` حتى تتمكن عمليات البث المباشر من الاحتفاظ بها
السياسة لا زائد القوة. قم باستبدال الإعدادات الافتراضية عبر
`torii.da_ingest.replication_policy.taikai_availability` إذا استخدمت شبكتك
دي cibles مختلفة.

## التكوين

La politique vit sous `torii.da_ingest.replication_policy` وكشف قالب
*افتراضي* بالإضافة إلى لوحة تجاوز لكل فئة. معرفات الفئة هي
غير حساس لحالة الأحرف ومقبول `taikai_segment`، `nexus_lane_sidecar`،
`governance_artifact`، أو `custom:<u16>` للملحقات المعتمدة على قدم المساواة
الحكم. تقبل فئات المخزون `hot`، `warm`، أو `cold`.

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

اترك الكتلة سليمة لاستخدام الإعدادات الافتراضية. صب ducir une
كلاس، ميتيز أ جور لوفرايد مراسل؛ صب المغير لا قاعدة صب دي
فئات جديدة، تحرير `default_retention`.

قد يتم فرض رسوم إضافية على فئات Taikai المتوفرة بشكل مستقل عبر
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

## دلالات الإنفاذ

- Torii يستبدل `RetentionPolicy` بواسطة المستخدم من خلال ملف التعريف
  قم بفرض التقطيع أو الانبعاث الواضح.
- البيانات التي تم إنشاؤها مسبقًا والتي تعلن عن ملف تعريف مختلف للاحتفاظ به
  تم الرفض مع `400 schema mismatch` حتى لا يكون العملاء القديمون
  لا يمكن تفعيل العقد.
- كل حدث تجاوز تم تسجيله (`blob_class`، ملخص السياسة مقابل الحضور)
  لإثبات عدم توافق المتصلين أثناء عملية الطرح.

Voir [خطة استيعاب توفر البيانات](ingest-plan.md) (قائمة التحقق من الصحة) من أجل
البوابة لا تغطي سوى يوم واحد من تنفيذ الاحتفاظ.

## سير عمل إعادة النسخ (suivi DA-4)

إن تطبيق الاحتفاظ ليس هو العرض الأول. المشغلون يفعلون ذلك
تأكد أيضًا من أن البيانات حية وأن أوامر النسخ المتماثل لا تزال قائمة
محاذاة مع السياسة التي تم تكوينها حتى تتمكن SoraFS من إعادة إنتاج النقط
لا تتوافق مع التشغيل الآلي.

1. **مراقبة الانجراف.** Torii
   `overriding DA retention policy to match configured network baseline` ربع
   أحد المتصلين يجمع قيم الاحتفاظ القديمة. Associez ce log avec la
   القياس عن بعد `torii_sorafs_replication_*` لتصحيح نقص النسخ المتماثلة
   أو عمليات إعادة الانتشار المتخلفة.
2. **النية المختلفة مقابل النسخ المتماثلة المباشرة.** استخدم مساعد التدقيق الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   أمر الشحن `torii.da_ingest.replication_policy` بعد التكوين
   فورني، فك تشفير كل بيان (JSON ou Norito)، وخيارات الارتباط
   الحمولات الصافية `ReplicationOrderV1` حسب ملخص البيان. إشارة الاستئناف
   الشروط الثنائية:

   - `policy_mismatch` - ملف تعريف الاحتفاظ بملف التعريف المتباين
     فرض (ceci ne devrait jamais Arriver sauf si Torii est mal تكوين).
   - `replica_shortfall` - أمر النسخ المتماثل المباشر الذي يتطلب أقل عدد من النسخ المتماثلة
     que `RetentionPolicy.required_replicas` أو توفر أقل من التخصيصات
     سا.

   تشير حالة الطلعة غير الصفرية إلى عدم كفاية النشاط حتى تتمكن من ذلك
   أتمتة CI/عند الطلب ثم استدعاء النداء الفوري. Joignez le Rapport JSON
   au paquet `docs/examples/da_manifest_review_template.md` لأصواتك
   بارلمنت.
3. **قم بإلغاء قفل إعادة النسخ.** عندما تشير المراجعة إلى عدم كفاية،
   قم بإصدار `ReplicationOrderV1` جديد عبر أدوات الحكم
   في [SoraFS سوق سعة التخزين](../sorafs/storage-capacity-marketplace.md)
   ثم قم بإعادة التدقيق حتى تتقارب مجموعة النسخ المتماثلة. صب ليه التجاوزات
   عاجل، قم بإقران أمر CLI بـ `iroha app da prove-availability` من أجل ذلك
   يمكن أن تشير SREs إلى ملخص الملخص وPDP المسبق.

غطاء الانحدار في `integration_tests/tests/da/replication_policy.rs`;
هناك سياسة الاحتفاظ غير المتوافقة مع `/v1/da/ingest` والتحقق منها
يفرض البيان الذي يستعيده كشف الملف الشخصي على نية المتصل.