---
lang: es
direction: ltr
source: docs/source/sorafs/storage_capacity_marketplace.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2f830d4a4549c49d0c9649b728ab6ec00b0b7bcfcbd5a50230686ff0cd0648d7
source_last_modified: "2026-01-22T15:38:30.696290+00:00"
translation_last_reviewed: 2026-01-30
---

# Mercado de capacidad de almacenamiento SoraFS (borrador SF-2c)

El ítem del roadmap SF-2c introduce un mercado gobernado donde los proveedores
de almacenamiento declaran capacidad comprometida, reciben órdenes de replicación y
ganan fees proporcionales a la disponibilidad entregada. Este documento acota
los entregables requeridos para la primera versión y los divide en pistas
accionables.

## Objetivos

- Expresar compromisos de capacidad de proveedor (bytes totales, límites por
  lane, expiración) en una forma verificable consumible por gobernanza,
  transporte SoraNet y Torii.
- Asignar pins entre proveedores según capacidad declarada, stake y
  restricciones de política manteniendo un comportamiento determinista.
- Medir la entrega de almacenamiento (éxito de replicación, uptime, pruebas de
  integridad) y exportar telemetría para la distribución de fees.
- Proveer procesos de revocación y disputa para que proveedores deshonestos
  puedan ser penalizados o removidos.

## Conceptos de dominio

| Concepto | Descripción | Entregable inicial |
|----------|-------------|--------------------|
| `CapacityDeclarationV1` | Payload Norito que describe ID de proveedor, soporte de perfil de chunker, GiB comprometidos, límites por lane, hints de pricing, compromiso de staking y expiración. | Esquema + validador en `sorafs_manifest::capacity`. |
| `ReplicationOrder` | Instrucción emitida por gobernanza que asigna un CID de manifiesto a uno o más proveedores, incluyendo nivel de redundancia y métricas SLA. | Esquema Norito compartido con Torii + API de smart contract. |
| `CapacityLedger` | Registro on-chain/off-chain que rastrea declaraciones de capacidad activas, órdenes de replicación, métricas de desempeño y acumulación de fees. | Módulo de smart contract o stub de servicio off-chain con snapshot determinista. |
| `MarketplacePolicy` | Política de gobernanza que define stake mínimo, requisitos de auditoría y curvas de penalización. | Estructura de config en `sorafs_manifest` + documento de gobernanza. |

### Esquemas implementados (estado)

## Desglose de trabajo

### 1. Capa de esquema y registro

| Tarea | Responsable(s) | Notas |
|------|----------------|-------|
| Definir `CapacityDeclarationV1`, `ReplicationOrderV1`, `CapacityTelemetryV1`. | Storage Team / Governance | Usar Norito; incluir versionado semántico y referencias de capacidades. |
| Implementar módulos de parser + validador en `sorafs_manifest`. | Storage Team | Aplicar IDs monótonos, límites de capacidad, requisitos de stake. |
| Extender los metadatos del registro de chunker con `min_capacity_gib` por perfil. | Tooling WG | Ayuda a clientes a aplicar requisitos mínimos de hardware por perfil. |
| Redactar el documento `MarketplacePolicy` que capture guardrails de admisión y el plan de penalizaciones. | Governance Council | Publicar en docs junto con los defaults de política. |

#### Definiciones de esquema (implementadas)

- `CapacityDeclarationV1` captura compromisos de capacidad firmados por proveedor,
  incluyendo handles canónicos de chunker, referencias de capacidad, límites de
  lane opcionales, hints de pricing, ventanas de validez y metadatos. La
  validación garantiza stake no cero, handles canónicos, alias deduplicados,
  límites por lane dentro del total declarado y contabilidad GiB monótona.【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` vincula manifiestos a asignaciones emitidas por
gobernanza con objetivos de redundancia, umbrales SLA y garantías por
asignación; los validadores aplican handles canónicos de chunker, proveedores
únicos y restricciones de deadline antes de que Torii o el registro ingieran la
orden.【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` expresa snapshots por epoch (GiB declarados vs
  utilizados, contadores de replicación, porcentajes de uptime/PoR) que
  alimentan la distribución de fees. Las comprobaciones de límites mantienen la
  utilización dentro de las declaraciones y los porcentajes dentro de 0 – 100 %.【crates/sorafs_manifest/src/capacity.rs:476】
