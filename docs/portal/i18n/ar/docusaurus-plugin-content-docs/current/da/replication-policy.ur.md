---
lang: ar
direction: rtl
source: docs/portal/docs/da/replication-policy.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة مستند ماخذ
قم بالتسجيل دون الحاجة إلى مزامنة السجلات.
:::

# سياسة النسخ المتماثل لتوفر البيانات (DA-4)

_حالت: قيد التقدم -- المالكون: Core Protocol WG / Storage Team / SRE_

خط أنابيب استيعاب DA `roadmap.md` (workstream DA-4) يحتوي على فئة blob
من أجل تحقيق أهداف الاحتفاظ الحتمية، استخدم هذه الأهداف. Torii احتفاظ
تم تكوين المغلفات التي تستمر في الضغط على زر المتصل وتكوين المتصل
لا توجد سياسة مطابقة، هناك حاجة إلى مدقق/عقدة تخزين مطلوبة في العصور و
النسخ المتماثلة عدد كبير من الاحتفاظ برغبة المرسل في منع الحصار.

## السياسة الافتراضية

| فئة النقطة | الاحتفاظ الساخن | احتباس البرد | النسخ المتماثلة المطلوبة | فئة التخزين | وسم الحوكمة |
|------------|--------------|----------------|----|----------------|----------------|
| `taikai_segment` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 أيام | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 يوما | 3 | `cold` | `da.governance` |
| _الافتراضي (جميع الفئات الأخرى)_ | 6 ساعات | 30 يوما | 3 | `warm` | `da.default` |

هناك قيمة `torii.da_ingest.replication_policy` مضمنة وتمام
`/v2/da/ingest` تم نشر عمليات الإرسال في وقت لاحق. Torii ملف تعريف الاحتفاظ المفروض
يظهر سات تكرر المكالمة وعندما لا يتطابق المتصلون مع قيم البطاقة
يجب أن تحذر من أن المشغلين يستخدمون أدوات تطوير البرامج (SDK) التي لا معنى لها.

### فئات توفر Taikai

بيانات توجيه Taikai (`taikai.trm`) `availability_class` (`hot`, `warm`,
أو `cold`) أعلن عن قرض. Torii طريقة المطابقة لسياسة المطابقة
عدد مرات تحرير الجدول العالمي للمشغلين عدد النسخ المتماثلة
مقياس سكي. الافتراضيات:

| فئة التوفر | الاحتفاظ الساخن | احتباس البرد | النسخ المتماثلة المطلوبة | فئة التخزين | وسم الحوكمة |
|--------------------|--------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 يوما | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 يوما | 3 | `cold` | `da.taikai.archive` |

التلميحات المفقودة افتراضيًا على `hot` قم بتشغيل البث المباشر على الإنترنت
الاحتفاظ بالسياسة کریں۔ إذا استخدمت أهدافًا مختلفة للشبكة، فاستخدمها
`torii.da_ingest.replication_policy.taikai_availability` الإعدادات الافتراضية
تجاوز کریں۔

## التكوين

هذه السياسة `torii.da_ingest.replication_policy` موجودة تحت رهن عقاري و*افتراضي*
القالب الثابت لكل فئة يتجاوز حدود المصفوفة. معرفات الطبقة
حساس لحالة الأحرف و`taikai_segment`، `nexus_lane_sidecar`،
`governance_artifact`، أو `custom:<u16>` (الامتدادات المعتمدة من قبل الإدارة)
كرتے ہيں. فئات التخزين `hot`، `warm`، أو `cold`

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

تعمل الإعدادات الافتراضية على منع حظر فيزا. كسی الطبقة کو تشديد
تجاوز التحديث المتعلق بالموضوع؛ يتم تحديث الفصول الجديدة الأساسية
`default_retention` تحرير.

تتجاوز فئات توفر Taikai كل شيء عبر
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

## دلالات التنفيذ

- Torii `RetentionPolicy` الذي يوفره المستخدم لملف التعريف المفروض سيستبدل كرتا ہے
  تقطيع أو انبعاث واضح.
- البيانات المعدة مسبقًا لإعلان ملف تعريف الاحتفاظ غير المتطابق، `400 schema mismatch`
  إن رفض القرض مرة أخرى يؤدي إلى إضعاف عقود العملاء القديمة التي لا تضعف.
- تجاوز سجل الأحداث ہوتا ہے (`blob_class`، تم الإرسال مقابل السياسة المتوقعة)
  تم إطلاق هذا التطبيق من قبل المتصلين غير المتوافقين.

البوابة المحدثة کے لئے [خطة استيعاب توفر البيانات](ingest-plan.md)
(قائمة التحقق من الصحة) هناك أيضًا إنفاذ للاحتفاظ وغطاء للقرار.

## سير عمل إعادة النسخ (متابعة DA-4)

لقد تم تطبيق الاحتفاظ بالبيانات. المشغلون هم على قيد الحياة بشكل ثابت
سياسة تكوين أوامر النسخ والبيانات بما يتوافق مع SoraFS
النقط غير المتوافقة التي تعمل على إعادة تكرارها.

1. **نظرة الانجراف.** Torii
   `overriding DA retention policy to match configured network baseline` تنبعث
   هذه هي قيم الاحتفاظ بالمتصل التي لا معنى لها. سجل کو
   `torii_sorafs_replication_*` القياس عن بعد الذي أدى إلى نقص النسخ المتماثلة أو
   عمليات إعادة الانتشار المتأخرة.
2. **النية مقابل النسخ المتماثلة الحية تختلف.** استخدام مساعد التدقيق:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   هذا الأمر `torii.da_ingest.replication_policy` يقوم بتنزيل ملف التكوين
   فك تشفير البيان (JSON أو Norito) كرتا ہے، ومختوم على `ReplicationOrderV1`
   الحمولات الصافية تتطابق مع ملخص البيان. خلاصةہ دو شرط العلم کرتا ہے:

   - `policy_mismatch` - سياسة فرض ملف تعريف الاحتفاظ الواضح
     (لم يتم إنشاء Torii بشكل خاطئ).
   - `replica_shortfall` - أمر النسخ المتماثل المباشر `RetentionPolicy.required_replicas`
     سے طلب النسخ المتماثلة أو الهدف سے كم المهام ديتا ہے.

   حالة الخروج غير الصفرية النقص النشط في حالة عدم الاتصال وCI/عند الطلب
   أتمتة الصفحة فوراً کر سکے۔ تقرير JSON
   حزمة `docs/examples/da_manifest_review_template.md` التي يتم إرفاقها بالبطاقة
   يصوت البرلمان على هذا التصويت.
3. ** مشغل إعادة النسخ المتماثل. ** بعد تقرير نقص التدقيق، هذا جديد
   `ReplicationOrderV1` جاري عبر أدوات الحوكمة جو
   [سوق سعة التخزين SoraFS](../sorafs/storage-capacity-marketplace.md)
   لا يزال هناك الكثير من التدقيق والمراجعة عندما لا تتقارب مجموعة النسخ المتماثلة.
   تجاوزات الطوارئ لإخراج CLI `iroha app da prove-availability`
   تشير كل هذه البيانات إلى SREs والملخص وأدلة PDP.

تغطية الانحدار `integration_tests/tests/da/replication_policy.rs` موجودة؛
مجموعة `/v2/da/ingest` تعمل على سياسة استبقاء غير متطابقة وتتحقق من البطاقة
تم جلب نية المتصل الواضحة وكشف الملف الشخصي المفروض.