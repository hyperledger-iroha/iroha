---
lang: he
direction: rtl
source: docs/portal/i18n/es/docusaurus-plugin-content-docs/current/sorafs/node-client-protocol.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2ec212eee3a3f1fd2b152cc4455024e382efe3e9a920056bbdcbf0e76ee1d0bf
source_last_modified: "2026-01-22T15:38:30+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Protocolo de nodo ↔ cliente de SoraFS

Esta guía resume la definición canónica del protocolo en
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Usa la especificación upstream para los layouts Norito a nivel de bytes y los
changelogs; la copia del portal mantiene los puntos operativos cerca del resto
de los runbooks de SoraFS.

## Anuncios de proveedor y validación

Los proveedores de SoraFS difunden payloads `ProviderAdvertV1` (ver
`crates/sorafs_manifest::provider_advert`) firmados por el operador gobernado.
Los anuncios fijan los metadatos de descubrimiento y los guardarraíles que el
orquestador multifuente impone en tiempo de ejecución.

- **Vigencia** — `issued_at < expires_at ≤ issued_at + 86,400 s`. Los proveedores
  deben refrescar cada 12 horas.
- **TLVs de capacidades** — la lista TLV anuncia funciones de transporte (Torii,
  QUIC+Noise, relés SoraNet, extensiones de proveedor). Los códigos desconocidos
  pueden omitirse cuando `allow_unknown_capabilities = true`, siguiendo la guía
  GREASE.
- **Pistas de QoS** — nivel de `availability` (Hot/Warm/Cold), latencia máxima de
  recuperación, límite de concurrencia y presupuesto de stream opcional. La QoS
  debe alinearse con la telemetría observada y se audita en admisión.
- **Endpoints y topics de rendezvous** — URLs de servicio concretas con
  metadatos TLS/ALPN más los topics de descubrimiento a los que los clientes
  deben suscribirse al construir sets de guards.
- **Política de diversidad de rutas** — `min_guard_weight`, topes de fan-out de
  AS/pool y `provider_failure_threshold` hacen posibles los fetches deterministas
  multi-peer.
- **Identificadores de perfil** — los proveedores deben exponer el handle
  canónico (p. ej., `sorafs.sf1@1.0.0`); `profile_aliases` opcionales ayudan a
  migrar clientes antiguos.

Las reglas de validación rechazan stake cero, listas vacías de capabilities,
endpoints o topics, vigencias desordenadas o objetivos de QoS faltantes. Los
sobres de admisión comparan los cuerpos del anuncio y la propuesta
(`compare_core_fields`) antes de difundir actualizaciones.

### Extensiones de fetch por rangos

Los proveedores con rango incluyen la siguiente metadata:

