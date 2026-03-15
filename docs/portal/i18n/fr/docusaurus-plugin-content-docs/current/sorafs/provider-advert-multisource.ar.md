---
lang: fr
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
Le SDK est également compatible avec les fichiers SoraFS.

## إضافات مخطط Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – أكبر نطاق متصل (بايت) لكل طلب، `>= 1`.
- `min_granularity` – pour `1 <= القيمة <= max_chunk_span`.
- `supports_sparse_offsets` – يسمح بإزاحات غير متصلة في طلب واحد.
- `requires_alignment` – Utilisez la fonction `min_granularity`.
- `supports_merkle_proof` – يشير إلى دعم شاهد PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` يفرضان ترميزا قانونيا
Il s'agit de charges utiles pour les potins.

### `StreamBudgetV1`
- Nom : `max_in_flight`, `max_bytes_per_sec`, et `burst_bytes`.
- قواعد التحقق (`StreamBudgetV1::validate`) :
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` est également compatible avec `> 0` et `<= max_bytes_per_sec`.

### `TransportHintV1`
- Type : `protocol: TransportProtocol`, `priority: u8` (niveau 0-15)
  `TransportHintV1::validate`).
- Paramètres de référence : `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- تُرفض إدخالات البروتوكول المكررة لكل مزود.

### إضافات `ProviderAdvertBodyV1`
- `stream_budget` Nom : `Option<StreamBudgetV1>`.
- `transport_hints` Nom : `Option<Vec<TransportHintV1>>`.
- كلا الحقلين يمران الآن عبر `ProviderAdmissionProposalV1` وأغلفة الحوكمة
  وfixtures الـ CLI وJSON التليمترية.

## التحقق والربط بالحوكمة

`ProviderAdvertBodyV1::validate` et `ProviderAdmissionProposalV1::validate`
يرفضان البيانات الوصفية غير السليمة:

- يجب أن تُفك قدرات النطاق وتستوفي حدود النطاق/التحبيب.
- تتطلب budgets de flux / conseils de transport pour TLV مطابقة من
  `CapabilityType::ChunkRangeFetch` وقائمة astuces غير فارغة.
- Les publicités et les publicités sont également liées aux publicités.
- تقارن أغلفة القبول proposition/annonces لبيانات النطاق عبر `compare_core_fields`
  Il s'agit de charges utiles pour les potins et les médias.

توجد تغطية الانحدار في
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## الأدوات وfixtures

- Vous pouvez utiliser les charges utiles comme `range_capability` et `stream_budget` et `transport_hints`.
  تحقّق عبر استجابات `/v2/sorafs/providers` وfixtures القبول؛ يجب أن تتضمن
  Les astuces JSON et les astuces pour les choses à faire.
- `cargo xtask sorafs-admission-fixtures` pour les budgets de flux et les conseils de transport ici
  artefacts JSON est un élément de référencement.
- تشمل luminaires تحت `fixtures/sorafs_manifest/provider_admission/` الآن :
  - annonces متعددة المصادر قياسية،
  - `multi_fetch_plan.json` pour le SDK de récupération de données à partir de la récupération.

## تكامل المُنسق وTorii- يعيد Torii `/v2/sorafs/providers` بيانات قدرة النطاق المحللة مع
  `stream_budget` et `transport_hints`. تُطلق تحذيرات downgrade عندما يحذف
  المزودون البيانات الجديدة، وتطبق نقاط نطاق البوابة القيود نفسها للعملاء المباشرين.
- يفرض المُنسق متعدد المصادر (`sorafs_car::multi_fetch`) حدود النطاق ومحاذاة
  Les budgets de flux et de flux sont également importants. تغطي اختبارات الوحدة سيناريوهات
  chunk الكبير جدا والبحث المتناثر والتخفيض.
- يبث `sorafs_car::multi_fetch` downgrade (pour le downgrade)
  الطلبات المخفّضة) لتمكين المشغلين من تتبع سبب استبعاد مزودين بعينهم أثناء التخطيط.

## مرجع التليمترية

Utilisez la fonction fetch pour Torii et Grafana **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) وقواعد التنبيه المرافقة
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| المقياس | النوع | الوسوم | الوصف |
|---------|-------|--------|-------|
| `torii_sorafs_provider_range_capability_total` | Jauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | مزودون يعلنون ميزات قدرة النطاق. |
| `torii_sorafs_range_fetch_throttle_events_total` | Compteur | `reason` (`quota`, `concurrency`, `byte_rate`) | محاولات fetch للنطاق تم تخفيضها حسب السياسة. |
| `torii_sorafs_range_fetch_concurrency_current` | Jauge | — | تيارات نشطة محكومة تستهلك ميزانية التزامن المشتركة. |

Utiliser PromQL :

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدم عداد التخفيض لتأكيد تطبيق الحصص قبل تفعيل القيم الافتراضية للمُنسق متعدد المصادر،
ونبّه عندما يقترب التزامن من الحدود القصوى لميزانيات البث عبر أسطولك.