- Helpers compartidos (`CapacityMetadataEntry`, `PricingScheduleV1`, validadores
  de lane/asignación/SLA) proporcionan validación determinista de claves y
  reportes de errores que CI y tooling downstream pueden reutilizar.【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` ahora expone el snapshot on-chain vía
  `/v2/sorafs/capacity/state`, combinando declaraciones de proveedores y
  entradas del fee ledger detrás de JSON Norito determinista.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- La cobertura de validación ejercita la aplicación de handles canónicos, la
  detección de duplicados, límites por lane, guardas de asignación de
  replicación y comprobaciones de rango de telemetría para que las regresiones
  se detecten inmediatamente en CI.【crates/sorafs_manifest/src/capacity.rs:792】
- Tooling de operador: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}`
  convierte especificaciones legibles en payloads Norito canónicos, blobs base64
  y resúmenes JSON para que los operadores preparen
  `/v2/sorafs/capacity/declare`, `/v2/sorafs/capacity/telemetry` y fixtures de
  órdenes de replicación con validación local.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】
  Los fixtures de referencia viven en `fixtures/sorafs_manifest/replication_order/`
  (`order_v1.json`, `order_v1.to`) y se generan con
  `cargo run -p sorafs_manifest --bin generate_replication_order_fixture`.

### 2. Smart contract / plano de control

| Tarea | Responsable(s) | Notas |
|------|----------------|-------|
| Prototipar contrato de registro (`PinProviderRegistry`) con CRUD para declaraciones de capacidad y órdenes de replicación. | Core Infra / Smart Contract Team | Asegurar hashing determinista y paridad de codificación Norito. |
| Exponer servicio gRPC/REST (`/v2/sorafs/capacity`) que refleje el estado del contrato para Torii/gateways. | Core Infra | Proveer paginación + atestación (hash de bloque, prueba). |
| Implementar ledger de acumulación de fees con rate card básico (GiB · hora * precio). | Economics WG / Core Infra | Exportar snapshots del ledger para integración de facturación. |
| Añadir hooks de disputa/resolución (ventana de challenge, envío de evidencia). | Governance Council | Determinar timeouts y penalizaciones por defecto. |

### 3. Integración de Torii y nodo SoraFS

| Tarea | Responsable(s) | Notas |
|------|----------------|-------|
| Torii: ingerir `CapacityDeclarationV1` y exponerlo vía API de discovery. | Networking TL | Alinear con los flujos existentes de provider advert. |
| `sorafs-node`: persistir asignaciones de replicación, programar descargas, aplicar cuotas por proveedor. | Storage Team | Construir encima del orquestador de fetch multi-source. |
| Actualizaciones de CLI: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}`, `sorafs_fetch --capacity-plan`. | Tooling WG | Proveer reportes JSON para operadores. |
| Telemetría: publicar `capacity_commitment_bytes`, `capacity_utilisation_percent`, `replication_order_backlog`. | Observability | Alimentar dashboards + alertas. |

- La API de Torii ahora acepta envíos al registro de capacidad mediante endpoints dedicados:
  - `POST /v2/sorafs/capacity/declare` envuelve un `CapacityDeclarationV1` firmado y encola la
    instrucción `RegisterCapacityDeclaration` correspondiente.【crates/iroha_torii/src/routing.rs:4390】【crates/iroha_torii/src/lib.rs:3175】
  - `POST /v2/sorafs/capacity/telemetry` registra snapshots de utilización por epoch mediante
    `RecordCapacityTelemetry`, aplicando límites de saneamiento antes del dispatch.【crates/iroha_torii/src/routing.rs:4744】【crates/iroha_torii/src/lib.rs:3248】
- Los payloads de telemetría ahora incluyen contadores PDP/PoTR para que la
  gobernanza pueda correlacionar fallos de prueba con facturación y
  enforcement. Los remitentes deben proveer `pdp_challenges`/`pdp_failures` y
  `potr_windows`/`potr_breaches`; Torii valida que los fallos nunca excedan el
  total de la ventana y expone errores descriptivos cuando faltan sondas. Estos
  contadores alimentan los nuevos knobs `SorafsPenaltyPolicy.max_pdp_failures`
  y `.max_potr_breaches` para que cualquier fallo de prueba pueda disparar un
  strike/slash inmediato sin esperar límites de utilización/uptime.
- La evidencia de fallos de prueba para gobernanza ahora se emite automáticamente.
  Siempre que `RecordCapacityTelemetry` recibe un snapshot cuyos contadores PDP
  o PoTR exceden los límites configurados, el runtime presenta un
  `proof_failure` `CapacityDisputeRecord` con un payload `CapacityDisputeV1`
  canónico. El digest de evidencia referencia el payload Norito de telemetría
  (recuperable vía la URI
  `norito://sorafs/capacity_telemetry/<provider_hex>/<start_epoch>-<end_epoch>`),
  para que gobernanza, revisores Taikai/CDN y auditores puedan obtener el
  snapshot exacto que disparó el strike sin depender de bundles ad hoc.
