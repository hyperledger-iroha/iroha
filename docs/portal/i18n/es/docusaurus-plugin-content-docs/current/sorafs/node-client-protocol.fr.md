---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocolo nuevo ↔ cliente SoraFS

Esta guía resume la definición canónica del protocolo dans
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilice la especificación upstream para los diseños Norito en el nivel de octeto y
los registros de cambios; la copia del portail guarde los puntos operativos près du
Resto de runbooks SoraFS.

## Anuncios facilitador y validación

Los proveedores SoraFS difusores de cargas útiles `ProviderAdvertV1` (ver
`crates/sorafs_manifest::provider_advert`) firmado por el operador gubernamental. les
anuncios fijan las metadonnées de découverte et les garde-fous que l'orchestrateur
Aplicación de múltiples fuentes para la ejecución.- **Duración de validez** — `issued_at < expires_at ≤ issued_at + 86 400 s`. les
  Los proveedores deben hacerlo antes de las 12 horas.
- **TLV de capacidades** — la lista TLV anuncia las funciones de transporte
  (Torii, QUIC+Noise, relais SoraNet, proveedor de extensiones). Los códigos desconocidos
  Es posible que se ignore cuando `allow_unknown_capabilities = true`, y después
  les recomendaciones GRASA.
- **Índices QoS** — nivel de `availability` (Caliente/Tibio/Frío), latencia máxima de
  recuperación, límite de concurrencia y presupuesto de flujo opcional. La QoS lo hace
  s'aligner sur la télémétrie observaée et est auditée lors de l'admission.
- **Puntos finales y temas de encuentro**: URL de servicio concreto con avec
  métadonnées TLS/ALPN, además de los temas de descubrimiento auxquels les clientes
  doivent s'abonner lors de la construcción de conjuntos de guardia.
- **Politique de diversité de chemin** — `min_guard_weight`, caps de fan-out
  AS/pool e `provider_failure_threshold` muestran posibles recuperaciones
  déterministes multi-peer.
- **Identificantes de perfil** — los proveedores deben exponer el mango
  canónico (ej. `sorafs.sf1@1.0.0`); asistente opcional `profile_aliases`
  les anciens client à migrer.Las reglas de validación rechazadas en juego cero, listas de videos de capacidades,
puntos finales o temas, duraciones mal ordenadas o objetivos QoS manquants. les
sobres de admisión comparados con el cuerpo de publicidad y propuesta
(`compare_core_fields`) delante del difusor de las actualizaciones del día.

### Extensiones de búsqueda por playas

Los proveedores con rango de capacidad incluyen los siguientes metadonnées:

| Campeón | Objetivo |
|-------|----------|
| `CapabilityType::ChunkRangeFetch` | Declare `max_chunk_span`, `min_granularity` y las banderas de alineación/preuve. |
| `StreamBudgetV1` | Opción de concurrencia/rendimiento de sobre (opcional `max_in_flight`, `max_bytes_per_sec`, `burst`). Requiere un rango de capacidad. |
| `TransportHintV1` | Préférences de transport ordonnées (ej. `torii_http_range`, `quic_stream`, `soranet_relay`). Las prioridades de `0–15` y los doblones son rechazados. |

Herramientas de soporte:

- Les pipelines d'advert fournisseur doivent valider capacidad range, stream
  Consejos de presupuesto y transporte antes de conocer las cargas útiles determinadas para las
  auditorías.
- `cargo xtask sorafs-admission-fixtures` reagrupación de anuncios multifuente
  Canoniques con accesorios de downgrade dans
  `fixtures/sorafs_manifest/provider_admission/`.
- La gama de anuncios que omittent `stream_budget` o `transport_hints` sont
  Rejetés par les loaders CLI/SDK antes de la planificación, alineando el arnés
  fuente múltiple en los asistentes de admisión Torii.

## Rango de puntos finales de la puerta de enlaceLas puertas de enlace aceptan solicitudes HTTP determinadas que reflejan las
Metadonnées des Ads.

### `GET /v2/sorafs/storage/car/{manifest_id}`

