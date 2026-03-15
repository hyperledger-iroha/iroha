---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

#إعلانات واضحة متعددة المصادر والجدولة

تُلخص هذه الصفحة المواصفة القياسية في
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
استخدم هذا البحث للمخططات Norito حرفيا ويظهر تغيرات التغيير؛ نسخة البوابة تُبقي إرشادات التعليمات
وملاحظات SDK ومراجع التليمترية قريبة من حقيبة سفرات التشغيل SoraFS.

## إضافات مخطط Norito

### سعة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – أكبر نطاق متصل (بايت) لكل طلب، `>= 1`.
– `min_granularity` – تميز البحث، `1 <= القيمة <= max_chunk_span`.
- `supports_sparse_offsets` – يسمح بإدخالات غير مسموح بها في طلب واحد.
- `requires_alignment` – عند التفعيل، يجب أن تصطف الإزاحات مع `min_granularity`.
- `supports_merkle_proof` – يشير إلى دعم شاهد PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` يفرضان ترميزا قانونيا
كي تبقى حمولات الـ ثرثرة حتمية.

###`StreamBudgetV1`
- وهو: `max_in_flight`, `max_bytes_per_sec`, و`burst_bytes` اختياري.
- متطلبات التحقق (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`، `max_bytes_per_sec > 0`.
  - `burst_bytes` عند وجوده يجب أن يكون `> 0` و`<= max_bytes_per_sec`.

###`TransportHintV1`
- وهو: `protocol: TransportProtocol`, `priority: u8` (نافذة 0-15 لها)
  `TransportHintV1::validate`).
- للاتصالات المعروفة: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- تُرفض التنوعات المكررة لكل صنف.

### الاضافات `ProviderAdvertBodyV1`
- `stream_budget` اختياري: `Option<StreamBudgetV1>`.
- `transport_hints` اختياري: `Option<Vec<TransportHintV1>>`.
- كلاين الحقل يمران الآن عبر `ProviderAdmissionProposalV1` وأغلفة التمر
  والتركيبات الـ CLI وJSON التليميترية.## التحقق والربط بالربط

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
رفض البيانات الوصفية غير السليمة:

- يجب أن تُفك قدرات النطاق وتستوفي نطاق النطاق/التحبيب.
- متطلبات تدفق الميزانيات / تلميحات النقل قيمة TLV مقارنة من
  `CapabilityType::ChunkRangeFetch` وقائمة التلميحات غير فارغة.
- بسببات المكررة والأولويات غير الصالحة ولكنه لا يتم الاختيار قبل بث الإعلانات.
- تقارن أغلفة تقبل الاقتراحات/الإعلانات لبيانات النطاق عبر `compare_core_fields`
  حتى ترفض الحمولات الـ ثرثرة غير المتطابقة جيدا.

توجد تغطية في كرة القدم
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

##الأدوات والتركيبات

- يجب أن تتضمن إعلانات الحمولات الصافية `range_capability` و`stream_budget` و`transport_hints`.
  تحقّق عبر استجابات `/v2/sorafs/providers` والتركيبات القديمة؛ يجب أن تتضمن
  ملخصات JSON القدرة على تحديد وميزانية البث ومصفوفات تلميحات لابتلاع التليميرية.
- `cargo xtask sorafs-admission-fixtures` يعرض ميزانيات البث داخل وتلميحات النقل
  المصنوعات اليدوية JSON كي تتابع الخطوط الأمامية بادئ ذي بدء.
- تشمل التركيبات تحت `fixtures/sorafs_manifest/provider_admission/` الآن:
  - اعلانات متعددة المصدر،
  - `multi_fetch_plan.json` لكي تقوم مجموعات SDK بتشغيل خطة جلب متعددة الأقران بشكل حتمي.

## تكامل المُنسق وTorii- إعادة Torii `/v2/sorafs/providers` قدرة بيانات النطاق المبتكرة مع
  `stream_budget` و`transport_hints`. تُطلق تحذيرات إلى الرجوع إلى إصدار أقدم عندما يحذف
  تحتوي على البيانات الجديدة، وتطبق نقاط حدود القيود الخاصة بذاتها.
- يفرض المُنسق ذو المصادر المتعددة (`sorafs_car::multi_fetch`) نطاق النطاق و ملائم
  شانت وستريم الميزانيات عند إسناد العمل. تغطي حالة الحالة سيناريوهات
  Chunk الكبير جدا جدا المنتظر والتخفيض.
- يبث `sorafs_car::multi_fetch` إشارات التخفيض (إخفاقات العازف،
  يطلب المخفّضة) يؤدي إلى بدء تشغيلهم من حذف سبب استبعادهم بعينهم أثناء التخطيط.

## مرجع التليميترية

تغذي أداة قياس نطاق الجلب في لوحة Torii Grafana **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) وملاحظات التنبيهات
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| المقياس | النوع | الوسوم | الوصف |
|---------|-------|-------|-------|
| `torii_sorafs_provider_range_capability_total` | مقياس | `feature` (`providers`، `supports_sparse_offsets`، `requires_alignment`، `supports_merkle_proof`، `stream_budget`، `transport_hints`) | يعلنون عن ميزات النطاق. |
| `torii_sorafs_range_fetch_throttle_events_total` | عداد | `reason` (`quota`، `concurrency`، `byte_rate`) | هدف الجلب للنطاق تم تحديده بشكل شائع حسب السياسة. |
| `torii_sorafs_range_fetch_concurrency_current` | مقياس | — | بطاريات حكومة مستهلكة للاستهلاك المشترك. |

أمثلة PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```استخدم عداد التخفيض لتأكيد تطبيق الحصص قبل تفعيل القيم الافتراضية للمُنسق متعدد المصادر،
ونبّه عندما تستهدف التزامن من الحدود القصوى لميزانيات البث عبر أسطولك.