- `POST /v2/sorafs/capacity/schedule` permite a los operadores enviar payloads
  `ReplicationOrderV1` emitidos por gobernanza; el manager embebido en
  `sorafs-node` valida la orden, rastrea asignaciones pendientes y devuelve un
  resumen de scheduling con la capacidad restante para que las herramientas de
  orquestación actúen sobre el resultado. `POST /v2/sorafs/capacity/complete`
  libera reservas cuando termina la ingesta, alimentando telemetría de
  liberación en los snapshots locales de capacidad. El nodo inicializa un
  `TelemetryAccumulator` junto al scheduler para que operadores (o workers en
  background) puedan derivar payloads `CapacityTelemetryV1` canónicos que
  capturen GiB·hora, uptime y métricas de éxito PoR antes de publicar vía Torii.【crates/iroha_torii/src/routing.rs:4806】【crates/sorafs_node/src/lib.rs:110】【crates/sorafs_node/src/telemetry.rs:1】
- La medición local ahora expone endpoints dedicados de observación.
  `POST /v2/sorafs/capacity/uptime`, `POST /v2/sorafs/capacity/por` y
  `POST /v2/sorafs/capacity/failure` actualizan el `CapacityMeter` embebido, el
  acumulador de telemetría y los gauges Prometheus sin emitir transacciones,
  garantizando que los datos de sonda y fallos de replicación alimenten
  dashboards y lógica de acumulación de fees de inmediato.【crates/iroha_torii/src/routing.rs:5023】【crates/iroha_torii/src/lib.rs:5301】
- El borrador de perfil de gateway sin confianza enumera la matriz de
  solicitud/respuesta HTTP, formatos de prueba y expectativas de telemetría que
  los gateways deben cumplir antes de entrar en la suite de conformidad SF-5.
  Consulte `docs/source/sorafs_gateway_profile.md` para la especificación
  normativa.
- `GET /v2/sorafs/capacity/state` ahora incluye una proyección `local_usage`
  que reporta GiB comprometidos/asignados, reservas por chunker, utilización por
  lane, órdenes pendientes y contadores de medición en vivo (GiB·hora, muestras
  de uptime/PoR, conteos de replicación) provenientes del medidor embebido. Un
  payload `telemetry_preview` refleja la presentación canónica de
  `CapacityTelemetryV1` para que los operadores comparen valores de dashboard
  contra el snapshot Norito antes de publicar nuevos adverts.【crates/iroha_torii/src/sorafs/api.rs:144】
- Los ledgers de crédito se exportan junto a los ledgers de fees en la misma
  respuesta. Cada entrada reporta crédito disponible, colateral bondado, conteos
  de strikes, totales de penalización y timestamps de saldo bajo para que la
  automatización de tesorería y los dashboards puedan bloquear pagos antes de que
  cierren las ventanas de settlement.【crates/iroha_torii/src/sorafs/registry.rs:123】【crates/iroha_torii/src/sorafs/api.rs:5096】
- Las disputas de capacidad son de primera clase en el registro de capacidad:
  `/v2/sorafs/capacity/state` ahora emite un arreglo `disputes` (con payloads
  base64, digests de evidencia y metadatos de estado), mientras
  `/v2/sorafs/capacity/dispute` acepta envíos firmados por gobernanza. Use el
  helper de CLI para fabricar solicitudes y anote el `dispute_id_hex` de la
  respuesta para revocaciones y trazabilidad de auditoría.【crates/iroha_torii/src/sorafs/api.rs:520】【crates/iroha_torii/src/routing.rs:4889】【docs/source/sorafs/dispute_revocation_runbook.md:45】
