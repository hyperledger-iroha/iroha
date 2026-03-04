---
lang: es
direction: ltr
source: docs/portal/docs/devportal/norito-rpc-adoption.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 39cbd5e448c8a868c50401c15466d7159cb08ad7be52dafb9e9dc66d5bba979d
source_last_modified: "2025-11-17T18:34:13.669847+00:00"
translation_last_reviewed: 2026-01-01
---

# Calendario de adopcion de Norito-RPC

> Las notas canonicas de planificacion viven en `docs/source/torii/norito_rpc_adoption_schedule.md`.  
> Esta copia del portal resume las expectativas de despliegue para autores de SDK, operadores y revisores.

## Objetivos

- Alinear cada SDK (Rust CLI, Python, JavaScript, Swift, Android) en el transporte binario Norito-RPC antes del cambio de produccion AND4.
- Mantener deterministas las puertas de fase, los paquetes de evidencia y los hooks de telemetria para que la gobernanza pueda auditar el despliegue.
- Facilitar la captura de evidencia de fixtures y canary con los helpers compartidos que indica el roadmap NRPC-4.

## Cronograma de fases

| Fase | Ventana | Alcance | Criterios de salida |
|------|---------|---------|---------------------|
| **P0 - Paridad de laboratorio** | Q2 2025 | Las suites smoke de Rust CLI + Python ejecutan `/v1/norito-rpc` en CI, el helper de JS pasa pruebas unitarias, el harness mock de Android ejercita ambos transportes. | `python/iroha_python/scripts/run_norito_rpc_smoke.sh` y `javascript/iroha_js/test/noritoRpcClient.test.js` en verde en CI; el harness de Android conectado a `./gradlew test`. |
| **P1 - Preview de SDK** | Q3 2025 | El paquete compartido de fixtures se versiona, `scripts/run_norito_rpc_fixtures.sh --sdk <label>` registra logs + JSON en `artifacts/norito_rpc/`, y los flags opcionales de transporte Norito se exponen en los samples de SDK. | Manifiesto de fixtures firmado, actualizaciones de README muestran uso opt-in, la API de preview de Swift disponible bajo el flag IOS2. |
| **P2 - Staging / preview AND4** | Q1 2026 | Los pools Torii de staging prefieren Norito, los clientes de preview AND4 de Android y las suites de paridad IOS2 de Swift usan por defecto el transporte binario, el dashboard de telemetria `dashboards/grafana/torii_norito_rpc_observability.json` poblado. | `docs/source/torii/norito_rpc_stage_reports.md` captura el canary, `scripts/telemetry/test_torii_norito_rpc_alerts.sh` pasa, el replay del harness mock de Android captura casos de exito/error. |
| **P3 - GA en produccion** | Q4 2026 | Norito se convierte en el transporte por defecto para todos los SDKs; JSON queda como fallback de brownout. Los jobs de release archivan artefactos de paridad con cada tag. | La checklist de release agrupa el output smoke de Norito para Rust/JS/Python/Swift/Android; se aplican los umbrales de alerta para los SLOs de tasa de error Norito vs JSON; `status.md` y las notas de release citan evidencia GA. |

## Entregables de SDK y hooks de CI

- **Rust CLI y harness de integracion** - extender las pruebas smoke de `iroha_cli pipeline` para forzar el transporte Norito una vez que `cargo xtask norito-rpc-verify` este disponible. Proteger con `cargo test -p integration_tests -- norito_streaming` (lab) y `cargo xtask norito-rpc-verify` (staging/GA), guardando artefactos bajo `artifacts/norito_rpc/`.
- **SDK de Python** - usar Norito RPC por defecto en el smoke de release (`python/iroha_python/scripts/release_smoke.sh`), mantener `run_norito_rpc_smoke.sh` como entrada de CI y documentar la paridad en `python/iroha_python/README.md`. Objetivo de CI: `PYTHON_BIN=python3 python/iroha_python/scripts/run_norito_rpc_smoke.sh`.
- **SDK de JavaScript** - estabilizar `NoritoRpcClient`, permitir que los helpers de governance/query usen por defecto Norito cuando `toriiClientConfig.transport.preferred === "norito_rpc"`, y capturar samples end-to-end en `javascript/iroha_js/recipes/`. CI debe ejecutar `npm test` y el job dockerizado `npm run test:norito-rpc` antes de publicar; provenance sube logs smoke de Norito bajo `javascript/iroha_js/artifacts/`.
- **SDK de Swift** - conectar el transporte Norito bridge bajo el flag IOS2, reflejar la cadencia de fixtures y asegurar que la suite de paridad Connect/Norito se ejecute dentro de los lanes Buildkite referenciados en `docs/source/sdk/swift/index.md`.
- **SDK de Android** - los clientes AND4 preview y el harness mock de Torii adoptan Norito, con telemetria de reintento/backoff documentada en `docs/source/sdk/android/networking.md`. El harness comparte fixtures con otros SDKs via `scripts/run_norito_rpc_fixtures.sh --sdk android`.

## Evidencia y automatizacion

- `scripts/run_norito_rpc_fixtures.sh` envuelve `cargo xtask norito-rpc-verify`, captura stdout/stderr y emite `fixtures.<sdk>.summary.json` para que los duenos de SDK tengan un artefacto determinista que adjuntar a `status.md`. Usa `--sdk <label>` y `--out artifacts/norito_rpc/<stamp>/` para mantener ordenados los paquetes de CI.
- `cargo xtask norito-rpc-verify` impone paridad de hash de esquema (`fixtures/norito_rpc/schema_hashes.json`) y falla si Torii retorna `X-Iroha-Error-Code: schema_mismatch`. Acompana cada falla con una captura de fallback JSON para depuracion.
- `scripts/telemetry/test_torii_norito_rpc_alerts.sh` y `dashboards/grafana/torii_norito_rpc_observability.json` definen los contratos de alerta para NRPC-2. Ejecuta el script despues de cada edicion del dashboard y guarda la salida de `promtool` en el paquete canary.
- `docs/source/runbooks/torii_norito_rpc_canary.md` describe los drills de staging y produccion; actualizalo cuando cambien los hashes de fixtures o los gates de alerta.

## Checklist para revisores

Antes de marcar un hito NRPC-4, confirma:

1. Los hashes del paquete de fixtures mas reciente coinciden con `fixtures/norito_rpc/schema_hashes.json` y el artefacto de CI correspondiente queda registrado en `artifacts/norito_rpc/<stamp>/`.
2. Los README de SDK y las docs del portal describen como forzar el fallback JSON y citan el transporte Norito por defecto.
3. Los dashboards de telemetria muestran paneles de tasa de error dual-stack con enlaces de alerta, y el dry run de Alertmanager (`scripts/telemetry/test_torii_norito_rpc_alerts.sh`) esta adjunto al tracker.
4. El calendario de adopcion aqui coincide con la entrada del tracker (`docs/source/torii/norito_rpc_tracker.md`) y el roadmap (NRPC-4) referencia el mismo paquete de evidencia.

Mantener la disciplina del calendario mantiene el comportamiento cross-SDK predecible y permite que la gobernanza audite la adopcion de Norito-RPC sin solicitudes a medida.
