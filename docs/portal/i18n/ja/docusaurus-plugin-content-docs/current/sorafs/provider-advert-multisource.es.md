---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/provider-advert-multisource.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Anuncios de proveedores multi-origen y planificación

Esta página destila la especificación canónica en
[`docs/source/sorafs/provider_advert_multisource.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/provider_advert_multisource.md).
Usa ese documento para los esquemas Norito verbatim y los changelogs; la copia del portal
mantiene la guía de operadores, las notas de SDK y las referencias de telemetría cerca del resto
de los runbooks de SoraFS.

## Adiciones al esquema Norito

### Capacidad de rango (`CapabilityType::ChunkRangeFetch`)
- `max_chunk_span` – mayor tramo contiguo (bytes) por solicitud, `>= 1`.
- `min_granularity` – resolución de búsqueda, `1 <= valor <= max_chunk_span`.
- `supports_sparse_offsets` – permite offsets no contiguos en una solicitud.
- `requires_alignment` – cuando es true, los offsets deben alinearse con `min_granularity`.
- `supports_merkle_proof` – indica soporte de testigos PoR.

`ProviderCapabilityRangeV1::to_bytes` / `from_bytes` aplican codificación canónica
para que los payloads de gossip sigan siendo deterministas.

### `StreamBudgetV1`
- Campos: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcional.
- Reglas de validación (`StreamBudgetV1::validate`):
  - `max_in_flight >= 1`, `max_bytes_per_sec > 0`.
  - `burst_bytes`, cuando está presente, debe ser `> 0` y `<= max_bytes_per_sec`.

### `TransportHintV1`
- Campos: `protocol: TransportProtocol`, `priority: u8` (ventana 0-15 aplicada por
  `TransportHintV1::validate`).
- Protocolos conocidos: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.
- Se rechazan entradas duplicadas de protocolo por proveedor.

### Adiciones a `ProviderAdvertBodyV1`
- `stream_budget` opcional: `Option<StreamBudgetV1>`.
- `transport_hints` opcional: `Option<Vec<TransportHintV1>>`.
- Ambos campos ahora fluyen por `ProviderAdmissionProposalV1`, los envelopes de gobernanza,
  los fixtures de CLI y el JSON telemétrico.

## Validación y vinculación con gobernanza

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
rechazan metadatos malformados:

- Las capacidades de rango deben decodificarse y cumplir los límites de span/granularidad.
- Los stream budgets / transport hints requieren un TLV `CapabilityType::ChunkRangeFetch`
  coincidente y una lista de hints no vacía.
- Protocolos de transporte duplicados y prioridades inválidas generan errores de validación
  antes de que los adverts se gossipeen.
- Los envelopes de admisión comparan proposal/adverts para metadatos de rango vía
  `compare_core_fields` para que los payloads de gossip desalineados se rechacen temprano.

La cobertura de regresión vive en
`crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`.

## Tooling y fixtures

- Los payloads de anuncios de proveedor deben incluir metadata `range_capability`,
  `stream_budget` y `transport_hints`. Valida via respuestas de `/v1/sorafs/providers` y
  fixtures de admisión; los resúmenes JSON deben incluir la capacidad parseada,
  el stream budget y los arrays de hints para ingestión de telemetría.
- `cargo xtask sorafs-admission-fixtures` expone stream budgets y transport hints dentro de
  sus artefactos JSON para que los dashboards sigan la adopción de la feature.
- Los fixtures bajo `fixtures/sorafs_manifest/provider_admission/` ahora incluyen:
  - adverts multi-origen canónicos,
  - una variante alternativa sin rango para pruebas de downgrade, y
  - `multi_fetch_plan.json` para que las suites de SDK reproduzcan un plan de fetch
    multi-peer determinista.

## Integración con orquestador y Torii

- Torii `/v1/sorafs/providers` devuelve metadata de capacidad de rango parseada junto con
  `stream_budget` y `transport_hints`. Se disparan advertencias de downgrade cuando los
  proveedores omiten la nueva metadata, y los endpoints de rango del gateway aplican las
  mismas restricciones para clientes directos.
- El orquestador multi-origen (`sorafs_car::multi_fetch`) ahora hace cumplir límites de
  rango, alineación de capacidades y stream budgets al asignar trabajo. Las pruebas unitarias
  cubren escenarios de chunk demasiado grande, búsqueda dispersa y throttling.
- `sorafs_car::multi_fetch` emite señales de downgrade (fallos de alineación,
  solicitudes throttled) para que los operadores rastreen por qué se omitieron
  proveedores específicos durante la planificación.

## Referencia de telemetría

La instrumentación de fetch de rango de Torii alimenta el dashboard de Grafana
**SoraFS Fetch Observability** (`dashboards/grafana/sorafs_fetch_observability.json`) y
las reglas de alerta asociadas (`dashboards/alerts/sorafs_fetch_rules.yml`).

| Métrica | Tipo | Etiquetas | Descripción |
|---------|------|-----------|-------------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Proveedores que anuncian features de capacidad de rango. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | Intentos de fetch de rango con throttling agrupados por política. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | Streams activos protegidos que consumen el presupuesto compartido de concurrencia. |

Ejemplos de PromQL:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```

Usa el contador de throttling para confirmar la aplicación de cuotas antes de habilitar
los valores por defecto del orquestador multi-origen y alerta cuando la concurrencia se
acerque a los máximos del presupuesto de streams de tu flota.