- `sorafs_manifest_stub capacity dispute` acepta una especificación declarativa
  al presentar disputas de gobernanza. Campos requeridos: `provider_id_hex`,
  `complainant_id_hex`, `kind` (`replication_shortfall`, `uptime_breach`,
  `proof_failure`, `fee_dispute` u `other`), `submitted_epoch`, `description` y
  un objeto `evidence` con `digest_hex` (BLAKE3-256). Campos opcionales incluyen
  `replication_order_id_hex`, `requested_remedy`, `evidence.media_type`,
  `evidence.uri` y `evidence.size_bytes`. La CLI emite bytes Norito canónicos,
  payloads base64 y un cuerpo de solicitud listo para Torii para que los
  operadores puedan registrar disputas o archivar evidencia de forma
  determinista. Consulte `docs/source/sorafs/dispute_revocation_runbook.md` para
  el playbook de gobernanza end-to-end.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:35】
- `sorafs_manifest_stub capacity {declaration, telemetry, replication-order, complete}`
  ganó helpers `--request-out` (con `--authority`/`--private-key` para
  declaraciones y telemetría) para que los operadores emitan payloads JSON
  listos para publicar en los endpoints de Torii sin ensamblar manualmente los
  cuerpos de solicitud.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:20】

### 4. Medición y distribución de fees

| Tarea | Responsable(s) | Notas |
|------|----------------|-------|
| Definir tipos de prueba (éxito PoR, intervalos de uptime, confirmaciones de ticket). | Storage Team / Observability | Reusar metadatos de árbol PoR existentes. |
| Construir pipeline de medición que agregue métricas por proveedor y por epoch. | Observability / Economics WG | Emitir snapshots JSON consumidos por facturación. |
| Implementar cálculo de recompensas (participación base + multiplicador por desempeño). | Economics WG | Documentar fórmulas; asegurar redondeo determinista. |
| Flujo de gobernanza para aprobación de pagos y enforcement de penalizaciones. | Governance Council | Proveer CLI + docs para revisión de tesorería. |

- `sorafs_node` ahora incluye un `CapacityMeter` ligero que rastrea órdenes
  programadas/completadas, GiB declarados y porciones pendientes para que las
  ventanas de telemetría se llenen directamente desde el worker embebido sin
  re-derivar utilización off-chain.【crates/sorafs_node/src/metering.rs:1】【crates/sorafs_node/src/lib.rs:27】
- `/v2/sorafs/capacity/state` ahora emite un payload determinista
  `fee_projection` junto al snapshot `metering` en vivo, y las exportaciones
  Prometheus alimentan el tablero Grafana listo para importar en
  `docs/source/grafana_sorafs_metering.json` para que los equipos de facturación
  monitoricen la acumulación GiB·hora, fees nano-SORA proyectados y cumplimiento
  SLA en tiempo real.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- Cuando el suavizado de medición está habilitado, el snapshot incluye
  `smoothed_gib_hours` y `smoothed_por_success_bps` para que los operadores
  comparen valores con tendencia EMA frente a los contadores brutos que usa la
  gobernanza para los pagos.【crates/sorafs_node/src/metering.rs:401】【crates/iroha_torii/src/sorafs/api.rs:816】

### 5. Gestión de disputas y revocación

| Tarea | Responsable(s) | Notas |
|------|----------------|-------|
| Definir payload `CapacityDisputeV1` (denunciante, evidencia, proveedor objetivo). | Governance Council | Esquema Norito + validador. |
| Soporte de CLI para presentar disputas y responder (con adjuntos de evidencia). | Tooling WG | Asegurar hashing determinista del bundle de evidencia. |
| Añadir checks automatizados para incumplimientos SLA repetidos (auto-escalar a disputa). | Observability | Umbrales de alertas y hooks de gobernanza. |
| Documentar playbook de revocación (periodo de gracia, evacuación de datos fijados). | Docs / Storage Team | Enlazar a doc de política y runbook de operador. |

## Requisitos de pruebas y CI

- Unit tests para todos los nuevos validadores de esquema (`sorafs_manifest`).
- Integration tests que simulen: declaración → orden de replicación → medición → pago.
- Workflow de CI para regenerar declaraciones/telemetría de capacidad de ejemplo
  y asegurar que las firmas siguen sincronizadas (extender `ci/check_sorafs_fixtures.sh`).
- Load tests para la API del registro (simular 10k proveedores, 100k órdenes).

## Telemetría y dashboards

- Paneles de dashboard:
  - Capacidad declarada vs utilizada por proveedor.
  - Backlog de órdenes de replicación y retraso promedio de asignación.
  - Cumplimiento de SLA (uptime %, tasa de éxito PoR).
  - Acumulación de fees y penalizaciones por epoch.
