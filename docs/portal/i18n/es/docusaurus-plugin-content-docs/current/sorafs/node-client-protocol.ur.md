---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# SoraFS نوڈ ↔ کلائنٹ پروٹوکول

یہ گائیڈ پروٹوکول کی canónico تعریف کو
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
میں resumir کرتی ہے۔ diseños Norito de nivel de bytes y registros de cambios en sentido ascendente
especificación استعمال کریں؛ copia del portal SoraFS runbooks کے ساتھ aspectos destacados operativos کو
قریب رکھتی ہے۔

## Anuncios de proveedores y validación

Proveedores SoraFS Cargas útiles `ProviderAdvertV1` (دیکھیں
`crates/sorafs_manifest::provider_advert`) chisme کرتے ہیں جو operador gobernado
نے signo کیے ہوتے ہیں۔ metadatos de descubrimiento de anuncios اور guardrails کو pin کرتے ہیں
Tiempo de ejecución del orquestador de múltiples fuentes میں aplicar کرتا ہے۔- **Vida útil** — `issued_at < expires_at ≤ issued_at + 86,400 s`. proveedores کو
  ہر 12 گھنٹے میں actualizar کرنا چاہیے۔
- **TLV de capacidad**: las funciones de transporte de listas de TLV anuncian کرتی ہے (Torii,
  QUIC+Noise, relés SoraNet, extensiones de proveedores). códigos desconocidos کو
  `allow_unknown_capabilities = true` پر skip کیا جا سکتا ہے، GREASE guiado کے مطابق۔
- **Sugerencias de QoS**: nivel `availability` (caliente/tibio/frío), latencia máxima de recuperación,
  límite de concurrencia o presupuesto de flujo opcional. QoS کو telemetría observada کے
  مطابق ہونا چاہیے اور admisión میں auditoría کیا جاتا ہے۔
- **Puntos finales y temas de encuentro**: metadatos TLS/ALPN concretos
  URL de servicio, temas de descubrimiento, clientes, conjuntos de guardias, suscripción
  کرنا چاہیے۔
- **Política de diversidad de rutas** — `min_guard_weight`, límites de distribución de AS/grupos,
  `provider_failure_threshold` recuperaciones deterministas de múltiples pares کو ممکن بناتے ہیں۔
- **Identificadores de perfil**: los proveedores کو identificadores canónicos exponen کرنا ہوتا ہے
  (مثلاً `sorafs.sf1@1.0.0`); opcional `profile_aliases` پرانے clientes کی
  migración میں مدد دیتے ہیں۔

Reglas de validación de riesgo cero, capacidades/puntos finales/listas de temas vacíos, mal ordenados
vidas, یا faltan objetivos de QoS کو rechazar کرتی ہیں۔ Anuncio de sobres de admisión.
اور cuerpos de propuesta (`compare_core_fields`) کو comparar کرتے ہیں پھر actualizaciones chismes
کرتے ہیں۔

### Extensiones de recuperación de rango

Los proveedores con capacidad de alcance incluyen metadatos:| Campo | Propósito |
|-------|---------|
| `CapabilityType::ChunkRangeFetch` | `max_chunk_span`, `min_granularity` Las banderas de alineación/prueba declaran کرتا ہے۔ |
| `StreamBudgetV1` | sobre de simultaneidad/rendimiento opcional (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). capacidad de alcance requerida ہے۔ |
| `TransportHintV1` | preferencias de transporte ordenadas (مثلاً `torii_http_range`, `quic_stream`, `soranet_relay`). prioridades `0–15` ہیں اور duplicados rechazar ہوتے ہیں۔ |

Soporte de herramientas:

- Canalizaciones de anuncios del proveedor, capacidad de alcance, presupuesto de flujo, sugerencias de transporte
  validar کرنے چاہییں پھر auditorías کے لیے cargas útiles deterministas emitir کریں۔
- `cargo xtask sorafs-admission-fixtures` anuncios canónicos de múltiples fuentes
  accesorios de degradación کے ساتھ `fixtures/sorafs_manifest/provider_admission/` میں paquete کرتا ہے۔
- Los anuncios con capacidad de rango, `stream_budget` y `transport_hints` omiten CLI/SDK
  cargadores انہیں programación سے پہلے rechazar کرتے ہیں تاکہ arnés de múltiples fuentes
  Torii expectativas de admisión کے ساتھ alineadas رہے۔

## Puntos finales del rango de puerta de enlace

Las solicitudes HTTP deterministas de las puertas de enlace aceptan metadatos publicitarios y espejos
کرتی ہیں۔

