---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocolo de nodo ↔ cliente de SoraFS

Esta guía resume la definición canónica del protocolo en
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Usa la especificación upstream para los diseños Norito a nivel de bytes y los
registros de cambios; la copia del portal mantiene los puntos operativos cerca del resto
de los runbooks de SoraFS.

## Anuncios de proveedor y validación

Los proveedores de SoraFS difunden payloads `ProviderAdvertV1` (ver
`crates/sorafs_manifest::provider_advert`) firmados por el operador gobernado.
Los anuncios fijan los metadatos de descubrimiento y los guardarraíles que el
orquestador multifuente impone en tiempo de ejecución.- **Vigencia** — `issued_at < expires_at ≤ issued_at + 86,400 s`. Los proveedores
  deben refrescar cada 12 horas.
- **TLVs de capacidades** — la lista TLV anuncia funciones de transporte (Torii,
  QUIC+Noise, relés SoraNet, extensiones de proveedor). Los códigos desconocidos
  pueden omitirse cuando `allow_unknown_capabilities = true`, siguiendo la guía
  GRASA.
- **Pistas de QoS** — nivel de `availability` (Caliente/Tibio/Frío), latencia máxima de
  recuperación, límite de concurrencia y presupuesto de flujo opcional. La QoS
  debe alinearse con la telemetría observada y se audita en admisión.
- **Endpoints y issues de rendezvous** — URLs de servicio concretas con
  metadatos TLS/ALPN más los temas de descubrimiento a los que los clientes
  deben suscribirse al construir conjuntos de guardias.
- **Política de diversidad de rutas** — `min_guard_weight`, topes de fan-out de
  AS/pool y `provider_failure_threshold` hacen posibles los fetches deterministas
  multipar.
- **Identificadores de perfil** — los proveedores deben exponer el mango
  canónico (p. ej., `sorafs.sf1@1.0.0`); `profile_aliases` opcionales ayudan a
  migrar clientes antiguos.

Las reglas de validación rechazan estaca cero, listas vacías de capacidades,
endpoints o topic, vigencias desordenadas o objetivos de QoS faltantes. Los
sobres de admisión comparan los cuerpos del anuncio y la propuesta
(`compare_core_fields`) antes de difundir actualizaciones.

### Extensiones de búsqueda por rangosLos proveedores con rango incluyen los siguientes metadatos:

| Campo | Propósito |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` y flags de alineación/prueba. |
| `StreamBudgetV1` | Sobre opcional de concurrencia/rendimiento (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). Requiere una capacidad de rango. |
| `TransportHintV1` | Preferencias de transporte ordenadas (p. ej., `torii_http_range`, `quic_stream`, `soranet_relay`). Las prioridades van de `0–15` y se rechazan duplicados. |

Soporte de herramientas:

- Los ductos de proveedor advert deben validar capacidad de rango, flujo
  Budget y Transport Hints antes de emitir payloads deterministas para auditorías.
- `cargo xtask sorafs-admission-fixtures` empaqueta anuncios multifuente
  canónicos junto con accesorios de downgrade en
  `fixtures/sorafs_manifest/provider_admission/`.
- Los anuncios con rango que omiten `stream_budget` o `transport_hints` son
  rechazados por los cargadores CLI/SDK antes de programar, manteniendo el arnés
  multifuente alineado con las expectativas de admisión de Torii.

## Puntos finales de rango del gateway

Los gateways aceptan solicitudes HTTP deterministas que reflejan los metadatos de
los anuncios.

### `GET /v2/sorafs/storage/car/{manifest_id}`| Requisito | Detalles |
|-----------|----------|
| **Encabezados** | `Range` (ventana única alineada a offsets de chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional y `X-SoraFS-Stream-Token` base64 obligatorio. |
| **Respuestas** | `206` con `Content-Type: application/vnd.ipld.car`, `Content-Range` que describen la ventana servida, metadatos `X-Sora-Chunk-Range` y encabezados de fragmentador/token ecoados. |
| **Modos de fallo** | `416` para rangos desalineados, `401` para tokens faltantes o inválidos, `429` cuando se exceden presupuestos de stream/bytes. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Fetch de un solo chunk con los mismos headers más el digest determinista del
trozo. Útil para reintentos o descargas forenses cuando no se necesitan rebanadas
COCHE.

## Flujo de trabajo del orquestador multifuente

Cuando se habilita el fetch multifuente SF-6 (CLI Rust vía `sorafs_fetch`,
SDK vía `sorafs_orchestrator`):1. **Recopilar entradas** — decodificar el plan de trozos del manifiesto, traer
   los anuncios más recientes y, opcionalmente, pasar una instantánea de telemetría
   (`--telemetry-json` o `TelemetrySnapshot`).
2. **Construir un marcador** — `Orchestrator::build_scoreboard` evalúa la
   elegibilidad y registro razones de rechazo; `sorafs_fetch --scoreboard-out`
   persistir el JSON.
3. **Programar fragmentos** — `fetch_with_scoreboard` (o `--plan`) impone
   restricciones de rango, presupuestos de stream, topes de reintentos/peers
   (`--retry-budget`, `--max-peers`) y emite un token de transmisión con alcance del
   manifiesto por cada solicitud.
4. **Verificar recibos** — las salidas incluyen `chunk_receipts` y
   `provider_reports`; los resúmenes de CLI persisten `provider_reports`,
   `chunk_receipts` e `ineligible_providers` para paquetes de evidencia.

Errores comunes que llegan a operadores/SDK:

| Error | Descripción |
|-------|-------------|
| `no providers were supplied` | No hay entradas elegibles tras el filtrado. |
| `no compatible providers available for chunk {index}` | Desajuste de rango o presupuesto para una porción específica. |
| `retry budget exhausted after {attempts}` | Incrementa `--retry-budget` o expulsa pares fallidos. |
| `no healthy providers remaining` | Todos los proveedores quedaron deshabilitados tras fallos repetidos. |
| `streaming observer failed` | El escritor CAR downstream abortó. |
| `orchestrator invariant violated` | Captura de manifiesto, marcador, instantánea de telemetría y JSON del CLI para triage. |

## Telemetría y evidencia- Métricas emitidas por el orquestador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (etiquetadas por manifiesto/región/proveedor). Configurar `telemetry_region` es
  config o vía flags de CLI para que los tableros se separen por flota.
- Los resúmenes de fetch en CLI/SDK incluyen marcador JSON persistente, recibos
  de chunks e informes de proveedores que deben viajar en paquetes de rollout
  para las puertas SF-6/SF-7.
- Los handlers del gateway exponen `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que los tableros SRE correlacionen decisiones del orquestador con el
  comportamiento del servidor.

## Ayudas de CLI y REST

- `iroha app sorafs pin list|show`, `alias list` y `replication list` envuelven los
  endpoints REST del pin-registry e imprimen Norito JSON crudo con bloques de
  attestation para evidencia de auditoría.
- `iroha app sorafs storage pin` y `torii /v2/sorafs/pin/register` aceptan manifiestos
  Norito o JSON más pruebas de alias opcionales y sucesores; pruebas malformadas
  elevan `400`, pruebas obsoletos exponente `503` con `Warning: 110`, y pruebas
  caducados devuelven `412`.
- Los puntos finales REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) incluyen estructuras de atestación para que los
  clientes verifiquen datos contra los encabezados del último bloque antes de actuar.

## Referencias- Especificación canónica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ayudas de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caja del orquestador: `crates/sorafs_orchestrator`
- Paquete de paneles: `dashboards/grafana/sorafs_fetch_observability.json`