---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# العديد من الإعلانات والإعلانات الترويجية

تحتوي إحدى صفحات الدرج على المواصفات الأساسية الحالية التي تلخص ملخصًا:
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
تعد مخططات Norito وسجلات التغيير بمثابة دستاويز بمهارة الاستخدام؛ بوابة كاي كابي
تعد مستندات IP وSDK وملاحظات SoraFS من أحدث دفاتر التشغيل.

## تمت إضافة مخطط Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – فی دخواست سبے بڑا مسلسلspan (بايت)، `>= 1`.
- `min_granularity` – ابحث عن ریزولوشن، `1 <= قدر <= max_chunk_span`.
- `supports_sparse_offsets` – هذا هو نوع آخر من سلسلة الإزاحات التي تسمح لك بذلك.
- `requires_alignment` – إذا كان صحيحًا، فستتم إزاحة `min_granularity` وفقًا للمحاذاة المطلوبة.
- `supports_merkle_proof` – شاهد PoR يظهر في الرياضة.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` التشفير المتعارف عليه
هذه حمولات ثرثرة حتمية.

###`StreamBudgetV1`
- التحويل: `max_in_flight`, `max_bytes_per_sec`, اختیاری `burst_bytes`.
- قواعد التحقق (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`، `max_bytes_per_sec > 0`.
  - `burst_bytes` موجود ہو تو `> 0` و `<= max_bytes_per_sec` ہونا چاہیے۔

###`TransportHintV1`
- التصنيف: `protocol: TransportProtocol`, `priority: u8` (0-15 عامًا)
  `TransportHintV1::validate` نافذ كرتا).
- البروتوكولات المعروفة: `torii_http_range`، `quic_stream`، `soranet_relay`،
  `vendor_reserved`.
- فيما يتعلق بإدخالات بروتوكول المزود، يتم إرسالها إلى البوابة.### تمت إضافة `ProviderAdvertBodyV1`
- اختیاری `stream_budget: Option<StreamBudgetV1>`.
- اختیاری `transport_hints: Option<Vec<TransportHintV1>>`.
- تم تطوير دونوڤيلز اب `ProviderAdmissionProposalV1`، ومظاريف الإدارة، وتركيبات CLI، وقياس JSON عن بعد.

## التحقق من صحة وحوكمة ملزمة

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
ملخص البيانات الوصفية التي تم الحصول عليها:

- قدرات النطاق لفك تشفير ما هو محدد وحدود الامتداد/التفاصيل حسب ما هو مطلوب.
- تدفق الميزانيات / تلميحات النقل إلى `CapabilityType::ChunkRangeFetch` TLV وقائمة التلميحات غير الفارغة مطلوبة.
- بروتوكولات النقل العامة وغير الصحيحة من الأولويات، القيل والقال، بالإضافة إلى أخطاء التحقق من الصحة.
- مظاريف القبول `compare_core_fields` عبارة عن اقتراح/إعلانات عبارة عن بيانات وصفية للنطاق ومقارنة البطاقة وعدم تطابق حمولات القيل والقال.

تغطية الانحدار موجودة:
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## الأدوات والتركيبات- الحمولات النافعة لإعلان الموفر تشمل `range_capability` و`stream_budget` و`transport_hints` وهي تشمل ما هو مطلوب.
  استجابات `/v2/sorafs/providers` وتجهيزات القبول للتحقق من صحة البطاقة؛ تتضمن ملخصات JSON القدرة على التحليل وميزانية البث ومصفوفات التلميحات التي تتضمن المزيد من المعلومات حول استيعاب القياس عن بعد.
- `cargo xtask sorafs-admission-fixtures` تحتوي عناصر JSON على ميزانيات البث وتلميحات النقل بالإضافة إلى لوحات المعلومات التي تتميز بمسار التبني.
- `fixtures/sorafs_manifest/provider_admission/` تحت التركيبات شامل البطاقة:
  - إعلانات أساسية متعددة المصادر،
  - `multi_fetch_plan.json` تحتوي على مجموعات SDK لإعادة تشغيل خطة الجلب المتعددة الأقران.

## المنسق وTorii الفائزون

- Torii `/v2/sorafs/providers` بيانات تعريف قدرة النطاق التي تم تحليلها مثل `stream_budget` و`transport_hints` واپس كرتا.
  يحرص مقدمو الخدمات على البيانات الوصفية الجديدة على الرجوع إلى إصدار سابق للتحذيرات، ونقاط نهاية نطاق البوابة للعملاء الذين يفرضون قيودًا أو قيودًا.
- منسق متعدد المصادر (`sorafs_car::multi_fetch`) عبر حدود النطاق، ومواءمة القدرة، وميزانيات الدفق لمهمة العمل ے دوران فرض کرتا ہے۔ قد تكون اختبارات الوحدة كبيرة جدًا، ومتفرقة، وسيناريوهات الاختناق تتضمن الكثير.
- `sorafs_car::multi_fetch` إشارات الرجوع إلى إصدار أقدم (فشل المحاذاة، الطلبات المُقيدة) دفق مشغلي كرتا ہے تاکہ دیکھ سکیں کہ پلاننج کے دوران موفري الخصوصية کیوں تخطي ہوئے.

## مرجع القياس عن بعدTorii أدوات جلب النطاق **SoraFS إمكانية ملاحظة الجلب** لوحة معلومات Grafana
(`dashboards/grafana/sorafs_fetch_observability.json`) وقواعد التنبيه ذات الصلة
(`dashboards/alerts/sorafs_fetch_rules.yml`) تغذية الكرت.

| متري | اكتب | التسميات | الوصف |
|--------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | مقياس | `feature` (`providers`، `supports_sparse_offsets`، `requires_alignment`، `supports_merkle_proof`، `stream_budget`، `transport_hints`) | تعلن ميزات قدرة النطاق عن موفري الخدمة. |
| `torii_sorafs_range_fetch_throttle_events_total` | عداد | `reason` (`quota`، `concurrency`، `byte_rate`) | السياسة تتوافق مع محاولات جلب النطاق المحدود. |
| `torii_sorafs_range_fetch_concurrency_current` | مقياس | — | استخدام ميزانية التزامن المشتركة تيارات حراسة نشطة. |

مثال على مقتطفات PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

يتم مراقبة تطبيق الحصص من خلال استخدام عداد الاختناق، حيث تم تنشيط الإعدادات الافتراضية للمنسق متعدد المصادر، وعندما يتزامن الأسطول مع الحد الأقصى لميزانية التدفق، فإنه يسمح بتنبيه الآخرين.