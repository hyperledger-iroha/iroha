---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado-capacidad-de-almacenamiento
título: Marketplace de capacidade de armazenamento SoraFS
sidebar_label: Mercado de capacidad
descripción: Plano SF-2c para mercado de capacidad, órdenes de replicación, telemetría y ganchos de gobierno.
---

:::nota Fuente canónica
Esta página espelha `docs/source/sorafs/storage_capacity_marketplace.md`. Mantenha ambos os locais alinhados mientras la documentacao herdada permanece activa.
:::

# Marketplace de capacidad de armazenamento SoraFS (Rascunho SF-2c)

El artículo SF-2c de la hoja de ruta introduce un mercado gobernado por proveedores de
armazenamento declaram capacidade comprometida, recebem órdenes de replicacao e
Ganham fee proporcionais una disponibilidade entregue. Este documento delimita
os entregaveis exigidos para a primeira release e os divide em trilhas acionaveis.

## Objetivos

- Expresar compromisos de capacidad (bytes totales, límites por carril, caducidad)
  En formato verificavel consumivel por gobernador, transporte SoraNet e Torii.
- Alocar pines entre proveedores de acuerdo com capacidade declarada, participación e
  restricciones de política manteniendo el comportamiento determinístico.
- Medir entrega de armazenamento (éxito de replicación, uptime, pruebas de
  integridade) e exportar telemetría para distribuir tarifas.
- Prover procesos de revogacao e disputa para que proveedores desonestos sejam
  penalizados o removidos.

