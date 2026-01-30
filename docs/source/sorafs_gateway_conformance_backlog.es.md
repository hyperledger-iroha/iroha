---
lang: es
direction: ltr
source: docs/source/sorafs_gateway_conformance_backlog.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d01d4577391cff6d0b4c547866a0f795699fc0c0e86112a91685a98f6d22387
source_last_modified: "2025-11-07T10:28:14.055720+00:00"
translation_last_reviewed: "2026-01-30"
---

# Backlog de conformidad del gateway SoraFS

Este documento vivo registra el trabajo requerido para entregar el harness de
conformidad SF-5a, cubriendo verificacion de replay, pruebas de carga,
automatizacion de release y reportes de governance.

## Desglose de hitos

### 1. Nucleo del harness de replay (Owner: Conformance WG, Issue: SF-5a-REPLAY)
- **Alcance**
  - Implementar un shim adaptador HTTP (Tokio + reqwest) con inyeccion determinista de headers.
  - Conectar la ingesta de manifiestos Norito y el pipeline de verificacion de pruebas (digest BLAKE3, validacion PoR).
  - Emitir reportes de atestacion firmados en Norito usando el hook de firmado definido en `sorafs_gateway_conformance.md`.
- **Dependencias**
  - Indice de fixtures (`fixtures/sorafs_gateway/index.norito.json`).
  - Helpers de schema de tokens desde `sorafs_token_schema`.
- **Entregables**
  - Crate Rust `sorafs_gateway_cert::replay`.
  - Fixtures golden + tests de regresion (`cargo test -p sorafs_gateway_cert -- replay_*`).
  - Muestras de atestacion archivadas bajo `artifacts/sorafs_gateway/replay/`.

### 2. Runner de carga concurrente (Owner: Reliability WG, Issue: SF-5a-LOAD)
- **Alcance**
  - Construir un generador de workload con semillas que sostenga >= 1 000 streams de rango concurrentes.
  - Capturar telemetria por request (histogramas de latencia, resultados de pruebas) y exportar via Prometheus.
  - Soportar inyeccion de fallas (timeouts, corrupcion PoR) controlada por flags de CLI.
- **Dependencias**
  - Librerias del harness de replay (reusar adaptador HTTP + pipeline de pruebas).
  - Pipeline de export de metricas (`sorafs_telemetry`).
- **Entregables**
  - Modulo `sorafs_gateway_cert::load` con integracion CLI (`--profile mock-chaos`).
  - Dashboards de telemetria agregados a `dashboards/sorafs_gateway_conformance.json`.
  - Documentacion que cubra perfiles de carga dentro de este backlog.

### 3. Empaquetado CLI (`sorafs-gateway-cert`) (Owner: Tooling WG, Issue: SF-5a-CLI)
- **Alcance**
  - Exponer flujos de replay y carga via un solo CLI con subcomandos:
    - `replay run --scenario sf1/full`
    - `load run --profile mock-chaos`
    - `report sign --input report.json`
  - Implementar discovery de configuracion (`.sorafs-gateway-cert/config.toml` + overrides de env).
- **Entregables**
  - Binary crate `sorafs-gateway-cert`.
  - Guia de instalacion en `docs/source/sorafs_gateway_cert_cli.md`.
  - Smoke tests ejecutados en CI (`cargo run --bin sorafs-gateway-cert -- replay --help`).

### 4. Automatizacion de publicacion de fixtures (Owner: Build Infra, Issue: SF-5a-FIXTURES)
- **Alcance**
  - Crear job de CI (`.buildkite/sorafs-fixtures.yml`) que empaquete bundles de fixtures, firme manifiestos y suba artefactos.
  - Publicar la lista de manifiestos firmados en Norito a `fixtures/releases/latest.manifest`.
  - Verificar firmas durante la ejecucion del pipeline y fallar ante drift.
- **Entregables**
  - Paso de Buildkite integrado en pipelines nightly y de merge.
  - Politica de retencion de artefactos documentada en `docs/source/fixtures_retention.md`.
  - Script de validacion `scripts/verify_sorafs_fixtures.sh` (envuelve `cargo xtask sorafs-gateway-fixtures --verify`) invocado pre-merge para que digests de fixtures, archivos auxiliares y matrices de escenarios solo cambien con commits intencionales.

### 5. Integracion CI / Nightly (Owner: CI WG, Issue: SF-5a-CI)
- **Alcance**
  - Agregar pipeline `ci/sorafs-gateway-cert` que ejecute perfiles de replay + carga contra builds nightly.
  - Introducir gate de merge para PRs que toquen gateway, orchestrator o codigo de fixtures.
- **Entregables**
  - Configuracion Buildkite con notificaciones al servicio PagerDuty `svc_sorafs_cert`.
  - Metricas exportadas a `ci/sorafs-gateway-cert:{profile}`.
  - Actualizaciones de documentacion CI en `docs/source/ci_matrix.md`.

### 6. Integracion de dashboard de governance (Owner: GovOps WG, Issue: SF-5a-DASHBOARD)
- **Alcance**
  - Exponer el estado de conformidad, el ultimo hash de atestacion y fallas pendientes en el dashboard de governance.
  - Proveer vistas de drill-down para corridas recientes con links a artefactos.
- **Entregables**
  - Paneles del dashboard (`dashboards/governance.json` secciones `sorafs_conformance_*`).
  - Job de ingesta de telemetria que lee sobres Norito de atestacion y actualiza el datastore del dashboard.
  - Entrada de runbook operacional para interpretar alertas del dashboard.

## Mejoras futuras

1. **Perfil HTTP/3 (QUIC)** — Extender el harness para ejercitar endpoints QUIC una vez el gateway los soporte (SF-5a-QUIC).
2. **Adaptadores de corrupcion/falla** — Implementar injectores modulares de fallas (p. ej., drop de headers, pruebas retrasadas) para tests personalizados de operadores (SF-5a-FAULTS).
3. **Laboratorio de latencia sintetica** — Proveer un harness de inyeccion de latencia controlada para dry runs de observabilidad (SF-5a-LATENCY).
4. **Alineacion de telemetria TLS** — Incorporar metricas de handshake TLS desde SF-5b en los reportes de conformidad para asegurar instrumentacion consistente (SF-5b-TLS-BRIDGE).

## Seguimiento de estado

| Workstream | Issue ID | Estado | Proximo checkpoint | Notas |
|------------|----------|--------|-------------------|-------|
| Replay harness core | SF-5a-REPLAY | En diseno | 2026-03-05 | A la espera de la finalizacion del indice de fixtures. |
| Load runner | SF-5a-LOAD | Planeado | 2026-03-08 | Distribucion de seeds acordada con Reliability WG. |
| CLI packaging | SF-5a-CLI | En progreso | 2026-03-04 | El scaffolding de parsing esta en revision. |
| Fixture publication | SF-5a-FIXTURES | No iniciado | 2026-03-06 | Requiere aprobacion de secretos Buildkite. |
| CI integration | SF-5a-CI | Planeado | 2026-03-10 | Depende de disponibilidad del CLI. |
| Governance dashboard | SF-5a-DASHBOARD | Planeado | 2026-03-12 | Mock-ups de diseno esperan sign-off de GovOps. |

Revisar y actualizar esta tabla al final de cada sprint para mantener a los stakeholders alineados con el roadmap.
