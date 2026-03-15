---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إعلانات ثبت متعددة الأصول والأجندة

هذه الصفحة تستأنف ملفًا قانونيًا محددًا
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
استخدم هذا المستند للمخططات Norito حرفيًا وسجلات التغيير؛ نسخة تفعل البوابة
حماية توجيه المشغلين وملاحظات SDK ومراجع القياس عن بعد الخاصة بالباقي
دوس رونبوكس SoraFS.

## Adicoes ao esquema Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` - أكبر امتداد متواصل (بايت) حسب الحاجة، `>= 1`.
- `min_granularity` - حل البحث، `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - يسمح بإزاحة أي جزء مجاور مطلوب.
- `requires_alignment` - عندما يكون صحيحًا، يتم الإزاحة من خلال `min_granularity`.
- `supports_merkle_proof` - يشير إلى دعم testemunhas PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` تشفير تطبيق كانونيكو
من أجل تحديد حمولات القيل والقال بشكل دائم.

###`StreamBudgetV1`
- الحرم الجامعي: `max_in_flight`، `max_bytes_per_sec`، `burst_bytes` اختياري.
- Regras de validacao (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`، `max_bytes_per_sec > 0`.
  - `burst_bytes`، عند التقديم، يجب أن يكون `> 0` و`<= max_bytes_per_sec`.

###`TransportHintV1`
- الحرم الجامعي: `protocol: TransportProtocol`، `priority: u8` (السنة 0-15 مطبقة
  `TransportHintV1::validate`).
- البروتوكولات المحددة: `torii_http_range`، `quic_stream`، `soranet_relay`،
  `vendor_reserved`.
- الإدخالات المكررة للبروتوكول من قبل مقدم الطلب.### أديكويس `ProviderAdvertBodyV1`
- `stream_budget` اختياري: `Option<StreamBudgetV1>`.
- `transport_hints` اختياري: `Option<Vec<TransportHintV1>>`.
- Ambos os Campos Agora passam por `ProviderAdmissionProposalV1`، مغلفات الحكم،
  تركيبات CLI وJSON للقياس عن بعد.

## Validacao e vinculacao com Governoranca

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
بيانات تعريف rejeitam مشوهة:

- يمكن لإمكانات النطاق فك التشفير واحتساب حدود النطاق/الحجم التفصيلي.
- تدفق الميزانيات / تلميحات النقل exigem um TLV `CapabilityType::ChunkRangeFetch`
  مراسل وقائمة تلميحات لا فازيا.
- بروتوكولات النقل المكررة والأولويات غير الصالحة تنطوي على أخطاء في التحقق
  قبل الإعلانات Serem ثرثرة.
- مغلفات القبول مقارنة الاقتراحات/الإعلانات لمجموعة البيانات الوصفية عبر
  `compare_core_fields` حتى يتم الرد على حمولات القيل والقال المتباينة.

تغطية التراجع في الحياة
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## تركيبات الأدوات- تتضمن حمولات الإعلانات المثبتة البيانات الوصفية `range_capability`،
  `stream_budget` و`transport_hints`. صالحة عبر ردود `/v2/sorafs/providers`
  تركيبات القبول الإلكترونية. تتضمن ملفات JSON إمكانية التحليل أو الدفق
  الميزانية ومصفوفات التلميحات لاستيعاب القياس عن بعد.
- `cargo xtask sorafs-admission-fixtures` يعرض الميزانيات وتلميحات النقل في الداخل
  تحتوي على ملفات JSON المصطنعة الخاصة بها حتى تصاحب لوحات المعلومات هذه الميزة.
- تتضمن التركيبات `fixtures/sorafs_manifest/provider_admission/` أغورا:
  - إعلانات canonicos متعددة الأصول،
  - `multi_fetch_plan.json` لإعادة إنشاء خطة جلب من مجموعات SDK
    حتمية متعددة الأقران

## التكامل مع الأوركسترا e Torii

- Torii `/v2/sorafs/providers` يعيد تحليل البيانات الوصفية للنطاق إلى جانب com
  `stream_budget` و`transport_hints`. تحذيرات التخفيض عند حدوث ذلك
  يقوم الباحثون بحذف البيانات الوصفية الجديدة، كما يتم تطبيق البوابة على نقاط نهاية النطاق
  القيود المفروضة على العملاء مباشرة.
- يا مُنسق متعدد الأصول (`sorafs_car::multi_fetch`) الآن يتم تطبيقه بحدود
  النطاق، وتعزيز القدرات، وتدفق الميزانيات من خلال العمل. وحدة
  تتضمن الاختبارات سيناريوهات كبيرة جدًا ومتناثرة واختناق.
- `sorafs_car::multi_fetch` إرسال خط الانحدار (falhas de alinhamento،
  المتطلبات المخفضة) لكي يتمكن المشغلون من تحديد مقدمي الخدمات المحددين
  للجهلاء أثناء التخطيط.

## مرجع القياس عن بعدمجموعة أدوات الجلب Torii للطعام أو لوحة القيادة Grafana
**SoraFS جلب إمكانية الملاحظة** (`dashboards/grafana/sorafs_fetch_observability.json`) e
كأنظمة التنبيه المرتبطة (`dashboards/alerts/sorafs_fetch_rules.yml`).

| متريكا | تيبو | التسميات | وصف |
|---------|------|--------|-----------|
| `torii_sorafs_provider_range_capability_total` | مقياس | `feature` (`providers`، `supports_sparse_offsets`، `requires_alignment`، `supports_merkle_proof`، `stream_budget`، `transport_hints`) | Provedores que anunciam Features دي مجموعة القدرة. |
| `torii_sorafs_range_fetch_throttle_events_total` | عداد | `reason` (`quota`، `concurrency`، `byte_rate`) | مجموعة المتدربين تجلب خنقًا سياسيًا. |
| `torii_sorafs_range_fetch_concurrency_current` | مقياس | - | تدفقات الأنشطة المحمية التي تستهلك أو مشاركة الميزانية في المطابقة. |

أمثلة على PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدم أداة التحكم في الاختناق لتأكيد تطبيق الحصص قبل التنشيط
الإعدادات الافتراضية لـ AOS تقوم بتنبيه منسق متعدد الأصول عند التزامن تقريبًا
الحد الأقصى هو تدفق الميزانية من البداية.