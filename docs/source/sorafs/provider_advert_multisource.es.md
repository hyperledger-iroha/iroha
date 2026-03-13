---
lang: es
direction: ltr
source: docs/source/sorafs/provider_advert_multisource.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e421e2df8e47982166d523697f9b237bd8f28372500115c38b6a8bef664805e5
source_last_modified: "2025-11-22T07:05:16.336563+00:00"
translation_last_reviewed: "2026-01-30"
---

# Extensiones de Provider Advert para scheduling multi-source (SF-2d)

Esta nota registra el trabajo de schema, validaciones y cambios de tooling
necesarios para el scheduling de chunks multi-source en SoraFS. Los payloads
Norito ahora exponen metadata de range fetch, budgets de throughput y
preferencias de transporte para que los clientes puedan planificar fetches de
multiples fuentes de forma determinista.

## Schema Norito

### `CapabilityType::ChunkRangeFetch` (`ProviderCapabilityRangeV1`)
- `max_chunk_span`: el bloque contiguo mas grande (bytes) que un provider
  entregara en un request ranged. Debe ser >= 1.
- `min_granularity`: la resolucion de seek mas pequena dentro de un chunk.
  Debe ser >= 1 y <= `max_chunk_span`.
- `supports_sparse_offsets`: indica que offsets no contiguos son legales dentro
  de un request.
- `requires_alignment`: cuando es `true`, los offsets deben alinearse a
  `min_granularity`.
- `supports_merkle_proof`: el provider puede adjuntar proof material junto a
  responses ranged.
- Helpers de encoding: `ProviderCapabilityRangeV1::to_bytes` /
  `from_bytes` aseguran payloads Norito canonicos.

### `StreamBudgetV1`
- Campos: `max_in_flight`, `max_bytes_per_sec`, `burst_bytes` opcional.
- Reglas de validacion (enforced por `StreamBudgetV1::validate` y ejercitadas en
  tests `crates/sorafs_manifest/src/provider_advert.rs`):
  - `max_in_flight` >= 1, `max_bytes_per_sec` > 0.
  - `burst_bytes`, cuando esta presente, debe ser > 0 y <= `max_bytes_per_sec`.

### `TransportHintV1`
- Campos: `protocol: TransportProtocol`, `priority: u8`.
- Ventana de prioridad 0-15 (enforced via `TransportHintV1::validate` y parsing
  de CLI). Colisiones de prioridad se rechazan por provider.
- Protocolos soportados: `torii_http_range`, `quic_stream`, `soranet_relay`,
  `vendor_reserved`.

### Adiciones a `ProviderAdvertBodyV1`
- `stream_budget: Option<StreamBudgetV1>` opcional.
- `transport_hints: Option<Vec<TransportHintV1>>` opcional.
- Ambos campos se propagan a `ProviderAdmissionProposalV1`, el sobre de
  governance, fixtures de CLI y outputs JSON de telemetria.

## Validacion y binding de governance

`ProviderAdvertBodyV1::validate` y `ProviderAdmissionProposalV1::validate`
ahora hacen cumplir:
- Los payloads de range capability deben decodificar y satisfacer limites de
  span/granularity.
- Stream budgets y transport hints deben pasar sus validadores respectivos.
- Stream budgets / transport hints requieren un TLV `CapabilityType::ChunkRangeFetch`
  acompañante, y las listas de transport hints deben ser no vacias.
- Entradas duplicadas de `TransportProtocol` se rechazan.
- Los sobres de admision comparan propuesta/adverts para stream budgets y
  transport hints (`compare_core_fields`), evitando gossip payloads desalineados.

Highlights de cobertura de tests:
- `crates/sorafs_manifest/src/provider_advert.rs` valida edge cases para stream
  budgets, prioridades de transport hints, protocolos duplicados y payloads de
  rango malformados.
