---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/node-client-protocol.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Protocolo de no  cliente da SoraFS

Esta guía resume una definición canónica del protocolo em
[`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md).
Utilice una especificación ascendente para diseños Norito en nivel de bytes y registros de cambios;
una copia del portal mantem os destaques operatorais perto do restante dos runbooks
SoraFS.

## Anuncios de proveedor y validacao

Proveedores SoraFS diseminan cargas útiles `ProviderAdvertV1` (veja
`crates/sorafs_manifest::provider_advert`) asesinados por el operador gobernado. os
Los anuncios fijan los metadados de descoberta y las barandillas que el orquestador.
Aplicaciones de múltiples fuentes en tiempo de ejecución.- **Vigencia** - `issued_at < expires_at <= issued_at + 86,400 s`. proveedores
  devem renovar cada 12 horas.
- **TLVs de capacidade** - a lista TLV anuncia recursos de transporte (Torii,
  QUIC+Noise, relés SoraNet, extensos de fornecedor). Códigos desconhecidos
  podem ser ignorados cuando `allow_unknown_capabilities = true`, siguiendo a
  Orientacao GRASA.
- **Sugerencias de QoS** - nivel de `availability` (Caliente/Tibio/Frío), latencia máxima de
  recuperacao, límite de concordancia y presupuesto de flujo opcional. Desarrollo de QoS
  alinhar com a telemetria observada e e auditada na admissao.
- **Puntos finales y temas de encuentro** - URL de servicios concretos con metadados
  TLS/ALPN mais os temas de descoberta aos quais os clientes devem se inscrever
  ao construir conjuntos de guardia.
- **Politica de diversidade de caminho** - `min_guard_weight`, caps de fan-out de
  AS/pool e `provider_failure_threshold` tornam possiveis busca deterministas
  multipar.
- **Identificadores de perfil** - provedores devem export o handle canonico (ej.
  `sorafs.sf1@1.0.0`); `profile_aliases` opcionais ajudam clientes antigos a migrar.

Registros de validación de rejeitam de participación cero, listas de capacidades/puntos finales/temas,
vigencias de orden o objetivos de QoS ausentes. Comparar sobres de admisión
os corpos do advert e da proposta (`compare_core_fields`) antes de diseminar
actualizacoes.

### Extensiones de rango de búsqueda

Los proveedores con gama incluyen los siguientes metadados:| Campo | propuesta |
|-------|-----------|
| `CapabilityType::ChunkRangeFetch` | Declara `max_chunk_span`, `min_granularity` y banderas de alinhamento/prova. |
| `StreamBudgetV1` | Sobre opcional de concorrencia/throughput (`max_in_flight`, `max_bytes_per_sec`, `burst` opcional). Requiere capacidad de alcance. |
| `TransportHintV1` | Preferencias de transporte ordenadas (ej. `torii_http_range`, `quic_stream`, `soranet_relay`). Prioridades sao `0-15` e duplicados sao rejeitados. |

Soporte de herramientas:

- Las canalizaciones de anuncios del proveedor deben validar la capacidad de alcance, el presupuesto de flujo y
  transport tips antes de emitir deterministas para auditorias.
- `cargo xtask sorafs-admission-fixtures` agrupa anuncios canónicos de múltiples fuentes
  junto con accesorios de degradación en `fixtures/sorafs_manifest/provider_admission/`.
- Anuncios con rango que omiten `stream_budget` o `transport_hints` sao rejeitados
  Cargadores de pelos CLI/SDK antes de programar, mantener o aprovechar fuentes múltiples
  alinhado com as expectativas de admissao do Torii.

## Puntos finales de rango de puerta de enlace

Gateways aceitam requisicoes HTTP deterministas que espelham os metadados do
anuncio.

### `GET /v1/sorafs/storage/car/{manifest_id}`| Requisito | Detalles |
|-----------|----------|
| **Encabezados** | `Range` (janela unica alinhada aos offsets de chunk), `dag-scope: block`, `X-SoraFS-Chunker`, `X-SoraFS-Nonce` opcional e `X-SoraFS-Stream-Token` base64 obrigator. |
| **Respuestas** | `206` con `Content-Type: application/vnd.ipld.car`, `Content-Range` descrevendo a janela servida, metadados `X-Sora-Chunk-Range` y encabezados de fragmentador/token ecoados. |
| **falsas** | `416` para rangos desalinhados, `401` para tokens ausentes/invalidos, `429` cuando presupuestos de flujo/bytes sao excedidos. |

### `GET /v1/sorafs/storage/chunk/{manifest_id}/{digest}`

Recupera el fragmento único con varios encabezados y el resumen determinista del fragmento.
Util para reintentos o descargas forenses cuando cortes de CAR sao desnecesarios.

## Flujo de trabajo del orquestador multifuente

Cuando o fetch SF-6 de múltiples fuentes está habilitado (CLI Rust vía `sorafs_fetch`,
SDK a través de `sorafs_orchestrator`):1. **Coletar entradas** - decodificar o plano de chunks do manifest, puxar os
   anuncios más recientes y, opcionalmente, pasar una instantánea de telemetría
   (`--telemetry-json` o `TelemetrySnapshot`).
2. **Construcción del marcador** - `Orchestrator::build_scoreboard` disponible
   elegibilidade e registra razoes de rejeicao; `sorafs_fetch --scoreboard-out`
   persistir en JSON.
3. **fragmentos de agenda** - `fetch_with_scoreboard` (o `--plan`) reforca restricciones
   rango, presupuestos de flujo, límites de reintento/par (`--retry-budget`, `--max-peers`)
   y emite un token de transmisión con un formulario de manifiesto para cada solicitud.
4. **Verificar recibos** - según lo indicado se incluyen `chunk_receipts` e `provider_reports`;
   sumarios de CLI persistem `provider_reports`, `chunk_receipts` y
   `ineligible_providers` paquetes de pruebas para.

Errores comunes presentados a operadores/SDK:

| Error | Descripción |
|------|-----------|
| `no providers were supplied` | Nenhuma entrada elegivel apos o filtro. |
| `no compatible providers available for chunk {index}` | No coinciden el alcance o el presupuesto con una porción específica. |
| `retry budget exhausted after {attempts}` | Aumente `--retry-budget` o elimine peers con falha. |
| `no healthy providers remaining` | Todos los proveedores foram desabilitados apos falhas repetidas. |
| `streaming observer failed` | O escritor CAR aguas abajo abortou. |
| `orchestrator invariant violated` | Capture manifiesto, marcador, instantánea de telemetría y CLI JSON para clasificación. |

## Telemetria y evidencias- Métricas emitidas por el orquestador:  
  `sorafs_orchestrator_active_fetches`, `sorafs_orchestrator_fetch_duration_ms`,
  `sorafs_orchestrator_retries_total`, `sorafs_orchestrator_provider_failures_total`
  (tagueadas por manifiesto/región/proveedor). Defina `telemetry_region` en la configuración
  o a través de banderas de CLI para participar en paneles de control.
- Resúmenes de fetch sin CLI/SDK que incluyen marcador JSON persistente, recibos fragmentados
  El proveedor informa que estamos desarrollando paquetes de implementación para las puertas SF-6/SF-7.
- Expoem de controladores de puerta de enlace `telemetry::sorafs.fetch.lifecycle|retry|provider_failure|error`
  para que los tableros SRE correlacionen las decisiones del orquestador con el comportamiento
  hacer servidor.

## Ayudantes de CLI y REST

- `iroha app sorafs pin list|show`, `alias list` e `replication list` involucran sistemas operativos
  endpoints REST do pin-registry e imprimem Norito JSON bruto con bloques de
  atestación para evidencias de auditoria.
- `iroha app sorafs storage pin` e `torii /v1/sorafs/pin/register` aceitam manifiestos
  Norito o JSON con alias de pruebas y sucesores opcionales; pruebas malformadas
  Geram `400`, pruebas obsoletas retornam `503` con `Warning: 110`, y pruebas vencidas
  retorno `412`.
- Puntos finales REST (`/v1/sorafs/pin`, `/v1/sorafs/aliases`, `/v1/sorafs/replication`)
  Incluye estructuras de atestación para que los clientes verifiquen datos contra el sistema operativo.
  ultimos headers de bloco antes de agir.

## Referencias- Especificaciones canónicas:
  [`docs/source/sorafs_node_client_protocol.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs_node_client_protocol.md)
- Tipos Norito: `crates/sorafs_manifest/src/{provider_advert,provider_admission}.rs`
- Ayudantes de CLI: `crates/iroha_cli/src/commands/sorafs.rs`,
  `crates/sorafs_car/src/bin/sorafs_fetch.rs`
- Caja del orquestador: `crates/sorafs_orchestrator`
- Pack de cuadros de mando: `dashboards/grafana/sorafs_fetch_observability.json`