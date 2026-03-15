---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Anuncios de proveedores multiorigen y planificación

Esta página destila la especificación canónica en
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Usa ese documento para los esquemas Norito textualmente y los changelogs; la copia del portal
mantiene la guía de operadores, las notas de SDK y las referencias de telemetría cerca del resto
de los runbooks de SoraFS.

## Adiciones al esquema Norito

### Capacidad de rango (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – mayor tramo contiguo (bytes) por solicitud, `>= 1`.
- `min_granularity` – resolución de búsqueda, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` – permite compensaciones no contiguas en una solicitud.
- `requires_alignment` – cuando es verdadero, los offsets deben alinearse con `min_granularity`.
- `supports_merkle_proof` – indica soporte de testigos PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplicar codificación canónica
para que los payloads de gossip sigan siendo deterministas.

### `StreamBudgetV1`
- Campos: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcionales.
- Reglas de validación (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, cuando esté presente, debe ser `> 0` y `<= max_bytes_per_sec`.### `TransportHintV1`
- Campos: `protocol: TransportProtocol`, `priority: u8` (ventana 0-15 aplicada por
  `TransportHintV1::validate`).
- Protocolos conocidos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Se rechazan entradas duplicadas de protocolo por proveedor.

### Adiciones a `ProviderAdvertBodyV1`
- `stream_budget` opcional: `Option<StreamBudgetV1>`.
- `transport_hints` opcional: `Option<Vec<TransportHintV1>>`.
- Ambos campos ahora fluyen por `ProviderAdmissionProposalV1`, los sobres de gobernanza,
  los dispositivos de CLI y el JSON telemétrico.

## Validación y vinculación con gobernanza

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
rechazan metadatos malformados:

- Las capacidades de rango deben decodificarse y cumplir los límites de span/granularidad.
- Los presupuestos de flujo / sugerencias de transporte requieren un TLV `CapabilityType::ChunkRangeFetch`
  coincidente y una lista de pistas no vacía.
- Protocolos de transporte duplicados y prioridades inválidas generan errores de validación
  antes de que los anuncios se chismeen.
- Los sobres de admisión comparan propuesta/anuncios para metadatos de rango vía
  `compare_core_fields` para que los payloads de gossip desalineados se rechacen temprano.

La cobertura de regresión vive en
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Herramientas y accesorios- Los payloads de anuncios de proveedor deben incluir metadatos `range_capability`,
  `stream_budget` y `transport_hints`. Valida vía respuestas de `/v2/sorafs/providers` y
  accesorios de admisión; los resúmenes JSON deben incluir la capacidad parseada,
  el stream Budget y los arrays de tips para ingestión de telemetría.
- `cargo xtask sorafs-admission-fixtures` exponen presupuestos de flujo y sugerencias de transporte dentro de
  sus artefactos JSON para que los paneles signifiquen la adopción de la función.
- Los accesorios bajo `fixtures/sorafs_manifest/provider_admission/` ahora incluyen:
  - anuncios canónicos de origen múltiple,
  - una variante alternativa sin rango para pruebas de degradación, y
  - `multi_fetch_plan.json` para que las suites de SDK reproduzcan un plan de recuperación
    determinista de pares múltiples.

## Integración con orquestador y Torii- Torii `/v2/sorafs/providers` devuelve metadatos de capacidad de rango parseada junto con
  `stream_budget` y `transport_hints`. Se disparan advertencias de downgrade cuando los
  los proveedores omiten la nueva metadatos, y los endpoints de rango del gateway aplican las
  Mismas restricciones para clientes directos.
- El orquestador multi-origen (`sorafs_car::multi_fetch`) ahora hace cumplir límites de
  rango, alineación de capacidades y flujo de presupuestos al asignar trabajo. Las pruebas unitarias
  cubren escenarios de trozos demasiado grandes, búsqueda dispersa y estrangulamiento.
- `sorafs_car::multi_fetch` emite señales de degradación (fallos de alineación,
  solicitudes throttled) para que los operadores rastreen por qué se omitieron
  proveedores específicos durante la planificación.

## Referencia de telemetría

La instrumentación de búsqueda de rango de Torii alimenta el tablero de Grafana
**SoraFS Obtención de observabilidad** (`dashboards/grafana/sorafs_fetch_observability.json`) y
las reglas de alerta asociadas (`dashboards/alerts/sorafs_fetch_rules.yml`).| Métrica | Tipo | Etiquetas | Descripción |
|---------|------|-----------|-------------|
| `torii_sorafs_provider_range_capability_total` | Calibre | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Proveedores que anuncian características de capacidad de rango. |
| `torii_sorafs_range_fetch_throttle_events_total` | Mostrador | `reason` (`quota`, `concurrency`, `byte_rate`) | Intentos de búsqueda de rango con estrangulamiento agrupados por política. |
| `torii_sorafs_range_fetch_concurrency_current` | Calibre | — | Corrientes activos protegidos que consumen el presupuesto compartido de concurrencia. |

Ejemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Utilice el contador de aceleración para confirmar la aplicación de cuotas antes de habilitar
los valores por defecto del orquestador multi-origen y alerta cuando la concurrencia se
acércate a los máximos del presupuesto de streams de tu flota.