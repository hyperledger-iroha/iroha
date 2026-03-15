---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: mercado-capacidad-de-almacenamiento
título: Mercado de capacidad de almacenamiento de SoraFS
sidebar_label: Mercado de capacidad
descripción: Plan SF-2c para el mercado de capacidad, órdenes de replicación, telemetría y ganchos de gobernanza.
---

:::nota Fuente canónica
Esta página refleja `docs/source/sorafs/storage_capacity_marketplace.md`. Mantenga ambos lugares alineados mientras la documentación heredada siga activa.
:::

# Mercado de capacidad de almacenamiento de SoraFS (Borrador SF-2c)

El item del roadmap SF-2c introduce un mercado gobernado donde los proveedores de
almacenamiento declaran capacidad comprometida, reciben órdenes de replicación y
ganan tarifas proporcionales a la disponibilidad entregada. Este documento delimita
los entregables requeridos para la primera liberación y los divide en pistas accionables.

## Objetivos

- Expresar compromisos de capacidad (bytes totales, límites por carril, caducidad)
  en una forma verificable consumible por gobernanza, transporte SoraNet y Torii.
- Asignar pines entre proveedores según capacidad declarada, participación y restricciones de
  politica mientras se mantiene un comportamiento determinista.
- Medir la entrega de almacenamiento (éxito de replicación, uptime, pruebas de integridad) y
  exportar telemetria para distribucion de honorarios.
- Proveer procesos de revocacion y disputa para que proveedores deshonestos sean
  penalizados o removidos.## Conceptos de dominio

