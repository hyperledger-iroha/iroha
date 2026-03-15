---
lang: ar
direction: rtl
source: docs/portal/docs/da/replication-policy.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

:::note Канонический источник
هذا الجزء يعرض `docs/source/da/replication_policy.md`. آخر الإصدارات
:::

# توافر البيانات السياسية (DA-4)

_الحالة: في العمل — الركاب: Core Protocol WG / فريق التخزين / SRE_

DA استيعاب خط الأنابيب يساعد في تحديد نسبة الاحتفاظ بالمدة
فئة النقطة، الموضحة في `roadmap.md` (مسار العمل DA-4). يتم الرد على Torii
الاحتفاظ بمظاريف الاحتفاظ، مما يسمح للمتصل، إذا لم تتم الموافقة عليه
سياسة قوية تضمن أن كل مدقق/مشرف يحصل على الرعاية
لا داعي للقلق بشأن هذا العصر والتكرار بدون فرص للتخصيص.

## سياسة التسامح

| كلاس بلوب | الاحتفاظ الساخن | احتباس البرد | النسخ المتماثلة المرغوبة | كلاس فرانينيا | وسم الحوكمة |
|------------|--------------|----------------|----|----------------|----------------|
| `taikai_segment` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 أيام | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 يوما | 3 | `cold` | `da.governance` |
| _افتراضي (جميع الفئات الأصلية)_ | 6 ساعات | 30 يوما | 3 | `warm` | `da.default` |

تم إنشاء هذه الأجهزة في `torii.da_ingest.replication_policy` وتكملها
يتم تنفيذ كل شيء `/v1/da/ingest`. Torii يكرر البيانات بشكل أساسي
الاحتفاظ بالملف الشخصي والتحقق من التفضيل، إذا أرسل المتصلون رسائل غير ضرورية
ملحوظة: يمكن للمشغلين استخدام SDK قوي.

### جودة عالية Taikai

بيانات توجيه Taikai (`taikai.trm`) ترجع إلى `availability_class`
(`hot`، `warm`، أو `cold`). Torii يشجع السياسة الهادئة حتى التقطيع،
يمكن للمشغلين نسخ نسخة طبق الأصل من البث بدون تعديل
أقراص عالمية. الافتراضيات:

| سهولة الوصول إلى الدرجة | الاحتفاظ الساخن | احتباس البرد | النسخ المتماثلة المرغوبة | كلاس فرانينيا | وسم الحوكمة |
|-------------------|--------------|----------------|----|----------------|----------------|
| `hot` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 يوما | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 يوما | 3 | `cold` | `da.taikai.archive` |

إذا قمت بالتواصل عبر الإنترنت، استخدم `hot` لمشاهدة البث المباشر على أعلى مستوى
ملف تعريف قوي. قم باقتراح الإعدادات الافتراضية من خلال
`torii.da_ingest.replication_policy.taikai_availability`، إذا تم استخدامه
خلايا أخرى.

## التكوين

السياسة موجودة تحت `torii.da_ingest.replication_policy` ومتاحة
*افتراضي* شابلون بلس ماسييف تجاوز لكل فئة. معرفات الفئة
التسجيل والبدء `taikai_segment`, `nexus_lane_sidecar`,
`governance_artifact`، Libo `custom:<u16>` للإدارة الشاملة والمتوافقة.
المبادئ الأساسية للفئات هي `hot` أو `warm` أو `cold`.

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

قم بتثبيت الكتلة دون تغيير لاستخدام الإعدادات الافتراضية مرة أخرى. أتمنى أن تكون سعيدًا
الطبقة، التعرف على التجاوز الجميل؛ من أجل إزالة الأساس للفئات الجديدة،
إزالة `default_retention`.

فئات توفر Taikai يمكن إعادة اقتراحها من خلاله
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

## تطبيق الدلالة

- Torii يتم حفظ الملف التعريفي الخاص به `RetentionPolicy` على الملف الشخصي
  قبل التقطيع أو البيان.
- البيانات المسبقة التي تعلن عن ملف تعريف الاحتفاظ غير المرغوب فيه،
  تم الإلغاء مع `400 schema mismatch` بحيث لا يمكن للعملاء المتميزين قبولهم
  العقد.
- عندما يتم تجاوز تسجيل الدخول (`blob_class`, отправленная poliтica vs
  ملاحظة)، لاستقبال المتصلين غير المتوافقين خلال فترة الطرح.

اختر [خطة استيعاب توفر البيانات](ingest-plan.md) (قائمة التحقق من الصحة)
بوابة обновленного، покраывающего إنفاذ الإبقاء.

## النسخ المتماثل لسير العمل (متابعة DA-4)

الاحتفاظ بالإنفاذ — الخطوة الأولى. يجب على المشغلين أيضًا الإشارة إلى ما هو مباشر
تتخلص البيانات وأوامر النسخ من الالتزامات السياسية الصارمة،
يمكن لـ SoraFS إعادة تكرار النقط غير الضرورية تلقائيًا.

1. ** سلايد الانجراف. ** Torii رساله
   `overriding DA retention policy to match configured network baseline`
   يقوم المتصل بتفعيل ميزة الاحتفاظ الجيدة. قم بتزويد هذا السجل ب
   جهاز القياس عن بعد `torii_sorafs_replication_*` لملاحظة النقص في النسخ المتماثل
   أو عمليات إعادة الانتشار المتزايدة.
2. **النية الرائعة والنسخ المتماثلة الحية.** استخدم مساعد التدقيق الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   أمر الاتصال `torii.da_ingest.replication_policy` من التكوين،
   فك ترميز كل بيان (JSON أو Norito)، ويمكن توفيره اختياريًا
   الحمولات النافعة `ReplicationOrderV1` في ملخص البيان. هذا يتضمن موقفين:

   - `policy_mismatch` - بيان ملف تعريف الاحتفاظ الذي تم إرجاعه إلى العملاء
     الملف الشخصي (لذا لا يلزمك ذلك، إذا تم إنشاء Torii بشكل صحيح).
   - `replica_shortfall` - يتم إلغاء أمر النسخ المتماثل المباشر بعد إجراء النسخ المتماثل
     `RetentionPolicy.required_replicas`، أو شاهد ما هو أكثر من ذلك.

   يشير كود NYULE إلى وجود عجز نشط في أتمتة CI/عند الطلب
   يمكن أن تتأخر قليلا. قم بتطبيق JSON من الحزمة
   `docs/examples/da_manifest_review_template.md` للمناقشة الساخنة.
3. **أعد إعادة النسخ المتماثل.** إذا قمت بالتدقيق في النقص، قم بالتسجيل
   `ReplicationOrderV1` الجديد من خلال أدوات التحكم، الموصوفة في
   [سوق سعة التخزين SoraFS](../sorafs/storage-capacity-marketplace.md)،
   وقم بالتدقيق مرة أخرى، في حالة عدم تكرار أي شيء. لتجاوزات الطوارئ
   قم بدعم اتصال CLI مع `iroha app da prove-availability` حتى تتمكن من الاتصال بـ SRE
   هذا هو ملخص وأدلة PDP.

تغطية الانحدار تصل إلى `integration_tests/tests/da/replication_policy.rs`;
تقوم المجموعة بإجراء الاحتفاظ بالسياسة غير الملائمة في `/v1/da/ingest` والتحقق من ذلك،
أن البيان الأكثر وضوحًا يُظهر الملف الشخصي غير المقصود للمتصل.