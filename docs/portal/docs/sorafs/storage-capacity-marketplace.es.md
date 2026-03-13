---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: d3468021402674d55f48d9eabe4cb36ceb6cbc2b704e9db95654373c544450d4
source_last_modified: "2025-11-20T07:32:59.870730+00:00"
translation_last_reviewed: 2026-01-30
---

---
id: storage-capacity-marketplace
title: Mercado de capacidad de almacenamiento de SoraFS
sidebar_label: Mercado de capacidad
description: Plan SF-2c para el mercado de capacidad, ordenes de replicacion, telemetria y hooks de gobernanza.
---

:::note Fuente canónica
Esta página refleja `docs/source/sorafs/storage_capacity_marketplace.md`. Mantén ambos lugares alineados mientras la documentación heredada siga activa.
:::

# Mercado de capacidad de almacenamiento de SoraFS (Borrador SF-2c)

El item del roadmap SF-2c introduce un mercado gobernado donde los providers de
almacenamiento declaran capacidad comprometida, reciben ordenes de replicacion y
ganan fees proporcionales a la disponibilidad entregada. Este documento delimita
los entregables requeridos para la primera release y los divide en tracks accionables.

## Objetivos

- Expresar compromisos de capacidad (bytes totales, limites por lane, expiracion)
  en una forma verificable consumible por gobernanza, transporte SoraNet y Torii.
- Asignar pins entre providers segun capacidad declarada, stake y restricciones de
  politica mientras se mantiene comportamiento determinista.
- Medir la entrega de storage (exito de replicacion, uptime, proofs de integridad) y
  exportar telemetria para distribucion de fees.
- Proveer procesos de revocacion y disputa para que providers deshonestos sean
  penalizados o removidos.

## Conceptos de dominio

| Concepto | Descripcion | Entregable inicial |
|---------|-------------|---------------------|
| `CapacityDeclarationV1` | Payload Norito que describe ID de provider, soporte de perfil de chunker, GiB comprometidos, limites por lane, pistas de pricing, compromiso de staking y expiracion. | Esquema + validador en `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instruccion emitida por gobernanza que asigna un CID de manifest a uno o mas providers, incluyendo nivel de redundancia y metricas SLA. | Esquema Norito compartido con Torii + API de smart contract. |
| `CapacityLedger` | Registry on-chain/off-chain que rastrea declaraciones de capacidad activas, ordenes de replicacion, metricas de rendimiento y accrual de fees. | Modulo de smart contract o stub de servicio off-chain con snapshot determinista. |
| `MarketplacePolicy` | Politica de gobernanza que define stake minimo, requisitos de auditoria y curvas de penalizacion. | Struct de config en `sorafs_manifest` + documento de gobernanza. |

### Esquemas implementados (estado)

## Desglose de trabajo

### 1. Capa de esquema y registry

| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Equipo de Storage / Gobernanza | Usar Norito; incluir versionado semantico y referencias de capacidades. |
| Implementar modulos de parser + validador en `sorafs_manifest`. | Equipo de Storage | Imponer IDs monotónicos, limites de capacidad, requisitos de stake. |
| Extender metadata del chunker registry con `min_capacity_gib` por perfil. | Tooling WG | Ayuda a los clientes a imponer requisitos minimos de hardware por perfil. |
| Redactar el documento `MarketplacePolicy` con guardrails de admision y calendario de penalizaciones. | Governance Council | Publicar en docs junto con defaults de politica. |

#### Definiciones de esquema (implementadas)

- `CapacityDeclarationV1` captura compromisos de capacidad firmados por provider, incluyendo handles canonicos del chunker, referencias de capacidades, caps opcionales por lane, pistas de pricing, ventanas de validez y metadata. La validacion asegura stake no cero, handles canonicos, aliases deduplicados, caps por lane dentro del total declarado y contabilidad de GiB monotónica.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` enlaza manifests con asignaciones emitidas por gobernanza que incluyen objetivos de redundancia, umbrales de SLA y garantias por asignacion; los validadores imponen handles canonicos, providers unicos y restricciones de deadline antes de que Torii o el registry consuman la orden.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` expresa snapshots por epoca (GiB declarados vs usados, contadores de replicacion, porcentajes de uptime/PoR) que alimentan la distribucion de fees. Las validaciones mantienen el uso dentro de la declaracion y los porcentajes dentro de 0-100%.【crates/sorafs_manifest/src/capacity.rs:476】
- Helpers compartidos (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores de lane/asignacion/SLA) proveen validacion determinista de keys y reportes de error que CI y tooling downstream pueden reutilizar.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` ahora expone el snapshot on-chain via `/v2/sorafs/capacity/state`, combinando declaraciones de provider y entradas del fee ledger con Norito JSON determinista.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La cobertura de validacion ejercita enforcement de handles canonicos, deteccion de duplicados, limites por lane, guardas de asignacion de replicacion y checks de rango de telemetria para que las regresiones aparezcan en CI de inmediato.【crates/sorafs_manifest/src/capacity.rs:792】
- Tooling para operadores: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` convierte especificaciones legibles en payloads Norito canonicos, blobs base64 y resúmenes JSON para que los operadores preparen fixtures de `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` y ordenes de replicacion con validacion local.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 Los fixtures de referencia viven en `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) y se generan via `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order`.