## Conceptos de dominio| Concepción | Descripción | Entrega inicial |
|---------|-----------|--------------------|
| `CapacityDeclarationV1` | Payload Norito descrevendo ID del proveedor, soporte de perfil de fragmentador, GiB comprometidos, límites por carril, sugerencias de precios, compromiso de participación y vencimiento. | Esquema + validador en `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrucciones emitidas para el gobierno que atribuye un CID de manifiesto a uno o más proveedores, incluido el nivel de redundancia y métricas SLA. | Esquema Norito compartido con Torii + API de contrato inteligente. |
| `CapacityLedger` | Registro dentro y fuera de la cadena que rastreia declaracoes de capacidade ativas, órdenes de replicación, métricas de desempeño y acumulación de tarifas. | Módulo de contrato inteligente o trozo de servicio fuera de cadena con instantánea determinística. |
| `MarketplacePolicy` | Política de gobierno que define el juego mínimo, los requisitos de auditoría y las curvas de penalidad. | Estructura de configuración en `sorafs_manifest` + documento de gobierno. |

### Esquemas implementados (estado)

## Desdobramento de trabajo

### 1. Camada de esquema y registro| Tarefa | Responsavel(es) | Notas |
|------|------------------|-------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipo de Almacenamiento / Gobernanza | Usar Norito; Incluir versiones semánticas y referencias de capacidad. |
| Implementar módulos de analizador + validador en `sorafs_manifest`. | Equipo de almacenamiento | Impor IDs monotonicos, limites de capacidade, requisitos de juego. |
| Estender metadatos del registro fragmentador con `min_capacity_gib` por perfil. | Grupo de Trabajo sobre Herramientas | Ayuda a los clientes a importar requisitos mínimos de hardware por perfil. |
| Redigir documento `MarketplacePolicy` capturando guardrails de admissao e calendario de penalidades. | Consejo de Gobierno | Publicar em docs junto con defaults de politica. |

#### Definiciones de esquema (implementadas)- `CapacityDeclarationV1` captura compromisos de capacidad assinados por proveedor, incluyendo maneja canónicos do fragmentador, referencias de capacidad, límites opcionales por carril, sugerencias de precios, janelas de validación y metadatos. A validacao garante stake nao zero, handles canonicos, alias deduplicados, caps por lane dentro del total declarado e contabilidade de GiB monotonicamente creciente. [cajas/sorafs_manifest/src/capacity.rs:28]
- `ReplicationOrderV1` vincula manifests a atribuicoes emitidos pelagobernanza com metas de redundancia, limiares de SLA e garantías por atribuicao; validadores impoem maneja canónicos, proveedores únicos y restricciones de fecha límite antes de Torii ou o registro ingerirem a ordem. [crates/sorafs_manifest/src/capacity.rs:301]
- `CapacityTelemetryV1` instantáneas rápidas por época (GiB declarados vs utilizados, contadores de replicación, porcentajes de tiempo de actividad/PoR) que alimentan a distribución de tarifas. Checagens de limites mantem utilizacao dentro de las declaraciones y porcentajes dentro de 0-100%. [crates/sorafs_manifest/src/capacity.rs:476]
- Helpers compartilhados (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de lane/atribuicao/SLA) fornecem validacao deterministica de chaves e report de error reutilizavel by CI e tools downstream. [crates/sorafs_manifest/src/capacity.rs:230]- `PinProviderRegistry` ahora expone una instantánea en cadena a través de `/v2/sorafs/capacity/state`, combinando declaraciones de proveedores y entradas del libro mayor de tarifas por medio de Norito JSON determinístico. [crates/iroha_torii/src/sorafs/registry.rs:17] [crates/iroha_torii/src/sorafs/api.rs:64]
- Una cobertura de validación ejercida la aplicación de controles canónicos, detección de duplicados, límites por carril, guardas de atribución de replicación y comprobaciones de rango de telemetría para que regrese inmediatamente a CI. [crates/sorafs_manifest/src/capacity.rs:792]
- Herramientas para operadores: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convierte especificaciones legivas en cargas útiles Norito canónicas, blobs base64 y resúmenes JSON para que los operadores preparen accesorios de `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` y órdenes de replicación con validación local. [crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1] Accesorios de referencia viven en `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) y sao gerados vía `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integración del plano de control| Tarefa | Responsavel(es) | Notas |
|------|------------------|-------|
| Controladores adicionales Torii `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` con cargas útiles Norito JSON. | Torii Equipo | Espelhar logica do validador; reutilizar ayudantes Norito JSON. |
| Propagar instantáneas `CapacityDeclarationV1` para metadatos del marcador del orquestador y planos de búsqueda del portal. | Equipo de trabajo de herramientas / orquestador | Estender `provider_metadata` com referencias de capacidad para que o puntuación de múltiples fuentes respeite limites por carril. |
| Alimentar órdenes de replicación en clientes de Orchestrator/Gateway para orientar atributos y sugerencias de conmutación por error. | Equipo de Networking TL / Gateway | O constructor del marcador consome órdenes assinadas pelagobernanza. |
| CLI de herramientas: extender `sorafs_cli` con `capacity declare`, `capacity telemetry`, `capacity orders import`. | Grupo de Trabajo sobre Herramientas | Fornecer JSON determinístico + dichas de marcador. |

### 3. Política de mercado y gobierno| Tarefa | Responsavel(es) | Notas |
|------|------------------|-------|
| Ratificar `MarketplacePolicy` (stake minimo, multiplicadores de penalidade, cadencia de auditoria). | Consejo de Gobierno | Publicar em docs, capturar histórico de revisiones. |
| Agregar ganchos de gobierno para que el Parlamento pueda aprobar, renovar y revocar declaraciones. | Consejo de Gobernanza / Equipo de Contratos Inteligentes | Usar eventos Norito + ingestao de manifests. |
| Implementar calendario de penalidades (reducción de honorarios, reducción de fianzas) ligado a violaciones de SLA telegrafadas. | Consejo de Gobierno / Tesorería | Alinhar com salidas de liquidación do `DealEngine`. |
| Documental proceso de disputa y matriz de escalamiento. | Documentos / Gobernanza | Enlace al runbook de disputa + ayudantes de CLI. |

### 4. Medición y distribución de tarifas| Tarefa | Responsavel(es) | Notas |
|------|------------------|-------|
| Expandir la ingesta de medición de Torii para aceitar `CapacityTelemetryV1`. | Torii Equipo | Validar GiB-hora, éxito PoR, tiempo de actividad. |
| Actualizar tubería de medición de `sorafs_node` para reportar utilización por orden + estadísticas SLA. | Equipo de almacenamiento | Alinhar com órdenes de replicacao e handles de fragmentador. |
| Pipeline de liquidación: convertidor de telemetría + datos de replicación en pagos denominados en XOR, producción de resumos prontos para gobierno y registro del estado del libro mayor. | Equipo de Tesorería / Almacenamiento | Conectar ao Deal Engine / Exportaciones de tesorería. |
| Exportar paneles/alertas para saude do metering (acumulación de ingesta, telemetría obsoleta). | Observabilidad | Estender pack de Grafana referenciado por SF-6/SF-7. |- Torii ahora expoe `/v2/sorafs/capacity/telemetry` e `/v2/sorafs/capacity/state` (JSON + Norito) para que los operadores envien instantáneas de telemetría por época e inspectores recuperem o libro mayor canónico para auditoría o empacotamento de evidencia. [crates/iroha_torii/src/sorafs/api.rs:268] [crates/iroha_torii/src/sorafs/api.rs:816]
- La integración `PinProviderRegistry` garantiza que las órdenes de replicación se ejecutan de forma accesoria en el mismo punto final; Los ayudantes de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) permiten validar y publicar telemetría a partir de ejecuciones automatizadas con hash determinístico y resolución de alias.
- Instantáneas de medición de productos entradas `CapacityTelemetrySnapshot` fijadas a instantánea `metering`, y exportaciones Prometheus alimentan la placa Grafana pronto para importar en `docs/source/grafana_sorafs_metering.json` para que equipos de monitorización de faturamento acumulacao de GiB-hora, fee nano-SORA proyectados y cumplimiento de SLA en tiempo real. [crates/iroha_torii/src/routing.rs:5143] [docs/source/grafana_sorafs_metering.json:1]
- Cuando el suavizado de medición está habilitado, la instantánea incluye `smoothed_gib_hours` e `smoothed_por_success_bps` para que los operadores comparen valores con EMA contra contadores brutos usados que se gobiernan para pagos. [crates/sorafs_node/src/metering.rs:401]

### 5. Tratamento de disputa e revogacao| Tarefa | Responsavel(es) | Notas |
|------|------------------|-------|
| Definir carga útil `CapacityDisputeV1` (reclamante, evidencia, proveedor alvo). | Consejo de Gobierno | Esquema Norito + validador. |
| Soporte de CLI para abrir disputas y responder (con anexos de evidencia). | Grupo de Trabajo sobre Herramientas | Garantía de hash determinístico del paquete de evidencia. |
| Adicionar checks automatizados para violacoes SLA repetidas (auto-escalada para disputa). | Observabilidad | Limiares de alerta e ganchos de gobierno. |
| Documental playbook de revogacao (periodo de graca, evacuacao de dados pinados). | Equipo de Documentos/Almacenamiento | Linkar ao doc de politica e runbook de operador. |

## Requisitos de testes e CI

- Testes unitarios para todos los novos validadores de esquema (`sorafs_manifest`).
- Pruebas de integración que simulan: declaración -> orden de réplica -> medición -> pago.
- Workflow de CI para regenerar declaracoes/telemetria de capacidade y garantizar que assinaturas permanecam sincronizadas (estender `ci/check_sorafs_fixtures.sh`).
- Pruebas de carga para una API de registro (10k proveedores simulares, 100k órdenes).

## Telemetría y paneles de control- Dolores de tablero:
  - Capacidade declarada vs utilizada por proveedor.
  - Backlog de órdenes de replicacao e atraso medio de atribuicao.
  - Cumplimiento de SLA (uptime %, taxa de sucesso PoR).
  - Devengo de tasas y penalidades por época.
- Alertas:
  - Proveedor abaixo da capacidade minima comprometida.
  - Ordem de replicacao travada > SLA.
  - Falhas ninguna tubería de medición.

## Entregaveis de documentacao

- Guía de operador para declarar capacidad, renovar compromisos y monitorear utilización.
- Guía de gobierno para aprobar declaraciones, emitir órdenes y lidar con disputas.
- Referencia de API para endpoints de capacidad y formato de orden de replicación.
- Preguntas frecuentes sobre el mercado para desarrolladores.

## Lista de verificación de preparación GA

El elemento de la hoja de ruta **SF-2c** bloquea el lanzamiento en producción con base en evidencia concreta
sobre contabilidad, tratamiento de disputas e incorporación. Use os artefatos abaixo para
Manter os criterios de aceitacao em sync com a implementacao.### Contabilidade notturna e reconciliacao XOR
- Exportar instantánea de estado de capacidad y exportar libro mayor XOR para mesma janela,
  depósitos ejecutar:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  O helper sai com codigo nao zero em asentamientos ou penalidades ausentes/excessivas e emite
  Un resumen Prometheus en formato de archivo de texto.
- O alerta `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`)
  Dispara quando métricas de reconciliacao reportam gaps; tableros ficam em
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archivar el resumen JSON y hashes en `docs/examples/sorafs_capacity_marketplace_validation/`
  junto com pacotes de gobernadora.

### Evidencia de disputa y corte
- Archivar disputas vía `sorafs_manifest_stub capacity dispute` (pruebas:
  `cargo test -p sorafs_car --test capacity_cli`) para mantener cargas útiles canónicas.
- Ejecutar `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` e como suites
  de penalidade (`record_capacity_telemetry_penalises_persistent_under_delivery`) para provar
  que disputas e slashes fazem replay determinístico.
- Siga `docs/source/sorafs/dispute_revocation_runbook.md` para captura de evidencia y escalamiento;
  linke aprovacoes de strike no relatorio de validacao.

### Incorporación de proveedores y pruebas de humo de salida
- Regenere artefatos de declaracao/telemetria com `sorafs_manifest_stub capacity ...` e rode os
  pruebas de CLI antes de la presentación (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Submeta vía Torii (`/v2/sorafs/capacity/declare`) y captura `/v2/sorafs/capacity/state` más
  Las capturas de pantalla hacen Grafana. Sigue el flujo de dicha en `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archivo de artefatos asesinados y salidas de reconciliacao dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependencias y secuenciación1. Finalizar SF-2b (política de admisión): el mercado depende de los proveedores aprobados.
2. Implementar camada de esquema + registro (este doc) antes de la integración Torii.
3. Completar tubería de medición antes de habilitar los pagos.
4. Etapa final: habilitar la distribución de tarifas controladas por el gobierno cuando los datos de
   medición forem verificados em staging.

El progreso debe ser rastreado sin hoja de ruta con referencias a este documento. actualizar o
hoja de ruta asim que cada secoo principal (esquema, plano de control, integracao, medición,
tratamento de disputa) característica de estado de atingir completa.