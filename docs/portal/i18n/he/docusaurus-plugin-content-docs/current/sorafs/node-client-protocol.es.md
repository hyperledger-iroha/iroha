---
lang: he
direction: rtl
source: docs/portal/docs/sorafs/node-client-protocol.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# פרוטוקול דה נודו ↔ cliente de SoraFS

Esta guía resume la definición canónica del protocolo en
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Usa la especificación upstream para los layouts Norito a nivel de bytes y los
יומני שינויים; la copia del portal mantiene los puntos operativos cerca del resto
de los runbooks de SoraFS.

## הכרזה על הוכחה ואימות

Los proveedores de SoraFS difunden payloads `ProviderAdvertV1` (ver
`crates/sorafs_manifest::provider_advert`) firmados por el operador gobernado.
Los anuncios fijan los metadatos de descubrimiento y los guardarraíles que el
orquestador multifuente impone en tiempo de ejecución.

- **ויג'נסיה** — `issued_at < expires_at ≤ issued_at + 86,400 s`. לוס מוכיחים
  deben refrescar cada 12 horas.
- **TLVs de capacidades** — רשימה TLV anuncia funciones de transporte (Torii,
  QUIC+Noise, מוסיף SoraNet, הרחבות מוכיחות). Los códigos desconocidos
  pueden omitirse cuando `allow_unknown_capabilities = true`, siguiendo la guía
  גריז.
- **Pistas de QoS** — nivel de `availability` (חם/חם/קר), latecia máxima de
  התאוששות, גבול קונקוררנסיה וקדם הזרם אופציונלי. La QoS
  debe alinearse con la telemetría observada y se audita en admisión.
- **נקודות קצה y topics de rendezvous** - URLs de servicio concretas con
  metadatos TLS/ALPN עוד על נושאים של פתיחת מסמכים
  deben suscribirse al construir sets de guards.
- **Política de diversidad de rutas** — `min_guard_weight`, topes de fan-out de
  AS/pool y `provider_failure_threshold` האפשרויות הבאות את ההחלטות
  ריבוי עמיתים.
