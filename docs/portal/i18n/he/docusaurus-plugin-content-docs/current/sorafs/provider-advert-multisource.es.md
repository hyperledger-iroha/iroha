---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# הסברה מרובת מקורות ותכנון

Esta página destila la especificación canónica en
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Usa ese documento para los esquemas Norito מילה במילה y los changelogs; la copia del portal
mantiene la guía de operadores, las notas de SDK y las referencias de telemetría cerca del resto
de los runbooks de SoraFS.

## Adiciones al esquema Norito

### Capacidad de Rango (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – ראש העיר tramo contiguo (בתים) por solicitud, `>= 1`.
- `min_granularity` – resolución de búsqueda, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` - היתר קיזוז ללא התחייבות.
- `requires_alignment` – cuando es true, los offsets deben alinearse con `min_granularity`.
- `supports_merkle_proof` – אינדיקציה סופורטה de testigos PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` קוד אפליקני קנוני
para que los payloads de gossip sigan siendo deterministas.

### `StreamBudgetV1`
- קמפוס: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` אופציונלי.
- Regglas de validación (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, cuando está presente, debe ser `> 0` y `<= max_bytes_per_sec`.

### `TransportHintV1`
- קמפוס: `protocol: TransportProtocol`, `priority: u8` (ventana 0-15 aplicada por
  `TransportHintV1::validate`).
- פרוטוקולים קונוסידוס: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Se rechazan entradas duplicadas de protocolo por proveedor.

### Adiciones a `ProviderAdvertBodyV1`
- `stream_budget` אופציונלי: `Option<StreamBudgetV1>`.
- `transport_hints` אופציונלי: `Option<Vec<TransportHintV1>>`.
- Ambos campos ahora fluyen por `ProviderAdmissionProposalV1`, los envelopes de gobernanza,
  los fixtures de CLI y el JSON telemétrico.

## אימות וניסיון עם גוברננזה

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
rechazan metadatos malformados:

- Las capacidades de rango deben decodificarse y cumplir los límites de span/granularidad.
- תקציבי זרם / רמזים לתחבורה נדרשים ל-TLV `CapabilityType::ChunkRangeFetch`
  coincidente y una list de hints no vacía.
- Protocolos de transporte duplicados y prioridades inválidas generan errores de validación
  antes de que los adverts se gossipeen.
- Los envelopes de admisión להשוואת הצעה/מודעות עבור metadatos de rango vía
  `compare_core_fields` para que los payloads de gossip desalineados se rechacen temprano.

La cobertura de regresión vive en
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## מתקנים ומכשירים- מטענים פרסומיים של הודעות מוכיחות כוללות מטא נתונים `range_capability`,
  `stream_budget` y `transport_hints`. Valida via respuestas de `/v2/sorafs/providers` y
  אביזרי כניסה; los resúmenes JSON deben כולל את capacidad parseada,
  el stream budget y los arrays de hints para ingestión de telemetria.
- `cargo xtask sorafs-admission-fixtures` תקציבי זרם חשיפה ורמזים לתחבורה dentro de
  sus artefactos JSON para que los לוחות מחוונים סימן לאימוץ של תכונה.
- אביזרי אבזור `fixtures/sorafs_manifest/provider_admission/` כולל:
  - פרסומות קנוניקוס מרובות מקורות,
  - una variante alternativa sin rango para pruebas de downgrade, y
  - `multi_fetch_plan.json` para que las suites de SDK reproduzcan un plan de fetch
    דטרמיניסטה מרובת עמיתים.

## אינטגרציה עם orquestador y Torii

- Torii `/v2/sorafs/providers` devuelve metadata de capacidad de rango parseada junto con
  `stream_budget` y `transport_hints`. אין פרסומות להורדת דירוג cuando los
  מוכיח את המטא נתונים של נואבה, ואת נקודות הקצה של ה-rango del gateway aplican las
  מגבלות שונות למנהלי לקוחות.
- El orquestador multi-origen (`sorafs_car::multi_fetch`) ahora hace cumplir límites de
  rango, alineación de capacidades ו-stream budgets al asignar trabajo. Las pruebas unitarias
  קוברן escenarios de chunk demasiado grande, búsqueda dispersa y throttling.
- `sorafs_car::multi_fetch` emit señales de downgrade (fallos de alineación,
  שידולים מצטמצמים) para que los operadores rastreen por qué se omitieron
  proveedores específicos durante la planificación.

## Referencia de telemetria

La instrumentación de fetch de rango de Torii alimenta el לוח המחוונים de Grafana
**SoraFS תצפית אחזור** (`dashboards/grafana/sorafs_fetch_observability.json`) y
las reglas de alerta asociadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| מטריקה | טיפו | כללי התנהגות | תיאור |
|--------|------|--------|----------------|
| `torii_sorafs_provider_range_capability_total` | מד | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | תכונות ידועות של מאפיינים חכמים. |
| `torii_sorafs_range_fetch_throttle_events_total` | מונה | `reason` (`quota`, `concurrency`, `byte_rate`) | כוונות הבאות של רנגו עם מצערת אגרופים לפוליטיקה. |
| `torii_sorafs_range_fetch_concurrency_current` | מד | — | זרמים activos protegidos que consumen el presupuesto compartido de concurrencia. |

דוגמאות של PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Usa el contador de throttling para confirmar la aplicación de cuotas antes de habilitar
los valores por defecto del orquestador multi-origen y alerta cuando la concurrencia se
acerque a los máximos del presupuesto de streams de tu flota.