| Campo | Propósito |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` y flags de alineación/prueba. |
| `StreamBudgetV1` | Envelope opcional de concurrencia/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). Requiere una capacidad de rango. |
| `TransportHintV1` | Preferencias de transporte ordenadas (p. ej., `torii_http_range`, `quic_stream`, `soranet_relay`). Las prioridades van de `0–15` y se rechazan duplicados. |

Soporte de tooling:

- Los pipelines de provider advert deben validar capacidad de rango, stream
  budget y transport hints antes de emitir payloads deterministas para auditorías.
- `cargo xtask sorafs-admission-fixtures` empaqueta anuncios multifuente
  canónicos junto con fixtures de downgrade en
  `fixtures/sorafs_manifest/provider_admission/`.
- Los anuncios con rango que omiten `stream_budget` o `transport_hints` son
  rechazados por los loaders CLI/SDK antes de programar, manteniendo el arnés
  multifuente alineado con las expectativas de admisión de Torii.

## Endpoints de rango del gateway

Los gateways aceptan solicitudes HTTP deterministas que reflejan la metadata de
los anuncios.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Requisito | Detalles |
|-----------|----------|
| **Headers** | `Range` (ventana única alineada a offsets de chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional y `X-SoraFS-Stream-Token` base64 obligatorio. |
| **Respuestas** | `206` con `Content-Type: application/vnd.ipld.car`, `Content-Range` que describe la ventana servida, metadata `X-Sora-Chunk-Range` y headers de chunker/token ecoados. |
| **Modos de fallo** | `416` para rangos desalineados, `401` para tokens faltantes o inválidos, `429` cuando se exceden presupuestos de stream/bytes. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch de un solo chunk con los mismos headers más el digest determinista del
chunk. Útil para reintentos o descargas forenses cuando no se necesitan slices
CAR.

## Flujo de trabajo del orquestador multifuente

Cuando se habilita el fetch multifuente SF-6 (CLI Rust vía `sorafs_fetch`,
SDKs vía `sorafs_orchestrator`):

1. **Recopilar entradas** — decodificar el plan de chunks del manifest, traer
   los anuncios más recientes y, opcionalmente, pasar un snapshot de telemetría
   (`--telemetry-json` o `TelemetrySnapshot`).
2. **Construir un scoreboard** — `Orchestrator::build_scoreboard` evalúa la
   elegibilidad y registra razones de rechazo; `sorafs_fetch --scoreboard-out`
   persiste el JSON.
3. **Programar chunks** — `fetch_with_scoreboard` (o `--plan`) impone
   restricciones de rango, presupuestos de stream, topes de reintentos/peers
   (`--retry-budget`, `--max-peers`) y emite un stream token con alcance del
   manifest por cada solicitud.
4. **Verificar recibos** — las salidas incluyen `chunk_receipts` y
   `provider_reports`; los resúmenes de CLI persisten `provider_reports`,
   `chunk_receipts` e `ineligible_providers` para bundles de evidencia.

Errores comunes que llegan a operadores/SDKs:

| Error | Descripción |
|-------|-------------|
| `no providers were supplied` | No hay entradas elegibles tras el filtrado. |
| `no compatible providers available for chunk {index}` | Desajuste de rango o presupuesto para un chunk específico. |
| `retry budget exhausted after {attempts}` | Incrementa `--retry-budget` o expulsa peers fallidos. |
| `no healthy providers remaining` | Todos los proveedores quedaron deshabilitados tras fallos repetidos. |
| `streaming observer failed` | El escritor CAR downstream abortó. |
| `orchestrator invariant violated` | Captura manifest, scoreboard, snapshot de telemetría y JSON del CLI para triage. |

## Telemetría y evidencia

- Métricas emitidas por el orquestador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (etiquetadas por manifest/región/proveedor). Configura `telemetry_region` en
  config o vía flags de CLI para que los dashboards separen por flota.
- Los resúmenes de fetch en CLI/SDK incluyen scoreboard JSON persistido, recibos
  de chunks e informes de proveedores que deben viajar en bundles de rollout
  para las puertas SF-6/SF-7.
- Los handlers del gateway exponen `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que los dashboards SRE correlacionen decisiones del orquestador con el
  comportamiento del servidor.

## Ayudas de CLI y REST

- `iroha app sorafs pin list|show`, `alias list` y `replication list` envuelven los
  endpoints REST del pin-registry e imprimen Norito JSON crudo con bloques de
  attestation para evidencia de auditoría.
- `iroha app sorafs storage pin` y `torii /v1/sorafs/pin/register` aceptan manifests
  Norito o JSON más proofs de alias opcionales y successors; proofs malformados
  elevan `400`, proofs obsoletos exponen `503` con `Warning: 110`, y proofs
  expirados devuelven `412`.
- Los endpoints REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) incluyen estructuras de attestation para que los
  clientes verifiquen datos contra los headers del último bloque antes de actuar.

## Referencias

- Especificación canónica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ayudas de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Crate del orquestador: `crates/sorafs_orchestrator`
- Paquete de dashboards: `dashboards/grafana/sorafs_fetch_observability.json`
