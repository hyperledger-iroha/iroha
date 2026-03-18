---
lang: pt
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
A solução de problemas Norito é a mesma e a mais importante. نسخة البوابة تُبقي إرشادات المشغلين
O SDK e o SDK do software estão disponíveis no SoraFS.

## إضافات مخطط Norito

### قدرة النطاق (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – Você pode usar o `>= 1`.
- `min_granularity` – `1 <= القيمة <= max_chunk_span`.
- `supports_sparse_offsets` – você pode fazer isso sem problemas.
- `requires_alignment` – Não há nenhum problema com `min_granularity`.
- `supports_merkle_proof` – é um problema de PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` `from_bytes` `ProviderCapabilityRangeV1::to_bytes`
Isso é útil para cargas úteis e fofocas.

### `StreamBudgetV1`
- Nome: `max_in_flight`, `max_bytes_per_sec`, e `burst_bytes`.
- Nome de usuário (`StreamBudgetV1::validate`):
  -`max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes` é o mesmo que `> 0` e `<= max_bytes_per_sec`.

###`TransportHintV1`
- Nome: `protocol: TransportProtocol`, `priority: u8` (faixa 0-15 etapas
  `TransportHintV1::validate`).
- Nomes de usuário: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- تُرفض إدخالات البروتوكول المكررة لكل مزود.

### `ProviderAdvertBodyV1`
- `stream_budget` Nome: `Option<StreamBudgetV1>`.
- `transport_hints` Nome: `Option<Vec<TransportHintV1>>`.
- كلا الحقلين يمران الآن عبر `ProviderAdmissionProposalV1` وأغلفة الحوكمة
  وfixtures são CLI e JSON.

## التحقق والربط بالحوكمة

`ProviderAdvertBodyV1::validate` e `ProviderAdmissionProposalV1::validate`
يرفضان البيانات الوصفية غير السليمة:

- يجب أن تُفك قدرات النطاق وتستوفي حدود النطاق/التحبيب.
- تتطلب orçamentos de fluxo / dicas de transporte قيمة TLV مطابقة من
  `CapabilityType::ChunkRangeFetch` dicas de instruções aqui.
- البروتوكولات المكررة والأولويات غير الصالحة تثير أخطاء تحقق قبل بث anúncios.
- تقارن أغلفة القبول proposta/anúncios لبيانات النطاق عبر `compare_core_fields`
  حتى تُرفض payloads e fofocas غير المتطابقة مبكرا.

توجد تغطية الانحدار في
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Jogos e luminárias

- Você pode usar cargas úteis como `range_capability`, `stream_budget` e `transport_hints`.
  تحقّق عبر استجابات `/v1/sorafs/providers` وfixtures القبول؛ يجب أن تتضمن
  ملخصات JSON القدرة المحللة وميزانية البث ومصفوفات dicas لابتلاع التليمترية.
- `cargo xtask sorafs-admission-fixtures` يعرض orçamentos de fluxo e dicas de transporte داخل
  artefacts JSON é o que você precisa para usar o arquivo JSON.
- Ajustes de luminárias em `fixtures/sorafs_manifest/provider_admission/`:
  - anúncios متعددة المصادر قياسية,
  - `multi_fetch_plan.json` para que o SDK do SDK possa ser usado para buscar o recurso de busca.

## تكامل المُنسق وTorii- يعيد Torii `/v1/sorafs/providers` بيانات قدرة النطاق المحللة مع
  `stream_budget` e `transport_hints`. Faça o downgrade do seu computador
  Você pode fazer isso sem precisar de mais nada.
- يفرض المُنسق متعدد المصادر (`sorafs_car::multi_fetch`) حدود النطاق ومحاذاة
  Orçamentos de fluxo e fluxo de trabalho. تغطي اختبارات الوحدة سيناريوهات
  chunk الكبير جدا والبحث المتناثر والتخفيض.
- `sorafs_car::multi_fetch` إشارات downgrade (إخفاقات المحاذاة,
  الطلبات المخفّضة) لتمكين المشغلين من تتبع سبب استبعاد مزودين بعينهم أثناء التخطيط.

## مرجع التليمترية

Você pode usar fetch em Torii ou Grafana **SoraFS Fetch Observability**
(`dashboards/grafana/sorafs_fetch_observability.json`) وقواعد التنبيه المرافقة
(`dashboards/alerts/sorafs_fetch_rules.yml`).

| المقياس | النوع | الوسوم | الوصف |
|--------|-------|--------|-------|
| `torii_sorafs_provider_range_capability_total` | Medidor | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | مزودون يعلنون ميزات قدرة النطاق. |
| `torii_sorafs_range_fetch_throttle_events_total` | Contador | `reason` (`quota`, `concurrency`, `byte_rate`) | Isso significa que fetch é o que você precisa para fazer isso. |
| `torii_sorafs_range_fetch_concurrency_current` | Medidor | — | Verifique se há algum problema com isso. |

Como usar PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

استخدم عداد التخفيض لتأكيد تطبيق الحصص قبل تفعيل القيم الافتراضية للمُنسق متعدد المصادر,
ونبّه عندما يقترب التزامن من الحدود القصوى لميزانيات البث عبر أسطولك.