---
lang: ar
direction: rtl
source: docs/source/da/replication_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 70163ed6740c80c48c78ae918c37d34e0022ab97ffabce6d451bbf85060e24b4
source_last_modified: "2026-01-22T15:38:30.661849+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

# سياسة النسخ المتماثل لتوفر البيانات (DA-4)

_الحالة: قيد التقدم — المالكون: مجموعة عمل البروتوكول الأساسي / فريق التخزين / SRE_

يفرض مسار استيعاب DA الآن أهدافًا محددة للاحتفاظ
كل فئة blob موصوفة في `roadmap.md` (مسار العمل DA-4). Torii يرفض ذلك
استمرار مظاريف الاحتفاظ المقدمة من المتصل والتي لا تتطابق مع التكوين
السياسة، مما يضمن أن كل عقدة تحقق/تخزين تحتفظ بما هو مطلوب
عدد العصور والنسخ المتماثلة دون الاعتماد على نية المرسل.

## السياسة الافتراضية

| فئة النقطة | الاحتفاظ الساخن | احتباس البرد | النسخ المتماثلة المطلوبة | فئة التخزين | وسم الحوكمة |
|------------|--------------|----------------|----|----------------|----------------|
| `taikai_segment` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `nexus_lane_sidecar` | 6 ساعات | 7 أيام | 4 | `warm` | `da.sidecar` |
| `governance_artifact` | 12 ساعة | 180 يوما | 3 | `cold` | `da.governance` |
| _الافتراضي (جميع الفئات الأخرى)_ | 6 ساعات | 30 يوما | 3 | `warm` | `da.default` |

يتم تضمين هذه القيم في `torii.da_ingest.replication_policy` ويتم تطبيقها على
جميع عمليات الإرسال `/v1/da/ingest`. يعيد Torii كتابة البيانات مع فرضها
ملف تعريف الاحتفاظ ويصدر تحذيرًا عندما يقدم المتصلون قيمًا غير متطابقة
يمكن للمشغلين اكتشاف حزم SDK التي لا معنى لها.

### فئات توفر Taikai

تتضمن بيانات توجيه Taikai (البيانات التعريفية `taikai.trm`) الآن
تلميح `availability_class` (`Hot`، أو `Warm`، أو `Cold`). عند وجوده، Torii
يحدد ملف تعريف الاحتفاظ المطابق من `torii.da_ingest.replication_policy`
قبل تقسيم الحمولة، مما يسمح لمشغلي الأحداث بالرجوع إلى المستوى غير النشط
عمليات التسليم دون تحرير جدول السياسة العالمية. الإعدادات الافتراضية هي:

| فئة التوفر | الاحتفاظ الساخن | احتباس البرد | النسخ المتماثلة المطلوبة | فئة التخزين | وسم الحوكمة |
|--------------------|--------------|----------------|-------------------|----------------|----------------|
| `hot` | 24 ساعة | 14 يوما | 5 | `hot` | `da.taikai.live` |
| `warm` | 6 ساعات | 30 يوما | 4 | `warm` | `da.taikai.warm` |
| `cold` | 1 ساعة | 180 يوما | 3 | `cold` | `da.taikai.archive` |

إذا أغفل البيان `availability_class`، فسيعود مسار الاستيعاب إلى
`hot` بحيث تحتفظ مجموعات البث المباشر بمجموعة النسخ المتماثلة الكاملة. يمكن للمشغلين
تجاوز هذه القيم عن طريق تحرير الجديد
كتلة `torii.da_ingest.replication_policy.taikai_availability` في التكوين.

## التكوين

تعيش هذه السياسة ضمن `torii.da_ingest.replication_policy` وتكشف عن ملف
*قالب افتراضي* بالإضافة إلى مجموعة من التجاوزات لكل فئة. معرفات الفئة هي
غير حساس لحالة الأحرف ويقبل `taikai_segment`، `nexus_lane_sidecar`،
`governance_artifact`، أو `custom:<u16>` للملحقات المعتمدة من قبل الإدارة.
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

اترك الكتلة دون تغيير لتعمل مع الإعدادات الافتراضية المذكورة أعلاه. لتشديد أ
فئة، تحديث تجاوز المطابقة؛ لتغيير خط الأساس للفئات الجديدة،
تحرير `default_retention`.لضبط فئات توفر Taikai المحددة، قم بإضافة إدخالات ضمن
`torii.da_ingest.replication_policy.taikai_availability`:

```toml
[[torii.da_ingest.replication_policy.taikai_availability]]
availability_class = "warm"
[torii.da_ingest.replication_policy.taikai_availability.retention]
hot_retention_secs = 14400         # 4 h
cold_retention_secs = 604800       # 7 d
required_replicas = 4
storage_class = "warm"
governance_tag = "da.taikai.warm"
```

## دلالات التنفيذ

- يستبدل Torii `RetentionPolicy` بملف التعريف المفروض
  قبل التقطيع أو الانبعاث الواضح.
- يتم رفض البيانات المعدة مسبقًا والتي تعلن عن عدم تطابق ملف تعريف الاحتفاظ
  مع `400 schema mismatch` لذلك لا يمكن للعملاء الذين لا معنى لهم إضعاف العقد.
- يتم تسجيل كل حدث تجاوز (`blob_class`، تم الإرسال مقابل السياسة المتوقعة)
  لتسليط الضوء على المتصلين غير المتوافقين أثناء الطرح.

راجع `docs/source/da/ingest_plan.md` (قائمة التحقق من الصحة) للتعرف على البوابة المحدثة
تغطية إنفاذ الاحتفاظ.

