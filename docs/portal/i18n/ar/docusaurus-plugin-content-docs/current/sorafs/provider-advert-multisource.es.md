---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إعلان الموردين والتخطيط متعدد المصادر

هذه الصفحة تعرض المواصفات القانونية باللغة الإنجليزية
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
يستخدم هذا المستند للنماذج Norito حرفيًا وسجلات التغيير؛ نسخة البوابة
الحفاظ على دليل المشغلين وملاحظات SDK ومراجع القياس عن بعد حول الباقي
من دفاتر التشغيل SoraFS.

## إضافة الإسكيما Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – اتصال العمدة (البايت) للطلب، `>= 1`.
- `min_granularity` – دقة البحث، `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - لا يسمح بالإزاحة بدون أي متجاورة عند الطلب.
- `requires_alignment` – عندما يكون هذا صحيحًا، يجب أن تكون الإزاحات خطية مع `min_granularity`.
- `supports_merkle_proof` – يشير إلى دعم الخصية PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` تطبيق الترميز الكنسي
حتى تكون حمولات القيل والقال ثابتة ومحددة.

###`StreamBudgetV1`
- الحرم الجامعي: `max_in_flight`، `max_bytes_per_sec`، `burst_bytes` اختياري.
- قواعد التحقق (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`، `max_bytes_per_sec > 0`.
  - `burst_bytes`، عندما يكون موجودًا، يجب أن يكون `> 0` و`<= max_bytes_per_sec`.###`TransportHintV1`
- الحرم الجامعي: `protocol: TransportProtocol`، `priority: u8` (الفتحة 0-15 قابلة للتطبيق
  `TransportHintV1::validate`).
- البروتوكولات conocidos: `torii_http_range`، `quic_stream`، `soranet_relay`،
  `vendor_reserved`.
- إذا قمت بإعادة إدخال البروتوكولات المكررة من قبل المورّد.

### الإضافة إلى `ProviderAdvertBodyV1`
- `stream_budget` اختياري: `Option<StreamBudgetV1>`.
- `transport_hints` اختياري: `Option<Vec<TransportHintV1>>`.
- مجال واسع الآن فلوين بور `ProviderAdmissionProposalV1`، لوس مغلفات الحكومة،
  تركيبات CLI وJSON القياس عن بعد.

## Validación y vinculación con gobernanza

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
rechazan metadatas المشوهة:

- يجب فك تشفير سعات النطاق واستكمال حدود النطاق/الحبيبات.
- تتطلب ميزانيات التدفق / تلميحات النقل وجود TLV `CapabilityType::ChunkRangeFetch`
  بالصدفة وقائمة التلميحات ليست فارغة.
- تؤدي بروتوكولات نقل النسخ المكررة والأولويات غير الصالحة إلى حدوث أخطاء في التحقق من الصحة
  قبل أن تصبح الإعلانات ثرثرة.
- مغلفات القبول تقارن الاقتراحات/الإعلانات لنطاق البيانات الوصفية
  `compare_core_fields` حتى يتم إعادة تحميل حمولات القيل والقال التي تم حذفها مؤقتًا.

La cobertura de regression vive en
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## الأدوات والتركيبات- يجب أن تشتمل حمولات إعلانات المورِّد على البيانات الوصفية `range_capability`،
  `stream_budget` و`transport_hints`. تم التحقق من صحة الرد عبر `/v2/sorafs/providers` y
  تركيبات القبول؛ يجب أن تتضمن ملخصات JSON القدرة على التحليل،
  ميزانية الدفق ومصفوفات التلميحات لاستيعاب القياس عن بعد.
- `cargo xtask sorafs-admission-fixtures` يعرض ميزانيات البث وتلميحات النقل داخلها
  تعمل عناصر JSON الخاصة بها على تمكين لوحات المعلومات من تفعيل الميزة.
- تتضمن التركيبات التالية `fixtures/sorafs_manifest/provider_admission/` الآن:
  - إعلانات canónicos متعددة الأصول،
  - بديل بدون نطاق لاختبارات الرجوع إلى إصدار أقدم، و
  - `multi_fetch_plan.json` حتى تتمكن مجموعات SDK من إنشاء خطة جلب
    تحديد متعدد الأقران.

## التكامل مع المشغل وTorii- Torii `/v2/sorafs/providers` يقوم بتحليل البيانات الوصفية ذات سعة النطاق جنبًا إلى جنب مع
  `stream_budget` و`transport_hints`. يتم إلغاء إخطارات الرجوع إلى المستوى الأدنى عندما يتم ذلك
  يحذف الموردون البيانات التعريفية الجديدة ونقاط النهاية في نطاق البوابة التي يتم تطبيقها
  القيود المفروضة على العملاء المباشرين.
- الأوركيستادور متعدد الأصول (`sorafs_car::multi_fetch`) الآن يكمل الحدود
  النطاق، وتخصيص القدرات، وتدفق الميزانيات لتخصيص العمل. لاس بروباس الوحدويين
  سيناريوهات القطعة كبيرة الحجم، ومشتتة، واختناق.
- `sorafs_car::multi_fetch` يصدر إشارات الرجوع إلى إصدار أقدم (سقوط التخصيص،
  تم التحكم في الطلبات) لكي يقوم المشغلون بحذفها
  موردون محددون أثناء التخطيط.

## مرجع القياس عن بعد

أداة جلب النطاق Torii تعمل على لوحة القيادة Grafana
**SoraFS جلب إمكانية الملاحظة** (`dashboards/grafana/sorafs_fetch_observability.json`) ذ
Las Reglas de notification asociadas (`dashboards/alerts/sorafs_fetch_rules.yml`).| متريكا | تيبو | اتيكيت | الوصف |
|---------|------|----------|-------------|
| `torii_sorafs_provider_range_capability_total` | مقياس | `feature` (`providers`، `supports_sparse_offsets`، `requires_alignment`، `supports_merkle_proof`، `stream_budget`، `transport_hints`) | يعلن الموردون عن ميزات سعة المدى. |
| `torii_sorafs_range_fetch_throttle_events_total` | عداد | `reason` (`quota`، `concurrency`، `byte_rate`) | نوايا جلب النطاقات من خلال خنق المجموعات السياسية. |
| `torii_sorafs_range_fetch_concurrency_current` | مقياس | — | تيارات الأنشطة المحمية التي تستهلك شرط التزامن المشترك. |

أمثلة على PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدام أداة التحكم في الاختناق لتأكيد تطبيق القطع قبل تأهيله
القيم الناتجة عن خلل الأوركيستراد متعدد المصادر والتنبيه عند حدوث التزامن
ابحث عن الحد الأقصى من تدفقات أسطولك.