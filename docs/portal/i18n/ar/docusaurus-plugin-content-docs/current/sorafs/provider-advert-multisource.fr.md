---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إعلانات الموردين والتخطيط متعدد المصادر

هذه الصفحة تكثف المواصفات الكنسي في
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
استخدم هذا المستند للمخططات Norito حرفيًا وسجلات التغيير؛ la copie du portail
الحفاظ على إرساليات التشغيل وملاحظات SDK ومراجع القياس عن بعد بالقرب من الباقي
من دفاتر التشغيل SoraFS.

## اجوتس أو المخطط Norito

### سعة الشاطئ (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – بالإضافة إلى الشواطئ الكبيرة المتجاورة (الثمانيات) حسب الطلب، `>= 1`.
- `min_granularity` - دقة البحث، `1 <= valeur <= max_chunk_span`.
- `supports_sparse_offsets` – السماح بالإزاحات غير المتجاورة في طلب واحد.
- `requires_alignment` – إذا كان صحيحًا، فستتم محاذاة الإزاحات على `min_granularity`.
- `supports_merkle_proof` – للإشارة إلى جائزة شحن تيموان بور.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` يطبق التشفير الكنسي
حتى تظل حمولات القيل والقال محددة.

###`StreamBudgetV1`
- الأبطال: `max_in_flight`، `max_bytes_per_sec`، `burst_bytes` أوبتيونيل.
- قواعد التحقق من الصحة (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`، `max_bytes_per_sec > 0`.
  - `burst_bytes`، لأنه موجود، وما إلى ذلك `> 0` و`<= max_bytes_per_sec`.###`TransportHintV1`
- الأبطال: `protocol: TransportProtocol`، `priority: u8` (النوافذ 0-15 زينة متساوية)
  `TransportHintV1::validate`).
- البروتوكولات connus: `torii_http_range`، `quic_stream`، `soranet_relay`،
  `vendor_reserved`.
- تم رفض مقبلات البروتوكول المزدوج من قبل المزود.

### اجوتس `ProviderAdvertBodyV1`
- خيار `stream_budget`: `Option<StreamBudgetV1>`.
- خيار `transport_hints`: `Option<Vec<TransportHintV1>>`.
- Les deux champs desormais عابرة عبر `ProviderAdmissionProposalV1`، المغلفات
  الحوكمة وتركيبات CLI وقياس JSON عن بعد.

## التحقق والاتصال بالحوكمة

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
rejettent les metadonnees mal formees:

- يجب فك سعات الشاطئ ومراعاة حدود الشاطئ/الحبيبات.
- ميزانيات التدفق / تلميحات النقل تتطلب TLV `CapabilityType::ChunkRangeFetch`
  مراسل وقائمة تلميحات غير فيديو.
- بروتوكولات النقل المزدوج والأولويات غير الصالحة تولد أخطاء
  التحقق من الصحة قبل نشر الإعلانات.
- مغلفات القبول تقارن المقترحات/الإعلانات من أجل metadonnees de plage عبر
  `compare_core_fields` يوضح أن حمولات القيل والقال غير المتوافقة يتم رفضها بالكامل.

تم العثور على غطاء الانحدار هناك
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## الأدوات والتركيبات- يجب أن تتضمن حمولات إعلانات الموردين البيانات التعريفية `range_capability`،
  `stream_budget` و`transport_hints`. Validez via les reponses `/v1/sorafs/providers` et les
  تجهيزات القبول؛ تتضمن السيرة الذاتية لـ JSON سعة التحليل وميزانية التدفق
  ولوحات التلميحات الخاصة بالقياس عن بعد.
- `cargo xtask sorafs-admission-fixtures` يعرض ميزانيات البث وينقل التلميحات فيها
  تتيح ses artefacts JSON أن تتبع لوحات المعلومات اعتماد الوظيفة.
- التركيبات الخاصة `fixtures/sorafs_manifest/provider_admission/` تتضمن الخلل:
  - الإعلانات القانونية متعددة المصادر،
  - `multi_fetch_plan.json` حتى تتمكن مجموعات SDK من تجديد خطة الجلب
    الحتمية المتعددة الأقران

## التكامل مع الأوركسترا وTorii- Torii `/v1/sorafs/providers` يعيد البيانات الوصفية لسعة الشاطئ مع
  `stream_budget` و`transport_hints`. يتم تقليل تحذيرات الرجوع إلى المستوى الأدنى عند
  يقوم الموردون بحذف البيانات الجديدة ونقاط نهاية شاطئ البوابة
  أضف الميمات المحظورة للعملاء المباشرين.
- المنسق متعدد المصادر (`sorafs_car::multi_fetch`) يعمل على إزالة حدود الحدود
  الشاطئ ومحاذاة القدرات وتدفق الميزانيات أثناء تأثير العمل.
  تغطي الاختبارات الوحدوية سيناريوهات القطع الكبيرة جدًا، والتي تسعى إلى التفريق، وما إلى ذلك
  اختناق.
- `sorafs_car::multi_fetch` نشر إشارات الرجوع إلى إصدار أقدم (رقائق المحاذاة،
  الطلبات المخفضة) حتى يتمكن المشغلون من تتبع قوي لبعض الموردين
  Ont Ete يتجاهل قلادة la Planification.

## مرجع القياس عن بعد

أداة جلب الشاطئ Torii تعمل على لوحة القيادة Grafana
**SoraFS جلب إمكانية الملاحظة** (`dashboards/grafana/sorafs_fetch_observability.json`) وآخرون
القواعد المرتبطة بالتنبيه (`dashboards/alerts/sorafs_fetch_rules.yml`).| متري | اكتب | آداب | الوصف |
|----------|------|-----------|-------------|
| `torii_sorafs_provider_range_capability_total` | مقياس | `feature` (`providers`، `supports_sparse_offsets`، `requires_alignment`، `supports_merkle_proof`، `stream_budget`، `transport_hints`) | يعلن الموردون عن وظائف سعة الشاطئ. |
| `torii_sorafs_range_fetch_throttle_events_total` | عداد | `reason` (`quota`، `concurrency`، `byte_rate`) | محاولات لجلب العروس من الشاطئ سياسيًا. |
| `torii_sorafs_range_fetch_concurrency_current` | مقياس | — | تدفقات الأنشطة المحمية تقتضي مشاركة الميزانية المتزامنة. |

أمثلة على PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدم مقياس الاختناق لتأكيد تطبيق الحصص قبل التنشيط
القيم الافتراضية لـ multi-source orchestrate، وتنبيه عند التزامن
الاقتراب من الحد الأقصى لميزانية التدفقات لأسطولك.