### `GET /v2/sorafs/storage/car/{manifest_id}`| Requisito | Detalles |
|-------------|---------|
| **Encabezados** | `Range` (ventana única alineada con compensaciones de fragmentos), `dag-scope: block`, `X-SoraFS-Chunker`, opcional `X-SoraFS-Nonce`, base64 `X-SoraFS-Stream-Token`. |
| **Respuestas** | `206` con `Content-Type: application/vnd.ipld.car`, `Content-Range` es una ventana servida y contiene metadatos `X-Sora-Chunk-Range` y encabezados de fragmentos/tokens y eco |
| **Modos de fallo** | rangos desalineados en `416`, tokens faltantes/no válidos en `401`, y los presupuestos de flujo/byte superan en `429` |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Recuperación de fragmento único y encabezados کے ساتھ más resumen de fragmento determinista ۔ reintentos
یا descargas forenses کے لیے مفید جب CAR slices غیر ضروری ہوں۔

## Flujo de trabajo del orquestador de múltiples fuentes

جب SF-6 recuperación de múltiples fuentes habilitada ہو (Rust CLI a través de `sorafs_fetch`, SDK a través de
`sorafs_orchestrator`):1. **Recopilar entradas**: decodificación del plan de fragmentos de manifiesto کریں، los últimos anuncios extraen کریں،
   اور instantánea de telemetría opcional (`--telemetry-json` یا `TelemetrySnapshot`) پاس کریں۔
2. **Construya un marcador** — Evaluación de elegibilidad `Orchestrator::build_scoreboard`
   کرتا ہے اور registro de motivos de rechazo کرتا ہے؛ `sorafs_fetch --scoreboard-out`
   JSON persiste کرتا ہے۔
3. **Programar fragmentos**: restricciones de rango `fetch_with_scoreboard` (یا `--plan`),
   presupuestos de flujo, reintentos/límites de pares (`--retry-budget`, `--max-peers`) aplican کرتا ہے
   اور ہر solicitud کے لیے emisión de token de flujo con ámbito de manifiesto کرتا ہے۔
4. **Verificar recibos** — salidas میں `chunk_receipts` اور `provider_reports` شامل
   ہوتے ہیں؛ Resúmenes de CLI `provider_reports`, `chunk_receipts`, otros
   `ineligible_providers` کو paquetes de evidencia کے لیے persisten کرتے ہیں۔

Errores de operadores/SDK:

| Error | Descripción |
|-------|-------------|
| `no providers were supplied` | filtrado کے بعد کوئی entrada elegible نہیں۔ |
| `no compatible providers available for chunk {index}` | مخصوص fragmento کے لیے rango یا desajuste presupuestario۔ |
| `retry budget exhausted after {attempts}` | `--retry-budget` بڑھائیں یا pares fallidos کو desalojar کریں۔ |
| `no healthy providers remaining` | fallas repetidas کے بعد تمام proveedores desactivan ہو گئے۔ |
| `streaming observer failed` | Aborto del escritor CAR en sentido descendente ہو گیا۔ |
| `orchestrator invariant violated` | triaje کے لیے manifiesto, marcador, instantánea de telemetría, اور CLI JSON captura کریں۔ |

## Telemetría y evidencia- Métricas del orquestador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (manifiesto/región/proveedor کے etiquetas کے ساتھ). paneles de control کو flota کے لحاظ سے
  partición کرنے کے لیے config یا CLI flags میں `telemetry_region` سیٹ کریں۔
- Resúmenes de búsqueda de CLI/SDK, marcador persistente JSON, recibos de fragmentos y
  informes de proveedores شامل ہوتے ہیں جو SF-6/SF-7 gates کے paquetes de implementación میں شامل ہونے چاہییں۔
- Controladores de puerta de enlace `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  exponer las decisiones del orquestador de paneles de control de SRE y el comportamiento del servidor
  correlacionar کر سکیں۔

## Ayudantes de CLI y REST

- `iroha app sorafs pin list|show`, `alias list`, o `replication list` registro pin
  Los puntos finales REST envuelven pruebas de auditoría y bloques de atestación.
  raw Norito Impresión JSON کرتے ہیں۔
- `iroha app sorafs storage pin` o `torii /v2/sorafs/pin/register` Norito y JSON
  manifiesta کے ساتھ pruebas de alias opcionales اور sucesores aceptan کرتے ہیں؛ mal formado
  pruebas de `400`, pruebas obsoletas de `503` o `Warning: 110`, pruebas caducadas
  Por `412`۔
- Puntos finales REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`, `/v2/sorafs/replication`)
  estructuras de atestación شامل کرتے ہیں تاکہ clientes últimos encabezados de bloque کے
  خلاف verificar datos کر سکیں۔

## Referencias- Especificaciones canónicas:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Norito tipos: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ayudantes de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caja del orquestador: `crates/sorafs_orchestrator`
- Paquete de tablero: `dashboards/grafana/sorafs_fetch_observability.json`