- **Identificadores de perfil** - los proveedores deben exponer el handle
  canónico (עמ' ej., `sorafs.sf1@1.0.0`); `profile_aliases` אופציונליות עידן א
  migrar clientes antiguos.

Las reglas de validación rechazan stake cero, lists vacías de capabilities,
נקודות קצה או נושאים, נקודות הגנה או נקודות קצה של נקודות איכות. לוס
sobres de admisión comparan los cuerpos del anuncio y la propuesta
(`compare_core_fields`) antes de difundir actualizaciones.

### הרחבות לשליפה של רנגוס

Los proveedores con rango כולל מטא נתונים הבאים:

| קמפו | פרופוזיטו |
|-------|--------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` y flags de alineación/prueba. |
| `StreamBudgetV1` | Envelope optional de concurrencia/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` אופציונלי). Requiere una capacidad de rango. |
| `TransportHintV1` | Preferencias de transporte ordenadas (עמ' ej., `torii_http_range`, `quic_stream`, `soranet_relay`). Las prioridades van de `0–15` y se rechazan duplicados. |

Soporte de Tooling:- Los pipelines de provider advert deben validar capacidad de rango, stream
  תקציב y תחבורה רמזים ante de emitir מטענים נקבעים עבור auditorías.
- `cargo xtask sorafs-admission-fixtures` empaqueta anuncios multifuente
  canónicos junto con fixtures de downgrade en
  `fixtures/sorafs_manifest/provider_admission/`.
- Los anuncios con rango que omiten `stream_budget` o `transport_hints` בן
  rechazados por los loaders CLI/SDK antes de programar, manteniendo el arnés
  multifuente alineado con las expectativas de admisión de Torii.

## נקודות קצה של שער

Los gateways aceptan provocate HTTP deterministas que reflejan la metadata de
los anuncios.

### `GET /v1/sorafs/storage/car/{manifest_id}`

| Requisito | פרטים |
|----------------|--------|
| **כותרות** | `Range` (ventana única alineada a offsets de chunks), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` אופציונלי y `X-SoraFS-Stream-Token` base64 obligator. |
| **Respuestas** | `206` con `Content-Type: application/vnd.ipld.car`, `Content-Range` que describe la ventana servida, metadata `X-Sora-Chunk-Range` y headers de chunker/token ecoados. |
| **מודוס דה fallo** | `416` עבור Rangos desalineados, `401` עבור אסימונים faltantes o inválidos, `429` cuando se exeden preupuestos de stream/bytes. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

אחזר את ה-un solo chunk con los mismos headers más el digest determinista del
נתח. Útil para reintentos o descargas forenses cuando no se necesitan slices
מכונית.

## Flujo de trabajo del orquestador multifuente

Cuando se habilita el fetch multifuente SF-6 (CLI Rust vía `sorafs_fetch`,
ערכות SDK דרך `sorafs_orchestrator`):

1. **Recopilar entradas** - decodificar el plan de chunks del manifest, traer
   los anuncios más recientes y, optionalmente, pasar un snapshot de telemetria
   (`--telemetry-json` או `TelemetrySnapshot`).
2. **בניית לוח תוצאות** — `Orchestrator::build_scoreboard` evalúa la
   elegibilidad y registra razones de rechazo; `sorafs_fetch --scoreboard-out`
   להתמיד ב-JSON.
3. **נתחי תוכנית** — איפוס `fetch_with_scoreboard` (o `--plan`)
   restricciones de rango, presupuestos de stream, topes de reintentos/peers
   (`--retry-budget`, `--max-peers`) y emite un stream token con alcance del
   manifest por cada solicitud.
4. **אימות recibos** - las salidas incluyen `chunk_receipts` y
   `provider_reports`; los resúmenes de CLI persisten `provider_reports`,
   `chunk_receipts` e `ineligible_providers` para bundles de evidencia.

שגיאות comunes que lgan a operadores/SDKs:

| שגיאה | תיאור |
|-------|-------------|
| `no providers were supplied` | אין חציר אנטראדאס elegibles tras el filtrado. |
| `no compatible providers available for chunk {index}` | Desajuste de rango o presupuesto para un chunk específico. |
| `retry budget exhausted after {attempts}` | Incrementa `--retry-budget` או גירוש עמיתים fallidos. |
| `no healthy providers remaining` | Todos los proveedores quedaron deshabilitados tras fallos repetidos. |
| `streaming observer failed` | אל המלווה CAR במורד הזרם הפלה. |
| `orchestrator invariant violated` | מניפסט קלט, לוח תוצאות, תמונת מצב של טלמטריה ו-JSON del CLI לטריאג'. |

## Telemetria y Evidencia- Métricas emitidas por el orquestador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (נימוס למניפסט/אזור/מוכיח). Configura `telemetry_region` en
  config o vía flags de CLI עבור לוחות מחוונים נפרדים פור פלטה.
- Los resúmenes de fetch en CLI/SDK כולל לוח תוצאות JSON מתמיד, recibos
  de chunks e informes de proveedores que deben viajar en bundles de rollout
  para las puertas SF-6/SF-7.
- Los handlers del gateway exponen `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que los לוחות מחוונים SRE correlacionen decisiones del orquestador con el
  comportamiento del servidor.

## Ayudas de CLI y REST

- `iroha app sorafs pin list|show`, `alias list` y `replication list` envuelven los
  נקודות קצה REST של pin-registry e imprimen Norito JSON crudo con bloques de
  אישור לראיה דה אודיטוריה.
- `iroha app sorafs storage pin` y `torii /v1/sorafs/pin/register` מניפסטים אקפטניים
  Norito o JSON עוד הוכחות לכינוי אופציונליים וממשיכים; הוכחה למלפורמדוס
  elevan `400`, הוכחות מיושנות exponen `503` con `Warning: 110`, y הוכחות
  expirados devuelven `412`.
- Los נקודות קצה REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`,
  `/v1/sorafs/replication`) כולל אישורים
  לקוחות אימות נתונים נגד כותרות דל último bloque antes de actuar.

## אסמכתאות

- Especificación canónica:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- טיפוס Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ayudas de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- ארגז אורקסטדור: `crates/sorafs_orchestrator`
- חבילת לוחות מחוונים: `dashboards/grafana/sorafs_fetch_observability.json`