- `crates/sorafs_manifest/src/provider_admission.rs` asegura que governance
  falla cuando budgets/hints divergen y rechaza prioridades invalidas.

## Tooling y fixtures

- Los payloads de provider advert deben incluir metadata `range_capability`,
  `stream_budget` y `transport_hints`. Validar via respuestas `/v2/sorafs/providers`
  y fixtures de admision; reportes JSON deben exponer secciones `range`,
  `stream_budget` y `transport_hints` estructuradas para telemetria.
- `cargo xtask sorafs-admission-fixtures` emite resúmenes JSON que ahora exponen
  stream budgets y transport hints para dashboards de telemetria.
- `crates/sorafs_car/src/bin/provider_admission_fixtures.rs` y
  `crates/sorafs_car/src/bin/sorafs_fetch.rs` fixtures/tests se actualizaron para
  usar encodings canonicos de range capability y ejercitar rutas de exito/downgrade
  con la nueva metadata.

## Trabajo de integracion pendiente

- Las respuestas Torii `/v2/sorafs/providers` ahora incluyen metadata de range
  capability parseada mas `stream_budget` y `transport_hints`, y emiten warnings
  de downgrade cuando los providers omiten los campos nuevos. Los endpoints de
  rango del gateway hacen cumplir los mismos constraints, con trabajo restante
  enfocado en telemetria del scheduler e integracion de tokens.
- El orquestador multi-source (`sorafs_car::multi_fetch`) ahora aplica la nueva
  metadata al asignar trabajos: limites de range-capability gatean elegibilidad
  de providers, violaciones de alignment suben como warnings explicitos y los
  stream budgets limitan la concurrencia, con unit tests cubriendo chunk-too-large,
  desajustes de length/offset, ausencia de capability y throttling de
  single-stream.【crates/sorafs_car/src/multi_fetch.rs:190-262】【crates/sorafs_car/src/multi_fetch.rs:456-520】【crates/sorafs_car/src/multi_fetch.rs:1341-1501】
- Fixtures en `fixtures/sorafs_manifest/provider_admission/` incluyen el plan
  moderno multi-fetch para integration tests de SDK (`multi_fetch_plan.json`).【fixtures/sorafs_manifest/provider_admission/README.md:1】
- Torii ahora exporta metricas de observabilidad de range fetch —
  `torii_sorafs_provider_range_capability_total` (gauge con label de feature),
  `torii_sorafs_range_fetch_throttle_events_total` (razones de throttle) y
  `torii_sorafs_range_fetch_concurrency_current` (streams activos guardados por
  tokens) — y el dashboard Grafana **SoraFS Fetch Observability** expone mix de
  capabilities, tasas de throttle y concurrencia en vivo para uso on-call.

## Referencia de telemetria

| Metrica | Tipo | Labels | Descripcion |
|---------|------|--------|-------------|
| `torii_sorafs_provider_range_capability_total` | Gauge | `feature` (`providers`, `supports_sparse_offsets`, `requires_alignment`, `supports_merkle_proof`, `stream_budget`, `transport_hints`) | Cuenta provider adverts que exponen la range capability y features asociadas. |
| `torii_sorafs_range_fetch_throttle_events_total` | Counter | `reason` (`quota`, `concurrency`, `byte_rate`) | Numero de intentos de range fetch throttled agrupados por la politica que rechazo el request. |
| `torii_sorafs_range_fetch_concurrency_current` | Gauge | — | Streams activos de range fetch guardados por limites de concurrencia de tokens. |

Paneles Grafana (dashboard UID `sorafs-fetch`) grafican tasas de throttle en
ventanas de 5 minutos y exponen gauges de concurrencia con thresholds de alerta.
Consultas de ejemplo:

```promql
sum(rate(torii_sorafs_range_fetch_throttle_events_total[5m])) by (reason)
max(torii_sorafs_range_fetch_concurrency_current)
torii_sorafs_provider_range_capability_total
```
