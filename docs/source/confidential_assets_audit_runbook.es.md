---
lang: es
direction: ltr
source: docs/source/confidential_assets_audit_runbook.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8691a94d23e589f46d8e8cf2359d6d9a31f7c38c5b7bf0def69c88d2dd081765
source_last_modified: "2026-01-22T15:38:30.658489+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

//! Manual de operaciones y auditoría de activos confidenciales al que hace referencia `roadmap.md:M4`.

# Runbook de operaciones y auditoría de activos confidenciales

Esta guía consolida las superficies de evidencia en las que confían los auditores y operadores.
al validar flujos de activos confidenciales. Complementa el manual de rotación.
(`docs/source/confidential_assets_rotation.md`) y el libro de calibración
(`docs/source/confidential_assets_calibration.md`).

## 1. Divulgación selectiva y feeds de eventos

- Cada instrucción confidencial emite una carga útil estructurada `ConfidentialEvent`
  (`Shielded`, `Transferred`, `Unshielded`) capturado en
  `crates/iroha_data_model/src/events/data/events.rs:198` y serializado por el
  ejecutores (`crates/iroha_core/src/smartcontracts/isi/world.rs:3699` – `4021`).
  La suite de regresión ejercita las cargas útiles concretas para que los auditores puedan confiar en
  diseños JSON deterministas (`crates/iroha_core/tests/zk_confidential_events.rs:19` – `299`).
- Torii expone estos eventos a través del canal estándar SSE/WebSocket; auditores
  suscríbete usando `ConfidentialEventFilter` (`crates/iroha_data_model/src/events/data/filters.rs:82`),
  opcionalmente, alcance una única definición de activo. Ejemplo de CLI:

  ```bash
  iroha ledger events data watch --filter '{ "confidential": { "asset_definition_id": "rose#wonderland" } }'
  ```

- Los metadatos de políticas y las transiciones pendientes están disponibles a través de
  `GET /v2/confidential/assets/{definition_id}/transitions`
  (`crates/iroha_torii/src/routing.rs:15205`), reflejado en el SDK de Swift
  (`IrohaSwift/Sources/IrohaSwift/ToriiClient.swift:3245`) y documentado en
  Tanto el diseño de activos confidenciales como las guías del SDK.
  (`docs/source/confidential_assets.md:70`, `docs/source/sdk/swift/index.md:334`).

## 2. Telemetría, paneles y evidencia de calibración

- Las métricas de tiempo de ejecución muestran la profundidad del árbol, el historial de compromiso/frontera, el desalojo de raíz
  contadores y proporciones de aciertos de caché del verificador
  (`crates/iroha_telemetry/src/metrics.rs:5760`–`5815`). Paneles de control Grafana en
  `dashboards/grafana/confidential_assets.json` envía los paneles asociados y
  alertas, con el flujo de trabajo documentado en `docs/source/confidential_assets.md:401`.
- Ejecuciones de calibración (NS/op, gas/op, ns/gas) con registros firmados en vivo
  `docs/source/confidential_assets_calibration.md`. El último Apple Silicon
  NEON run está archivado en
  `docs/source/confidential_assets_calibration_neon_20260428.log`, y lo mismo
  El libro mayor registra las exenciones temporales para los perfiles SIMD-neutral y AVX2 hasta que
  los hosts x86 se conectan.

## 3. Respuesta a incidentes y tareas del operador

- Los procedimientos de rotación/actualización residen en
  `docs/source/confidential_assets_rotation.md`, que cubre cómo montar nuevos
  paquetes de parámetros, programar actualizaciones de políticas y notificar a billeteras/auditores. el
  listas de rastreadores (`docs/source/project_tracker/confidential_assets_phase_c.md`)
  propietarios de runbooks y expectativas de ensayo.
- Para ensayos de producción o ventanas de emergencia, los operadores adjuntan evidencia a
  Entradas `status.md` (por ejemplo, el registro de ensayo de varios carriles) e incluyen:
  `curl` prueba de transiciones de políticas, instantáneas Grafana y el evento relevante
  resúmenes para que los auditores puedan reconstruir los cronogramas de acuñación → transferencia → revelación.

## 4. Cadencia de revisión externa

- Alcance de la revisión de seguridad: circuitos confidenciales, registros de parámetros, política.
  transiciones y telemetría. Este documento más los formularios del libro de contabilidad de calibración.
  el paquete de pruebas enviado a los proveedores; La programación de revisiones se rastrea a través de
  M4 en `docs/source/project_tracker/confidential_assets_phase_c.md`.
- Los operadores deben mantener `status.md` actualizado con cualquier hallazgo o seguimiento del proveedor.
  elementos de acción. Hasta que se complete la revisión externa, este runbook sirve como base
  Los auditores de referencia operativa pueden realizar pruebas.