## سير عمل إعادة النسخ (متابعة DA-4)

إن فرض الاحتفاظ هو الخطوة الأولى فقط. ويجب على المشغلين أيضًا إثبات ذلك
تظل البيانات المباشرة وأوامر النسخ المتماثل متوافقة مع السياسة التي تم تكوينها
يمكن لـ SoraFS إعادة نسخ النقط غير المتوافقة تلقائيًا.

1. **احترس من الانجراف.** ينبعث Torii
   `overriding DA retention policy to match configured network baseline` كلما
   يرسل المتصل قيم الاحتفاظ التي لا معنى لها. إقران هذا السجل مع
   `torii_sorafs_replication_*` القياس عن بعد لاكتشاف النقص في النسخ المتماثلة أو تأخيرها
   عمليات إعادة الانتشار.
2. **الهدف المختلف مقابل النسخ المتماثلة المباشرة.** استخدم مساعد التدقيق الجديد:

   ```bash
   cargo xtask da-replication-audit \
     --config configs/iroha/torii.toml \
     --manifest spool/da/manifests/*.json \
     --replication-order artifacts/da/orders/*.norito \
     --json-out artifacts/da/replication_audit.json
   ```

   يقوم الأمر بتحميل `torii.da_ingest.replication_policy` من الملف المتوفر
   config، ويفك تشفير كل بيان (JSON أو Norito)، ويطابق بشكل اختياري أي بيان
   حمولات `ReplicationOrderV1` حسب ملخص البيان. الملخص يشير إلى اثنين
   الشروط:

   - `policy_mismatch` – يختلف ملف تعريف الاحتفاظ الظاهر عن المطبق
     السياسة (لا ينبغي أن يحدث هذا أبدًا ما لم يتم تكوين Torii بشكل خاطئ).
   - `replica_shortfall` - يطلب أمر النسخ المتماثل المباشر نسخًا متماثلة أقل من
     `RetentionPolicy.required_replicas` أو يوفر تعيينات أقل من تلك الخاصة به
     الهدف.

   تشير حالة الخروج غير الصفرية إلى وجود عجز نشط، لذا فإن أتمتة CI/عند الطلب
   يمكن الصفحة على الفور. قم بإرفاق تقرير JSON إلى ملف
   حزمة `docs/examples/da_manifest_review_template.md` لأصوات البرلمان.
3. **تشغيل إعادة النسخ المتماثل.** عندما تبلغ عملية التدقيق عن وجود عجز، قم بإصدار ملف جديد
   `ReplicationOrderV1` عبر أدوات الإدارة الموضحة في
   `docs/source/sorafs/storage_capacity_marketplace.md` وأعد تشغيل التدقيق
   حتى تتقارب مجموعة النسخ المتماثلة. بالنسبة لتجاوزات الطوارئ، قم بإقران مخرج CLI
   مع `iroha app da prove-availability` بحيث يمكن لـ SREs الرجوع إلى نفس الملخص
   وأدلة PDP.

توجد تغطية الانحدار في `integration_tests/tests/da/replication_policy.rs`؛
يرسل الجناح سياسة احتفاظ غير متطابقة إلى `/v1/da/ingest` ويتحقق منها
أن البيان الذي تم جلبه يكشف عن الملف الشخصي المفروض بدلاً من المتصل
نية.

## القياس عن بعد ولوحات المعلومات لإثبات الصحة (جسر DA-5)

يتطلب عنصر خريطة الطريق **DA-5** أن تكون نتائج إنفاذ PDP/PoTR قابلة للتدقيق فيها
في الوقت الحقيقي. تقوم أحداث `SorafsProofHealthAlert` الآن بقيادة مجموعة مخصصة من
مقاييس Prometheus:

-`torii_sorafs_proof_health_alerts_total{provider_id,trigger,penalty}`
-`torii_sorafs_proof_health_pdp_failures{provider_id}`
-`torii_sorafs_proof_health_potr_breaches{provider_id}`
-`torii_sorafs_proof_health_penalty_nano{provider_id}`
-`torii_sorafs_proof_health_cooldown{provider_id}`
-`torii_sorafs_proof_health_window_end_epoch{provider_id}`

لوحة **SoraFS PDP & PoTR Health** Grafana
(`dashboards/grafana/sorafs_pdp_potr_health.json`) يعرض الآن تلك الإشارات:- * إثبات التنبيهات الصحية عن طريق Trigger * الرسوم البيانية لمعدلات التنبيه عن طريق علامة الزناد / العقوبة
  يمكن لمشغلي Taikai/CDN إثبات ما إذا كانت الضربات PDP فقط أو PoTR فقط أو الضربات المزدوجة
  إطلاق النار.
- *يقدم مقدمو الخدمة في فترة التهدئة* تقريرًا عن المبلغ المباشر لمقدمي الخدمات الموجودين حاليًا تحت مستوى
  SorafsProofHealthAlert فترة التهدئة.
- *لقطة نافذة إثبات الصحة* تدمج عدادات PDP/PoTR، ومبلغ العقوبة،
  علامة التهدئة، وفترة نهاية نافذة الإضراب لكل مزود حتى مراجعي الإدارة
  يمكن إرفاق الجدول بحزم الحادث.

يجب أن تربط سجلات التشغيل هذه اللوحات عند تقديم أدلة إنفاذ جدول أعمال التنمية؛ هم
ربط فشل دليل تيار CLI مباشرة بالبيانات التعريفية للعقوبة على السلسلة و
قم بتوفير خطاف إمكانية المراقبة الموضح في خريطة الطريق.