| Concepto | Descripción | Entrega inicial |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Carga útil Norito que describe ID de proveedor, soporte de perfil de fragmentador, GiB comprometidos, límites por carril, pistas de precios, compromiso de participación y vencimiento. | Esquema + validador en `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrucción emitida por gobernanza que asigna un CID de manifiesto a uno o más proveedores, incluyendo nivel de redundancia y métricas SLA. | Esquema Norito compartido con Torii + API de contrato inteligente. |
| `CapacityLedger` | Registro on-chain/off-chain que rastrea declaraciones de capacidad activa, órdenes de replicación, métricas de rendimiento y acumulación de tarifas. | Módulo de contrato inteligente o stub de servicio off-chain con snapshot determinista. |
| `MarketplacePolicy` | Política de gobernanza que define el juego mínimo, los requisitos de auditoría y las curvas de penalización. | Estructura de configuración en `sorafs_manifest` + documento de gobernanza. |

### Esquemas implementados (estado)

##Desglose de trabajo

### 1. Capa de esquema y registro| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipo de Almacenamiento / Gobernanza | Usar Norito; incluir versionado semántico y referencias de capacidades. |
| Implementar módulos de analizador + validador en `sorafs_manifest`. | Equipo de almacenamiento | Imponer identificaciones monotónicas, límites de capacidad, requisitos de participación. |
| Extender metadatos del registro fragmentador con `min_capacity_gib` por perfil. | Grupo de Trabajo sobre Herramientas | Ayuda a los clientes a imponer requisitos mínimos de hardware por perfil. |
| Redactar el documento `MarketplacePolicy` con guardrails de admision y calendario de penalizaciones. | Consejo de Gobierno | Publicar en docs junto con defaults de politica. |

#### Definiciones de esquema (implementadas)- `CapacityDeclarationV1` captura compromisos de capacidad firmados por proveedor, incluyendo handles canonicos del chunker, referencias de capacidades, caps opcionales por lane, pistas de pricing, ventanas de validez y metadata. La validacion asegura stack no cero, handles canonicos, alias deduplicados, caps por lane dentro del total declarado y contabilidad de GiB monotónica.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` enlaza manifiestos con asignaciones emitidas por gobernanza que incluyen objetivos de redundancia, umbrales de SLA y garantías por asignación; los validadores imponen maneja canónicos, proveedores únicos y restricciones de fecha límite antes de que Torii o el registro consuman la orden.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` expresa instantáneas por época (GiB declarados vs usados, contadores de replicación, porcentajes de uptime/PoR) que alimentan la distribución de tarifas. Las validaciones mantienen el uso dentro de la declaración y los porcentajes dentro de 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Helpers compartidos (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de lane/asignacion/SLA) proveen validación determinista de claves y reportes de error que CI y tooling downstream pueden reutilizar.【crates/sorafs_manifest/src/capacity.rs:230】- `PinProviderRegistry` ahora exponen el snapshot on-chain vía `/v1/sorafs/capacity/state`, combinando declaraciones de proveedor y entradas del fee ledger con Norito JSON determinista.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La cobertura de validación ejercita cumplimiento de handles canónicos, detección de duplicados, límites por carril, guardas de asignación de replicación y comprobaciones de rango de telemetría para que las regresiones aparezcan en CI de inmediato.【crates/sorafs_manifest/src/capacity.rs:792】
- Tooling para operadores: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convierte especificaciones legibles en payloads Norito canónicos, blobs base64 y resúmenes JSON para que los operadores preparen accesorios de `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry` y órdenes de replicacion con validación local.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Los accesorios de referencia viven en `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) y se generan a través de `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integración del plano de control| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Agregar controladores Torii `/v1/sorafs/capacity/declare`, `/v1/sorafs/capacity/telemetry`, `/v1/sorafs/capacity/orders` con cargas útiles Norito JSON. | Torii Equipo | Reflejar la lógica del validador; reutilizar ayudantes Norito JSON. |
| Propagar instantáneas de `CapacityDeclarationV1` a metadatos del marcador del orquestador y planos de búsqueda de puerta de enlace. | Tooling WG / Equipo de Orquestador | Extender `provider_metadata` con referencias de capacidad para que el scoring multi-source respete limites por carril. |
| Inyectar órdenes de replicación en clientes de orquestador/gateway para guiar asignaciones y sugerencias de conmutación por error. | Equipo de Networking TL / Gateway | El constructor del marcador consume órdenes firmadas por gobernanza. |
| CLI de herramientas: extensor `sorafs_cli` con `capacity declare`, `capacity telemetry`, `capacity orders import`. | Grupo de Trabajo sobre Herramientas | Proveer JSON determinista + salidas de marcador. |

### 3. Política del mercado y gobernanza| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Ratificar `MarketplacePolicy` (stake minimo, multiplicadores de penalizacion, cadencia de auditoria). | Consejo de Gobierno | Publicar en docs, capturar historial de revisión. |
| Agregar ganchos de gobernanza para que el Parlamento pueda aprobar, renovar y revocar declaraciones. | Consejo de Gobernanza / Equipo de Contratos Inteligentes | Usar eventos Norito + ingesta de manifiestos. |
| Implementar el esquema de penalizaciones (reducción de honorarios, recorte de fianzas) ligado a violaciones de SLA telegrafiadas. | Consejo de Gobierno / Tesorería | Alineal con salidas de liquidación de `DealEngine`. |
| Documentar el proceso de disputa y la matriz de escalada. | Documentos / Gobernanza | Vincular al runbook de disputa + helpers de CLI. |

### 4. Medición y distribución de honorarios| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Expandir la ingesta de medición en Torii para aceptar `CapacityTelemetryV1`. | Torii Equipo | Validar GiB-hora, salida PoR, tiempo de actividad. |
| Actualizar el pipeline de metering de `sorafs_node` para reportar utilización por orden + estadísticas SLA. | Equipo de almacenamiento | Alinear con ordenes de replicacion y handles de fragmentador. |
| Pipeline de liquidación: convertir telemetria + datos de replicacion en pagos denominados en XOR, producir resúmenes listos para gobernanza y registrar el estado del libro mayor. | Equipo de Tesorería / Almacenamiento | Conectar con exportaciones de Deal Engine / Treasury. |
| Exportar paneles/alertas para salud del metering (backlog de ingesta, telemetria stale). | Observabilidad | Extender el paquete de Grafana referenciado por SF-6/SF-7. |- Torii ahora exponen `/v1/sorafs/capacity/telemetry` y `/v1/sorafs/capacity/state` (JSON + Norito) para que los operadores envien snapshots de telemetria por epoca y los inspectores recuperen el ledger canonico para auditorias o empaquetado de evidencia.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- La integración `PinProviderRegistry` asegura que las órdenes de replicación sean accesibles por el mismo endpoint; helpers de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) ahora validan y publican telemetría desde ejecuciones automatizadas con hash determinista y resolución de alias.
- Los snapshots de metering producen entradas `CapacityTelemetrySnapshot` fijadas al snapshot `metering`, y los exports Prometheus alimentan el tablero Grafana listo para importar en `docs/source/grafana_sorafs_metering.json` para que los equipos de facturación monitoreen la acumulación de GiB-hour, fee nano-SORA proyectados y cumplimiento de SLA en tiempo real.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Cuando el suavizado de medición está habilitado, el snapshot incluye `smoothed_gib_hours` e `smoothed_por_success_bps` para que los operadores comparen valores con EMA frente a contadores crudos que la gobernanza usa para pagos.【crates/sorafs_node/src/metering.rs:401】

### 5. Manejo de disputas y revocaciones| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Definir la carga útil `CapacityDisputeV1` (demandante, evidencia, proveedor objetivo). | Consejo de Gobierno | Esquema Norito + validador. |
| Soporte de CLI para iniciar disputas y responder (con adjuntos de evidencia). | Grupo de Trabajo sobre Herramientas | Asegurar hash determinista del paquete de evidencia. |
| Agregar checks automatizados para violaciones SLA repetidas (auto-escalado a disputa). | Observabilidad | Umbrales de alerta y ganchos de gobernanza. |
| Documentar el playbook de revocacion (período de gracia, evacuación de datos pineados). | Equipo de Documentos/Almacenamiento | Vincular a documento de política y runbook de operadores. |

## Requisitos de pruebas y CI

- Pruebas unitarias para todos los validadores de esquema nuevos (`sorafs_manifest`).
- Pruebas de integración que simulan: declaración -> orden de replicación -> medición -> pago.
- Workflow de CI para regenerar declaraciones/telemetria de capacidad y asegurar que las firmas se mantengan sincronizadas (extender `ci/check_sorafs_fixtures.sh`).
- Pruebas de carga para la API del registro (10k proveedores simulares, 100k órdenes).

## Telemetría y paneles de control- Paneles de tablero:
  - Capacidad declarada vs utilizada por proveedor.
  - Backlog de órdenes de replicación y demora promedio de asignación.
  - Cumplimiento de SLA (uptime %, tasa de éxito PoR).
  - Devengo de tasas y penalizaciones por época.
- Alertas:
  - Proveedor por debajo de la capacidad mínima comprometida.
  - Orden de replicación atascada > SLA.
  - Fallas en el tubo de medición.

## Entregables de documentación

- Guía de operador para declarar capacidad, renovar compromisos y monitorear utilización.
- Guía de gobernanza para aprobar declaraciones, emitir órdenes y manejar disputas.
- Referencia de API para endpoints de capacidad y formato de orden de replicación.
- Preguntas frecuentes del mercado para desarrolladores.

## Lista de verificación de preparación para GA

El elemento de hoja de ruta **SF-2c** bloquea el lanzamiento en producción sobre evidencia concreta
acerca de contabilidad, manejo de disputas e incorporación. Usa los artefactos siguientes
para mantener los criterios de aceptación en sincronización con la implementación.### Contabilidad nocturna y reconciliacion XOR
- Exporta el snapshot de estado de capacidad y el export del ledger XOR para la misma
  ventana, luego ejecuta:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  El helper sale con código no cero si hay asentamientos o penalizaciones faltantes/excesivas y
  emite un resumen de Prometheus en formato de archivo de texto.
- La alerta `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`)
  dispara cuando las métricas de reconciliacion reportan lagunas; los tableros viven en
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiva el resumen JSON y los hashes en `docs/examples/sorafs_capacity_marketplace_validation/`
  junto con los paquetes de gobernanza.

### Evidencia de disputa y slashing
- Presenta disputas vía `sorafs_manifest_stub capacity dispute` (pruebas:
  `cargo test -p sorafs_car --test capacity_cli`) para mantener cargas útiles canónicas.
- Ejecuta `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` y las suites de
  penalizacion (`record_capacity_telemetry_penalises_persistent_under_delivery`) para probar que
  disputas y slashes se reproducen de manera determinista.
- Sigue `docs/source/sorafs/dispute_revocation_runbook.md` para captura de evidencia y escalada;
  enlaza las aprobaciones de huelgas en el reporte de validación.### Incorporación de proveedores y pruebas de humo de salida
- Regenera artefactos de declaracion/telemetria con `sorafs_manifest_stub capacity ...` y
  reejecuta las pruebas de CLI antes del envío (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Envialos via Torii (`/v1/sorafs/capacity/declare`) y luego captura `/v1/sorafs/capacity/state` mas
  capturas de pantalla de Grafana. Sigue el flujo de salida en `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiva artefactos firmados y salidas de reconciliación dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependencias y secuenciación

1. Terminar SF-2b (política de admisión): el mercado depende de proveedores validados.
2. Implementar esquema + capa de registro (este doc) antes de la integración de Torii.
3. Completar el pipeline de medición antes de habilitar los pagos.
4. Paso final: habilitar la distribución de tarifas controlada por gobernanza una vez que los datos
   de medición se verifiquen en puesta en escena.

El progreso debe rastrearse en la hoja de ruta con referencias a este documento. Actualiza el
hoja de ruta una vez que cada sección mayor (esquema, plano de control, integración, medición,
manejo de disputas) alcance estado característica completa.