---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# توفير وتخطيط متعدد التخصصات

هذا الجزء هو المواصفات القانونية في
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
استخدم هذا المستند لنظام الملحقات Norito وسجل التغيير؛ بوابة النسخة
بعد تعليمات التشغيل السهلة، وحزم SDK وإرشادات القياس عن بعد للأخير
أفضل دفاتر التشغيل SoraFS.

## إضافة المخطط Norito

### القدرة على التحمل (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – الحد الأقصى للفاصل الزمني غير المحدود (البيانات) للخلف، `>= 1`.
- `min_granularity` – البحث عن التباين, `1 <= значение <= max_chunk_span`.
- `supports_sparse_offsets` – توفير إزاحات غير محددة في عملية واحدة.
- `requires_alignment` – إذا كان صحيحًا، سيتم تفعيل الإزاحات الإضافية إلى `min_granularity`.
- `supports_merkle_proof` – يدعم دعم свидетельств PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` مطلوب قانونيًا
الترميز الذي يسمح بتحديد الحمولات النافعة.

###`StreamBudgetV1`
- بول: `max_in_flight`، `max_bytes_per_sec`، اختياري `burst_bytes`.
- التحقق من الصحة (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`، `max_bytes_per_sec > 0`.
  - `burst_bytes`، إذا كان الأمر كذلك، فيجب أن يكون `> 0` و`<= max_bytes_per_sec`.

###`TransportHintV1`
-اللون: `protocol: TransportProtocol`, `priority: u8` (أوكنو 0-15 التحكم)
  `TransportHintV1::validate`).
- البروتوكولات المعروفة: `torii_http_range`، `quic_stream`، `soranet_relay`،
  `vendor_reserved`.
- إخلاء المسؤولية عن بروتوكول إلغاء الاشتراك.### الإضافة إلى `ProviderAdvertBodyV1`
- اختياري `stream_budget: Option<StreamBudgetV1>`.
- اختياري `transport_hints: Option<Vec<TransportHintV1>>`.
- قم بالتمرير عبر `ProviderAdmissionProposalV1`، مظاريف الإدارة،
  تركيبات CLI وJSON القياس عن بعد.

## التحقق من الصحة ومنحها للحوكمة

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
إلغاء النسخ المتماثل:

- القدرة على فك التشفير الصحيح والالتزام بالحدود
  غشائي/حبيبي.
- تيار الميزانيات / تلميحات النقل مطلوب TLV `CapabilityType::ChunkRangeFetch`
  وتلميحات غير واضحة.
- تحديد بروتوكولات النقل والأولويات غير الصحيحة
  التحقق من صحة الإعلانات القيل والقال.
- مظاريف القبول تلخص الاقتراح/الإعلانات عبر الإنترنت
  `compare_core_fields`، لإلغاء حجب الحمولات النافعة للقيل والقال.

الانحدار يحدث في الغلق
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## الأدوات والتركيبات- تتضمن الحمولات الصافية التي يوفرها مقدمو الطلبات `range_capability`، `stream_budget`
  و `transport_hints`. تحقق من خلال الإجابات `/v1/sorafs/providers` وتركيبات القبول؛
  تشتمل متطلبات السيرة الذاتية لـ JSON على إمكانات واسعة النطاق وميزانية التدفق وتلميحات ضخمة
  لاستيعاب القياس عن بعد.
- `cargo xtask sorafs-admission-fixtures` يعرض ميزانيات البث وتلميحات النقل
  في عناصر JSON الخاصة بها، لتتمكن لوحات المعلومات من التحكم في وظيفة الاتصال.
- تشمل التركيبات في `fixtures/sorafs_manifest/provider_admission/`:
  - الإعلانات القانونية المتعددة الاستخدامات،
  - `multi_fetch_plan.json`، يمكن لمجموعات SDK أن تساعد في تحديد المحددات
    خطة جلب متعددة الأقران.

## التكامل مع الأوركسترا وTorii

- Torii `/v1/sorafs/providers` يوفر إمكانات تبادلية واسعة النطاق
  هنا مع `stream_budget` و`transport_hints`. يجب أن يتم تخفيض الاقتراح المسبق عندما
  يقدم الموردون طرقًا بديلة جديدة، حيث يتم تعزيزها من خلال نقاط نهاية النطاق
  للعملاء المميزين.
- أوركسترا متعدد الاستخدامات (`sorafs_car::multi_fetch`) يخفض حدود الانطلاق،
  تمكين الإمكانيات وتدفق الميزانيات من خلال الأعمال المجدولة. تم الانتهاء من اختبارات الوحدة
  случаи сличком бользит чан, разregенногоза и خنق.
- `sorafs_car::multi_fetch` يسمح بإرجاع الإشارات إلى إصدار أقدم (تحديث تلقائي،
  اختناقات) ، بحيث يمكن للمشغلين التعامل مع مزودي الخرسانة بشكل أفضل
  تم اقتراحه من خلال التخطيط.

## أجهزة قياس عن بعد لاسلكيةجلب نطاق الأداة في Torii في Grafana لوحة القيادة **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) والتنبيهات المناسبة
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| متريكا | النوع | ميتكي | الوصف |
|---------|-----|-------|----------|
| `torii_sorafs_provider_range_capability_total` | مقياس | `feature` (`providers`، `supports_sparse_offsets`، `requires_alignment`، `supports_merkle_proof`، `stream_budget`، `transport_hints`) | مزودو الخدمة بوظائف قابلة للتخصيص. |
| `torii_sorafs_range_fetch_throttle_events_total` | عداد | `reason` (`quota`، `concurrency`، `byte_rate`) | نطاق الجلب يتم من خلال الاختناق والتجميع السياسي. |
| `torii_sorafs_range_fetch_concurrency_current` | مقياس | — | نقاط الحماية النشطة، توافقات الميزانية المحتملة. |

الأمثلة الأولية PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدم التحكم الذكي لتأكيد الإرشادات التي سبقت الإضافة
عازفو أوركسترا متعددو الأطوار، ويستجيبون للتنبيهات عند التوافق
يمكنك الاستفادة من الحد الأقصى لميزانية التدفق الخاصة بأسطولك.