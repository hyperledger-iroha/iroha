---
lang: es
direction: ltr
source: docs/automation/android/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 27b5ac3c7adb19a87f0b3d076f3c9618b188602898ed3954808ac9f7a52b3a62
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Baseline de automatización de documentación Android (AND5)

El elemento del roadmap AND5 exige que la automatización de documentación,
localización y publicación sea auditable antes de que AND6 (CI & Compliance)
pueda comenzar. Esta carpeta registra los comandos, artefactos y el diseño de
la evidencia que AND5/AND6 referencian, reflejando los planes capturados en
`docs/source/sdk/android/developer_experience_plan.md` y
`docs/source/sdk/android/parity_dashboard_plan.md`.

## Pipelines y comandos

| Tarea | Comando(s) | Artefactos esperados | Notas |
|------|------------|----------------------|-------|
| Sincronización de stubs de localización | `python3 scripts/sync_docs_i18n.py` (opcionalmente pasar `--lang <code>` por ejecución) | Archivo de log almacenado en `docs/automation/android/i18n/<timestamp>-sync.log` más los commits de stubs traducidos | Mantiene `docs/i18n/manifest.json` alineado con los stubs traducidos; el log registra los códigos de idioma tocados y el commit capturado en la baseline. |
| Verificación de fixtures + paridad Norito | `ci/check_android_fixtures.sh` (envuelve `python3 scripts/check_android_fixtures.py --json-out artifacts/android/parity/<stamp>/summary.json`) | Copiar el resumen JSON generado en `docs/automation/android/parity/<stamp>-summary.json` | Verifica payloads de `java/iroha_android/src/test/resources`, hashes de manifiestos y longitudes de fixtures firmados. Adjunta el resumen junto con la evidencia de cadencia bajo `artifacts/android/fixture_runs/`. |
| Manifiesto de samples y prueba de publicación | `scripts/publish_android_sdk.sh --version <semver> [--repo-url …]` (ejecuta tests + SBOM + provenance) | Metadatos del bundle de provenance y el `sample_manifest.json` resultante de `docs/source/sdk/android/samples/` almacenado en `docs/automation/android/samples/<version>/` | Une las apps de ejemplo AND5 con la automatización de releases: captura el manifiesto generado, el hash del SBOM y el log de provenance para la revisión beta. |
| Feed del panel de paridad | `python3 scripts/check_android_fixtures.py … --json-out artifacts/android/parity/<stamp>/summary.json` seguido de `python3 scripts/android_parity_metrics.py --summary <summary> --output artifacts/android/parity/<stamp>/metrics.prom` | Copiar el snapshot `metrics.prom` o el export JSON de Grafana en `docs/automation/android/parity/<stamp>-metrics.prom` | Alimenta el plan de dashboard para que AND5/AND7 puedan verificar contadores de envíos inválidos y adopción de telemetría. |

## Captura de evidencia

1. **Marca todo con timestamp.** Nombra los archivos usando marcas UTC
   (`YYYYMMDDTHHMMSSZ`) para que los dashboards de paridad, las actas de
   gobernanza y los docs publicados puedan referenciar la misma ejecución.
2. **Referencia commits.** Cada log debe incluir el hash de commit de la
   ejecución y cualquier configuración relevante (por ejemplo,
   `ANDROID_PARITY_PIPELINE_METADATA`). Cuando la privacidad requiera
   redacción, incluye una nota y enlaza al vault seguro.
3. **Archivo mínimo de contexto.** Solo se registran resúmenes estructurados
   (JSON, `.prom`, `.log`). Artefactos pesados (APK bundles, screenshots) deben
   permanecer en `artifacts/` o en object storage con un hash firmado registrado
   en el log.
4. **Actualiza entradas de estado.** Cuando los hitos AND5 avancen en
   `status.md`, cita el archivo correspondiente (por ejemplo,
   `docs/automation/android/parity/20260324T010203Z-summary.json`) para que los
   auditores puedan rastrear la baseline sin revisar logs de CI.

Seguir este layout satisface el requisito de AND6 de “baselines de
docs/automation disponibles para auditoría” y mantiene el programa de
documentación Android alineado con los planes publicados.
