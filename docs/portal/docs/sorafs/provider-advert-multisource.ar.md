---
lang: ar
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e9f2cc35c57ca6e054276972f341d4fa44ff4b164a5c0bb707025b80c4e7bf25
source_last_modified: "2025-11-08T17:35:21.580244+00:00"
translation_last_reviewed: 2026-01-30
---

# إعلانات مزودي متعدد المصادر والجدولة

تُلخص هذه الصفحة المواصفة القياسية في
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
استخدم هذا المستند للمخططات Norito حرفيا وسجلات التغيير؛ نسخة البوابة تُبقي إرشادات المشغلين
وملاحظات SDK ومراجع التليمترية قريبة من بقية كتيبات التشغيل في SoraFS.

## إضافات مخطط Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – أكبر نطاق متصل (بايت) لكل طلب، `>= 1`.
- `min_granularity` – دقة البحث، `1 <= القيمة <= max_chunk_span`.
- `supports_sparse_offsets` – يسمح بإزاحات غير متصلة في طلب واحد.
- `requires_alignment` – عند التفعيل، يجب أن تصطف الإزاحات مع `min_granularity`.
- `supports_merkle_proof` – يشير إلى دعم شاهد PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` يفرضان ترميزا قانونيا
كي تبقى payloads الـ gossip حتمية.

### `StreamBudgetV1`
- الحقول: `max_in_flight`, `max_bytes_per_sec`, و`burst_bytes` اختياري.
- قواعد التحقق (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` عند وجوده يجب أن يكون `> 0` و`<= max_bytes_per_sec`.

### `TransportHintV1`
- الحقول: `protocol: TransportProtocol`, `priority: u8` (نافذة 0-15 تفرضها
  `TransportHintV1::validate`).
- البروتوكولات المعروفة: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- تُرفض إدخالات البروتوكول المكررة لكل مزود.

### إضافات `ProviderAdvertBodyV1`
- `stream_budget` اختياري: `Option<StreamBudgetV1>`.
- `transport_hints` اختياري: `Option<Vec<TransportHintV1>>`.
- كلا الحقلين يمران الآن عبر `ProviderAdmissionProposalV1` وأغلفة الحوكمة
  وfixtures الـ CLI وJSON التليمترية.

## التحقق والربط بالحوكمة

`ProviderAdvertBodyV1::validate` و`ProviderAdmissionProposalV1::validate`
يرفضان البيانات الوصفية غير السليمة:

- يجب أن تُفك قدرات النطاق وتستوفي حدود النطاق/التحبيب.
- تتطلب stream budgets / transport hints قيمة TLV مطابقة من
  `CapabilityType::ChunkRangeFetch` وقائمة hints غير فارغة.
- البروتوكولات المكررة والأولويات غير الصالحة تثير أخطاء تحقق قبل بث adverts.
- تقارن أغلفة القبول proposal/adverts لبيانات النطاق عبر `compare_core_fields`
  حتى تُرفض payloads الـ gossip غير المتطابقة مبكرا.

توجد تغطية الانحدار في
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## الأدوات وfixtures

- يجب أن تتضمن payloads إعلانات المزود `range_capability` و`stream_budget` و`transport_hints`.
  تحقّق عبر استجابات `/v1/sorafs/providers` وfixtures القبول؛ يجب أن تتضمن
  ملخصات JSON القدرة المحللة وميزانية البث ومصفوفات hints لابتلاع التليمترية.
- `cargo xtask sorafs-admission-fixtures` يعرض stream budgets وtransport hints داخل
  artefacts JSON كي تتابع لوحات المراقبة تبني الميزة.
- تشمل fixtures تحت `fixtures/sorafs_manifest/provider_admission/` الآن:
  - adverts متعددة المصادر قياسية،
  - `multi_fetch_plan.json` لكي تعيد مجموعات SDK تشغيل خطة fetch متعددة الأقران بشكل حتمي.

## تكامل المُنسق وTorii

- يعيد Torii `/v1/sorafs/providers` بيانات قدرة النطاق المحللة مع
  `stream_budget` و`transport_hints`. تُطلق تحذيرات downgrade عندما يحذف
  المزودون البيانات الجديدة، وتطبق نقاط نطاق البوابة القيود نفسها للعملاء المباشرين.
- يفرض المُنسق متعدد المصادر (`sorafs_car::multi_fetch`) حدود النطاق ومحاذاة
  القدرات وstream budgets عند إسناد العمل. تغطي اختبارات الوحدة سيناريوهات
  chunk الكبير جدا والبحث المتناثر والتخفيض.
- يبث `sorafs_car::multi_fetch` إشارات downgrade (إخفاقات المحاذاة،
  الطلبات المخفّضة) لتمكين المشغلين من تتبع سبب استبعاد مزودين بعينهم أثناء التخطيط.

## مرجع التليمترية

تغذي أداة قياس fetch النطاق في Torii لوحة Grafana **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) وقواعد التنبيه المرافقة
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| المقياس | النوع | الوسوم | الوصف |
|---------|-------|--------|-------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | مزودون يعلنون ميزات قدرة النطاق. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | محاولات fetch للنطاق تم تخفيضها حسب السياسة. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | تيارات نشطة محكومة تستهلك ميزانية التزامن المشتركة. |

أمثلة PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدم عداد التخفيض لتأكيد تطبيق الحصص قبل تفعيل القيم الافتراضية للمُنسق متعدد المصادر،
ونبّه عندما يقترب التزامن من الحدود القصوى لميزانيات البث عبر أسطولك.