- Alertas:
  - Proveedor por debajo de la capacidad comprometida mínima.
  - Orden de replicación atascada > SLA.
  - Fallos en el pipeline de medición.

## Entregables de documentación

- Guía de operador para declarar capacidad, renovar compromisos y monitorear utilización.
- Guía de gobernanza para aprobar declaraciones, emitir órdenes y manejar disputas.
- Referencia API para los endpoints de capacidad y el formato de órdenes de replicación.
- FAQ del marketplace para desarrolladores.
- Runbook de onboarding y salida de proveedores que cubra almacenamiento de artefactos,
  aprobaciones basadas en roles y comandos de verificación Torii
  (`docs/source/sorafs/capacity_onboarding_runbook.md`).

## Checklist de preparación para GA

El ítem del roadmap **SF-2c** condiciona el rollout de producción a evidencia
concreta en contabilidad, gestión de disputas y onboarding. Use los artefactos
siguientes para mantener los criterios de aceptación sincronizados con la
implementación.

### Contabilidad nocturna y reconciliación XOR

1. Exporte las transferencias XOR que ejecutó tesorería durante la ventana
   anterior y el snapshot `/v2/sorafs/capacity/state` correspondiente. Concílie
   los datos con el nuevo helper:
   ```bash
   python3 scripts/telemetry/capacity_reconcile.py \
     --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
     --ledger /var/lib/iroha/exports/sorafs-capacity-ledger-$(date +%F).ndjson \
     --label nightly-capacity \
     --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
     --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
   ```
   El script termina con código no cero cuando faltan settlements/penalizaciones
   o hay pagos en exceso, y emite un textfile Prometheus que refleja el resumen
   JSON para que la gobernanza pueda reproducir la evidencia.
2. Triangule la salida de reconciliación con
   `dashboards/grafana/sorafs_capacity_penalties.json` y la regla de Alertmanager
   `SoraFSCapacityReconciliationMismatch`
   (`dashboards/alerts/sorafs_capacity_rules.yml`). La regla dispara cuando las
   métricas de reconciliación reportan settlements faltantes/en exceso o
   transferencias inesperadas por proveedor.
3. Archive el digest de reconciliación nocturna junto a los paquetes de
   gobernanza en `docs/examples/sorafs_capacity_marketplace_validation/` para que
   las aprobaciones de tesorería referencien evidencia determinista. La lista de
   validación en `docs/source/sorafs/reports/capacity_marketplace_validation.md`
   enlaza el bundle de sign-off más reciente; extienda esa tabla cuando se
   revisen nuevas exportaciones.

### Evidencia de disputa y slashing

1. Genere payloads de disputa con el harness de CLI
   (`sorafs_manifest_stub capacity dispute` y
   `cargo test -p sorafs_car --test capacity_cli`) para que cada bundle
   `CapacityDisputeV1` tenga artefactos JSON/Norito canónicos.
2. Ejercite los hooks deterministas del ledger ejecutando
   `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` y las
   suites de penalización a las que se refiere el roadmap
   (`record_capacity_telemetry_penalises_persistent_under_delivery`). Estas
   pruebas viven en `crates/iroha_core/src/smartcontracts/isi/sorafs.rs` y
   garantizan que disputas, penalizaciones y slashes se reproduzcan fielmente
   entre nodos.
3. Siga el flujo de escalamiento en
   `docs/source/sorafs/dispute_revocation_runbook.md`—el runbook cubre captura de
   evidencia, votaciones del consejo y procedimientos de revocación/failover.
   Enlace las minutas resultantes o aprobaciones de strike de vuelta en
   `docs/source/sorafs/reports/capacity_marketplace_validation.md` para que los
   revisores puedan rastrear disputas individuales desde payload CLI → mutación
   del ledger → aprobación de gobernanza.

### Pruebas de humo de onboarding y salida de proveedores

1. Prepare declaraciones con `sorafs_manifest_stub capacity declaration --spec <file>`
   y reproduzca la regresión de CLI (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)
   antes de entregar los envíos a Torii. El helper emite `.to` Norito, JSON y
   salidas Base64 más los metadatos de plan de chunk descritos antes en esta guía.
2. Verifique el comportamiento de Torii llamando a `POST /v2/sorafs/capacity/declare`
   y `GET /v2/sorafs/capacity/state`—los registros deben coincidir con los
   defaults de gobernanza capturados en
   `docs/source/sorafs/provider_admission_policy.md` y el runbook en
   `docs/source/sorafs/runbooks/multi_source_rollout.md`.
