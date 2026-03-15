---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# إعلانات مزودي متعدد المصادر والجدولة

تُلخص هذه الصفحة المواصفة القياسية في
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
استخدم هذا المستند للمخططات Norito حرفيا وسجلات التغيير؛ نسخة البوابة تُبقي إرشادات المشغلين
El SDK y el software están disponibles en los archivos SoraFS.

## إضافات مخطط Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – أكبر نطاق متصل (بايت) لكل طلب، `>= 1`.
- `min_granularity` – دقة البحث، `1 <= القيمة <= max_chunk_span`.
- `supports_sparse_offsets` – يسمح بإزاحات غير متصلة في طلب واحد.
- `requires_alignment` – Establece una conexión con `min_granularity`.
- `supports_merkle_proof` – يشير إلى دعم شاهد PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` يفرضان ترميزا قانونيا
كي تبقى cargas útiles الـ chismes حتمية.

### `StreamBudgetV1`
- Título: `max_in_flight`, `max_bytes_per_sec`, y `burst_bytes`.
- قواعد التحقق (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` está conectado a `> 0` y `<= max_bytes_per_sec`.

### `TransportHintV1`
- Título: `protocol: TransportProtocol`, `priority: u8` (versión 0-15)
  `TransportHintV1::validate`).
- Fuentes de datos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- تُرفض إدخالات البروتوكول المكررة لكل مزود.

### Mensaje `ProviderAdvertBodyV1`
- `stream_budget` Nombre: `Option<StreamBudgetV1>`.
- `transport_hints` Nombre: `Option<Vec<TransportHintV1>>`.
- كلا الحقلين يمران الآن عبر `ProviderAdmissionProposalV1` وأغلفة الحوكمة
  وfixtures الـ CLI و JSON التليمترية.## التحقق والربط بالحوكمة

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
يرفضان البيانات الوصفية غير السليمة:

- يجب أن تُفك قدرات النطاق وتستوفي حدود النطاق/التحبيب.
- تتطلب presupuestos de flujo / sugerencias de transporte قيمة TLV مطابقة من
  `CapabilityType::ChunkRangeFetch` وقائمة sugerencias غير فارغة.
- البروتوكولات المكررة والأولويات غير الصالحة تثير أخطاء تحقق قبل بث anuncios.
- تقارن أغلفة القبول propuesta/anuncios لبيانات النطاق عبر `compare_core_fields`
  حتى تُرفض cargas útiles الـ chismes غير المتطابقة مبكرا.

توجد تغطية الانحدار في
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## الأدوات وaccesorios

- Hay cargas útiles que incluyen `range_capability`, `stream_budget` y `transport_hints`.
  تحقّق عبر استجابات `/v1/sorafs/providers` y accesorios القبول؛ يجب أن تتضمن
  ملخصات JSON القدرة المحللة وميزانية البث ومصفوفات sugerencias لابتلاع التليمترية.
- `cargo xtask sorafs-admission-fixtures` يعرض presupuestos de flujo y sugerencias de transporte داخل
  artefactos JSON كي تابع لوحات المراقبة تبني الميزة.
- تشمل accesorios تحت `fixtures/sorafs_manifest/provider_admission/` الآن:
  - anuncios متعددة المصادر قياسية،
  - `multi_fetch_plan.json` Para obtener el SDK, busque y busque archivos.

## تكامل المُنسق وTorii- يعيد Torii `/v1/sorafs/providers` بيانات قدرة النطاق المحللة مع
  `stream_budget` y `transport_hints`. تُطلق تحذيرات degradar عندما يحذف
  المزودون البيانات الجديدة، وتطبق نقاط نطاق البوابة القيود نفسها للعملاء المباشرين.
- يفرض المُنسق متعدد المصادر (`sorafs_car::multi_fetch`) حدود النطاق ومحاذاة
  القدرات وstream presupuestos عند إسناد العمل. تغطي اختبارات الوحدة سيناريوهات
  trozo الكبير جدا والبحث المتناثر والتخفيض.
- يبث `sorafs_car::multi_fetch` إشارات downgrade (إخفاقات المحاذاة،
  الطلبات المخفّضة) لتمكين المشغلين من تتبع سبب استبعاد مزودين بعينهم أثناء التخطيط.

## مرجع التليمترية

Utilice Torii y Grafana **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) وقواعد التنبيه المرافقة
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| المقياس | النوع | الوسوم | الوصف |
|---------|-------|--------|-------|
| `torii_sorafs_provider_range_capability_total` | Calibre | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | مزودون يعلنون ميزات قدرة النطاق. |
| `torii_sorafs_range_fetch_throttle_events_total` | Mostrador | `reason` (`quota`, `concurrency`, `byte_rate`) | محاولات buscar للنطاق تم تخفيضها حسب السياسة. |
| `torii_sorafs_range_fetch_concurrency_current` | Calibre | — | تيارات نشطة محكومة تستهلك ميزانية التزامن المشتركة. |

Aplicación PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```استخدم عداد التخفيض لتأكيد تطبيق الحصص قبل تفعيل القيم الافتراضية للمُنسق متعدد المصادر،
ونبّه عندما يقترب التزامن من الحدود القصوى لميزانيات البث عبر أسطولك.