| Exigencia | Detalles |
|----------|---------|
| **Encabezados** | `Range` (ventana única alineada en los desplazamientos de fragmentos), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional y `X-SoraFS-Stream-Token` base64 obligatoria. |
| **Respuestas** | `206` con `Content-Type: application/vnd.ipld.car`, `Content-Range` decrivant el servicio de ventana, métadonnées `X-Sora-Chunk-Range` y encabezados fragmentados/token renovados. |
| **Modos de limpieza** | `416` para placas mal alineadas, `401` para tokens manquants/invalides, `429` cuando los presupuestos stream/octet están pasados. |

### `GET /v2/sorafs/storage/chunk/{manifest_id}/{digest}`

Obtener un solo fragmento con los mismos encabezados, además del resumen determinado por
trozo. Útil para reintentos o descargas forenses cuando
Las rodajas de CAR son inútiles.

## Flujo de trabajo del orquestador de múltiples fuentes

Cuando se activa la búsqueda de SF-6 de múltiples fuentes (CLI Rust a través de `sorafs_fetch`,
SDK a través de `sorafs_orchestrator`):1. **Recoge los platos principales** — decodifica el plan de trozos del manifiesto, recupera
   Los últimos anuncios y, opcionalmente, pasar una instantánea de televisión.
   (`--telemetry-json` o `TelemetrySnapshot`).
2. **Construir un marcador** — `Orchestrator::build_scoreboard` évalue
   l'éligibilité et registrar les raisons de rejet ; `sorafs_fetch --scoreboard-out`
   persistir el JSON.
3. **Planificador de fragmentos de archivos** — `fetch_with_scoreboard` (o `--plan`) impone archivos
   rango de restricciones, flujo de presupuestos, límites de reintento/peer (`--retry-budget`,
   `--max-peers`) y lanzó un alcance de token de transmisión en el manifiesto para cada caso.
   solicitar.
4. **Vérifier les reçus** — les sorties incluent `chunk_receipts` et
   `provider_reports`; los currículums CLI persistente `provider_reports`,
   `chunk_receipts` e `ineligible_providers` para los paquetes de preuves.

Errores corrientes recuperados por los operadores/SDK:

| Error | Descripción |
|--------|-------------|
| `no providers were supplied` | Aucune entrada elegible después del filtrado. |
| `no compatible providers available for chunk {index}` | Error de playa o presupuesto para una porción específica. |
| `retry budget exhausted after {attempts}` | Aumente `--retry-budget` o évincez les peers en échec. |
| `no healthy providers remaining` | Todos los proveedores se desactivan después de repetir la operación. |
| `streaming observer failed` | Le escritor CAR aguas abajo avorté. |
| `orchestrator invariant violated` | Capture el manifiesto, el marcador, la instantánea de televisión y JSON CLI para la clasificación. |

## Télémétrie et preuves- Métriques émises par l'orchestrateur :  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (taggées par manifest/région/fournisseur). Définissez `telemetry_region` es
  Configure o a través de flags CLI para particionar los paneles de control de la flota.
- Los currículums de fetch CLI/SDK incluyen el marcador JSON persistente, los recursos
  de chunks et les rapports fournisseurs qui doivent voyager dans les bundles
  de despliegue para las puertas SF-6/SF-7.
- Puerta de enlace de los manejadores expuesta `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  Para que los paneles de control SRE correspondan a las decisiones orquestadas con el
  Servidor de comportamiento.

## Ayudas CLI y REST

- Archivos embalsados `iroha app sorafs pin list|show`, `alias list` y `replication list`
  puntos finales REST del registro de pines e impresión del Norito JSON bruto con bloques
  atestación para la auditoría.
- `iroha app sorafs storage pin` e `torii /v2/sorafs/pin/register` aceptan des
  manifiesta Norito o JSON, además de las pruebas de alias opcionales y sucesores;
  des pruebas mal formadas renvoient `400`, des pruebas obsolètes exponennt `503` avec
  `Warning: 110`, y las pruebas caducadas se entregan a `412`.
- Los puntos finales REST (`/v2/sorafs/pin`, `/v2/sorafs/aliases`,
  `/v2/sorafs/replication`) incluye las estructuras de atestación para que les
  Los clientes verifican las donaciones con los últimos encabezados de bloque antes de girar.

## Referencias- Especificación canónica :
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito : `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ayudas CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Orquestador de cajas: `crates/sorafs_orchestrator`
- Paquete de paneles de control: `dashboards/grafana/sorafs_fetch_observability.json`