### 2. Integracion del plano de control

| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Agregar handlers Torii `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry`, `/v2/sorafs/capacity/orders` con payloads Norito JSON. | Torii Team | Reflejar la logica del validador; reutilizar helpers Norito JSON. |
| Propagar snapshots de `CapacityDeclarationV1` a metadata del scoreboard del orquestador y planes de fetch de gateway. | Tooling WG / Equipo de Orchestrator | Extender `provider_metadata` con referencias de capacidad para que el scoring multi-source respete limites por lane. |
| Inyectar ordenes de replicacion en clientes de orquestador/gateway para guiar asignaciones y hints de failover. | Networking TL / Gateway team | El builder del scoreboard consume ordenes firmadas por gobernanza. |
| Tooling CLI: extender `sorafs_cli` con `capacity declare`, `capacity telemetry`, `capacity orders import`. | Tooling WG | Proveer JSON determinista + salidas de scoreboard. |

### 3. Politica del marketplace y gobernanza

| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Ratificar `MarketplacePolicy` (stake minimo, multiplicadores de penalizacion, cadencia de auditoria). | Governance Council | Publicar en docs, capturar historial de revisiones. |
| Agregar hooks de gobernanza para que el Parlamento pueda aprobar, renovar y revocar declaraciones. | Governance Council / Smart Contract team | Usar eventos Norito + ingesta de manifests. |
| Implementar el esquema de penalizaciones (reduccion de fees, slashing de bond) ligado a violaciones de SLA telegrafiadas. | Governance Council / Treasury | Alinear con outputs de settlement de `DealEngine`. |
| Documentar el proceso de disputa y la matriz de escalamiento. | Docs / Governance | Vincular al runbook de disputa + helpers de CLI. |

### 4. Medicion y distribucion de fees

| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Expandir la ingesta de metering en Torii para aceptar `CapacityTelemetryV1`. | Torii Team | Validar GiB-hour, exito PoR, uptime. |
| Actualizar el pipeline de metering de `sorafs_node` para reportar utilizacion por orden + estadisticas SLA. | Storage Team | Alinear con ordenes de replicacion y handles de chunker. |
| Pipeline de settlement: convertir telemetria + datos de replicacion en pagos denominados en XOR, producir resúmenes listos para gobernanza y registrar el estado del ledger. | Treasury / Storage Team | Conectar con exportaciones de Deal Engine / Treasury. |
| Exportar dashboards/alertas para salud del metering (backlog de ingesta, telemetria stale). | Observability | Extender el pack de Grafana referenciado por SF-6/SF-7. |

- Torii ahora expone `/v2/sorafs/capacity/telemetry` y `/v2/sorafs/capacity/state` (JSON + Norito) para que operadores envien snapshots de telemetria por epoca y los inspectores recuperen el ledger canonico para auditorias o empaquetado de evidencia.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- La integracion `PinProviderRegistry` asegura que las ordenes de replicacion sean accesibles por el mismo endpoint; helpers de CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) ahora validan y publican telemetria desde ejecuciones automatizadas con hashing determinista y resolucion de aliases.
- Los snapshots de metering producen entradas `CapacityTelemetrySnapshot` fijadas al snapshot `metering`, y los exports Prometheus alimentan el tablero Grafana listo para importar en `docs/source/grafana_sorafs_metering.json` para que los equipos de facturacion monitoreen la acumulacion de GiB-hour, fees nano-SORA proyectados y cumplimiento de SLA en tiempo real.【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Cuando el smoothing de metering esta habilitado, el snapshot incluye `smoothed_gib_hours` y `smoothed_por_success_bps` para que operadores comparen valores con EMA frente a contadores crudos que la gobernanza usa para pagos.【crates/sorafs_node/src/metering.rs:401】

### 5. Manejo de disputas y revocaciones

