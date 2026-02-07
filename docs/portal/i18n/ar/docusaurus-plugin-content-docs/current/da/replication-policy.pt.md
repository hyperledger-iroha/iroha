---
lang: ar
direction: rtl
source: docs/portal/docs/da/replication-policy.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::ملاحظة فونتي كانونيكا
إسبيلها `docs/source/da/replication_policy.md`. Mantenha كما أدعية الآيات م
:::

# سياسة النسخ المتماثل لتوافر البيانات (DA-4)

_الحالة: تقدم مستمر -- الاستجابات: فريق عمل البروتوكول الأساسي / فريق التخزين / SRE_

يتم تطبيق خط أنابيب استيعاب DA Agora Metas Deterministicas de Retencao لكل شخص
تم وصف فئة النقطة في `roadmap.md` (مسار العمل DA-4). Torii طلب الاستمرار
مظاريف الاحتفاظ بالمعلومات الخاصة بالشخص الذي لا يتواصل مع السياسة
التكوين، مما يضمن أن كل عقدة مدقق/تخزين تحتفظ برقم
يتطلب العصر والنسخ المتماثلة، ولا يعتمد ذلك على تصميم المُصدر.

## سياسة بادراو

| كلاس دي بلوب | ريتينكاو ساخن | ريتينكاو بارد | النسخ المتماثلة المطلوبة | فئة التخزين | العلامة دي الحكم |
|---------------|---------------|-------------------------------------|----------|---|----------------|
| `taikai_segment` | 24 ساعة | 14 دياس | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 دياس | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 دياس | 3 | `cold` | `da.governance` |
| _افتراضي (جميعها كفئات demais)_ | 6 ساعات | 30 دياس | 3 | `warm` | `da.default` |

هذه القيم متضمنة في `torii.da_ingest.replication_policy` ومطبقة
جميعها كإرسالات `/v1/da/ingest`. يتم إعادة إنشاء البيانات Torii مع الملف الشخصي
من خلال الاحتفاظ بالرسوم وإصدار تحذير عندما يطلب المتصلون قيمًا متباينة
حتى يتمكن المشغلون من اكتشاف حزم SDK غير المكتملة.

### فئات التوفر Taikai

بيانات التدوير Taikai (`taikai.trm`) تعلن `availability_class`
(`hot`، `warm`، أو `cold`). Torii يتم تطبيق ذلك من قبل المراسلين السياسيين
القطع حتى يتمكن المشغلون من إرسال عدوى النسخ المتماثلة من خلال البث المباشر
تحرير لوحة عالمية. الافتراضيات:

| فئة التوفر | ريتينكاو ساخن | ريتينكاو بارد | النسخ المتماثلة المطلوبة | فئة التخزين | العلامة دي الحكم |
|---------------------------|--------------|--------------|------------------------------------|----------|---|----------------|
| `hot` | 24 ساعة | 14 دياس | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 دياس | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 دياس | 3 | `cold` | `da.taikai.archive` |

تلميحات ausentes usam `hot` por padrao para que transmissoes ao vivo retenham a
السياسة أقوى. استبدال نظام التشغيل الافتراضي عبر
`torii.da_ingest.replication_policy.taikai_availability` إذا كنت تستخدمه
مختلفون تمامًا.

## التكوين

الحياة السياسية تنهد `torii.da_ingest.replication_policy` وتعرض قالبًا
*افتراضي* عبارة عن مصفوفة تتجاوز الفئة. معرفات الفئة ناو
الاختلاف الكبير/الصغير والصحيح `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`، أو `custom:<u16>` للتحسينات الشاملة للحكم.
فئات التخزين هذه `hot`، `warm`، أو `cold`.

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

Deixe o block سليم للتنقل مع الإعدادات الافتراضية. لتحمله
فئة، تحقيق أو تجاوز المراسلات؛ الفقرة مودار قاعدة دي نوفا الطبقات،
تحرير `default_retention`.

يمكن تصنيف فئات Taikai المتاحة بشكل مستقل
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

- Torii بديل أو `RetentionPolicy` fornecido pelo usuario pelo perfil imposto
  ما قبل القيام بالتقطيع أو إرسال البيانات.
- تظهر الإنشاءات المسبقة التي تعلن عن ملف تعريف متباين للاحتفاظ به
  تم تجديده مع `400 schema mismatch` للعملاء القدامى الذين لا يملكون
  enfraquecer أو contrato.
- كل حدث تجاوز وتسجيل (`blob_class`، إرسال سياسي مقابل انتظار)
  لتصدير المتصلين لا يتوافقون أثناء الطرح.

Veja [خطة استيعاب توفر البيانات](ingest-plan.md) (قائمة التحقق من الصحة) الفقرة
o تم تحديث البوابة من خلال تطبيق الاحتفاظ.

## سير عمل إعادة النسخ (تابع DA-4)

إنفاذ الاحتفاظ بالبيانات أو الخطوة الأولى. المشغلون Tambem Devem
إثبات إظهار الحياة وأوامر التكرار المستمرة في السياسة
تم تكوينه حتى يتمكن SoraFS من إعادة نسخ النقط من أجل مطابقة الشكل
com.automated.

1. **لاحظ الانجراف.** Torii تنبعث
   `overriding DA retention policy to match configured network baseline` عندما
   يقدم المتصل قيمًا غير ثابتة. الجمع بين سجل esse كوم أ
   القياس عن بعد `torii_sorafs_replication_*` للكشف عن النسخ المتماثلة الخاطئة
   يعيد نشر atrasados.
2. **النوايا المختلفة مقابل النسخ المتماثلة المباشرة.** استخدم مساعد الاستماع الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   قم بالأمر `torii.da_ingest.replication_policy` للتكوين
   Fornecida، وفك تشفير كل بيان (JSON أو Norito)، ومطابقة Faz اختياريًا
   الحمولات الصافية `ReplicationOrderV1` لملخص البيان. يا سيرة ذاتية أدعية
   كونديكو:

   - `policy_mismatch` - ملف الاحتفاظ بالبيانات المتباينة عن السياسة
     imposta (لا ينبغي أن يحدث هذا إلا إذا تم تكوين Torii بشكل سيء).
   - `replica_shortfall` - طلب النسخ المتماثلة مباشرة من خلال أقل النسخ المتماثلة
     ما هو `RetentionPolicy.required_replicas` أو ما هو أقل من السمات التي يمكنك القيام بها
     يا ألفو.

   تشير حالة الصيد التي لا تصل إلى الصفر إلى وجود عجز في عمل السيارة
   يمكن إرسال صفحة CI/on-call على الفور. ملحق أو رابط JSON إلى الحزمة
   `docs/examples/da_manifest_review_template.md` للتصويت في البرلمان.
3. **احتفظ بإعادة النسخ.** عند الإبلاغ عن النقص، قم بإصداره
   novo `ReplicationOrderV1` عبر أدوات الإدارة الموصوفة
   [سوق سعة التخزين SoraFS](../sorafs/storage-capacity-marketplace.md)
   ركبنا قاعة الاستماع مرة أخرى وتحدثنا مع مجموعة من النسخ المتماثلة. تجاوزات الفقرة
   في حالة الطوارئ، شارك في مساعدة CLI مع `iroha app da prove-availability` para
   يمكن لـ SREs الرجوع إلى كل ملخص وأدلة PDP.

تغطية التراجع الحي في `integration_tests/tests/da/replication_policy.rs`;
مجموعة ترسل سياسة متباينة للمحافظة على `/v1/da/ingest` والتحقق
يعرض بيان البحث الملف الشخصي عند محاولة المتصل.