| Tarea | Responsable(s) | Notas |
|------|----------------|------|
| Definir el payload `CapacityDisputeV1` (demandante, evidencia, provider objetivo). | Governance Council | Esquema Norito + validador. |
| Soporte de CLI para iniciar disputas y responder (con adjuntos de evidencia). | Tooling WG | Asegurar hashing determinista del bundle de evidencia. |
| Agregar checks automatizados para violaciones SLA repetidas (auto-escalado a disputa). | Observability | Umbrales de alerta y hooks de gobernanza. |
| Documentar el playbook de revocacion (periodo de gracia, evacuacion de datos pineados). | Docs / Storage Team | Vincular a documento de politica y runbook de operadores. |

## Requisitos de testing y CI

- Tests unitarios para todos los validadores de esquema nuevos (`sorafs_manifest`).
- Tests de integracion que simulan: declaracion -> orden de replicacion -> metering -> payout.
- Workflow de CI para regenerar declaraciones/telemetria de capacidad y asegurar que las firmas se mantengan sincronizadas (extender `ci/check_sorafs_fixtures.sh`).
- Tests de carga para la API del registry (simular 10k providers, 100k ordenes).

## Telemetria y dashboards

- Paneles de dashboard:
  - Capacidad declarada vs utilizada por provider.
  - Backlog de ordenes de replicacion y demora promedio de asignacion.
  - Cumplimiento de SLA (uptime %, tasa de exito PoR).
  - Accrual de fees y penalizaciones por epoca.
- Alertas:
  - Provider por debajo de la capacidad minima comprometida.
  - Orden de replicacion atascada > SLA.
  - Fallas en el pipeline de metering.

## Entregables de documentacion

- Guia de operador para declarar capacidad, renovar compromisos y monitorear utilizacion.
- Guia de gobernanza para aprobar declaraciones, emitir ordenes y manejar disputas.
- Referencia de API para endpoints de capacidad y formato de orden de replicacion.
- FAQ del marketplace para developers.

## Checklist de readiness para GA

El item de roadmap **SF-2c** bloquea el rollout en produccion sobre evidencia concreta
acerca de contabilidad, manejo de disputas y onboarding. Usa los artefactos siguientes
para mantener los criterios de aceptacion en sync con la implementacion.

### Contabilidad nocturna y reconciliacion XOR
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
  El helper sale con codigo no cero si hay settlements o penalizaciones faltantes/excesivas y
  emite un resumen de Prometheus en formato textfile.
- La alerta `SoraFSCapacityReconciliationMismatch` (en `dashboards/alerts/sorafs_capacity_rules.yml`)
  dispara cuando las metricas de reconciliacion reportan gaps; los dashboards viven en
  `dashboards/grafana/sorafs_capacity_penalties.json`.
- Archiva el resumen JSON y los hashes en `docs/examples/sorafs_capacity_marketplace_validation/`
  junto con los paquetes de gobernanza.

### Evidencia de disputa y slashing
- Presenta disputas via `sorafs_manifest_stub capacity dispute` (tests:
  `cargo test -p sorafs_car --test capacity_cli`) para mantener payloads canonicos.
- Ejecuta `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` y las suites de
  penalizacion (`record_capacity_telemetry_penalises_persistent_under_delivery`) para probar que
  disputas y slashes se reproducen de manera determinista.
- Sigue `docs/source/sorafs/dispute_revocation_runbook.md` para captura de evidencia y escalamiento;
  enlaza las aprobaciones de strikes en el reporte de validacion.

### Onboarding de providers y exit smoke tests
- Regenera artefactos de declaracion/telemetria con `sorafs_manifest_stub capacity ...` y
  reejecuta los tests de CLI antes del submit (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`).
- Envialos via Torii (`/v2/sorafs/capacity/declare`) y luego captura `/v2/sorafs/capacity/state` mas
  screenshots de Grafana. Sigue el flujo de salida en `docs/source/sorafs/capacity_onboarding_runbook.md`.
- Archiva artefactos firmados y outputs de reconciliacion dentro de
  `docs/examples/sorafs_capacity_marketplace_validation/`.

## Dependencias y secuenciacion

1. Terminar SF-2b (politica de admision) — el marketplace depende de providers validados.
2. Implementar esquema + capa de registry (este doc) antes de la integracion de Torii.
3. Completar el pipeline de metering antes de habilitar payouts.
4. Paso final: habilitar distribucion de fees controlada por gobernanza una vez que los datos
   de metering se verifiquen en staging.

El progreso debe rastrearse en el roadmap con referencias a este documento. Actualiza el
roadmap una vez que cada seccion mayor (esquema, plano de control, integracion, metering,
manejo de disputas) alcance estado feature complete.
