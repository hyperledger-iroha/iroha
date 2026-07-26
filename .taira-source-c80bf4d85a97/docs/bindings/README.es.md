---
lang: es
direction: ltr
source: docs/bindings/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: cb91ce03aee552c65d15ed1c019da4b3b3db9d48d299b3374ca78b4a8c6c1781
source_last_modified: "2025-11-15T13:38:36.954059+00:00"
translation_last_reviewed: 2026-01-21
---

# Gobernanza de bindings y fixtures del SDK

WP1-E en el roadmap señala “docs/bindings” como el lugar canónico para mantener
el estado de bindings entre lenguajes. Este documento registra el inventario de
bindings, los comandos de regeneración, los guardas de deriva y las ubicaciones
de evidencia para que las puertas de paridad GPU (WP1-E/F/G) y el consejo de
cadencia entre SDKs tengan una referencia única.

## Guardarraíles compartidos
- **Playbook canónico:** `docs/source/norito_binding_regen_playbook.md` describe
  la política de rotación, la evidencia esperada y el flujo de escalamiento para
  Android, Swift, Python y futuras bindings.
- **Paridad del esquema Norito:** `scripts/check_norito_bindings_sync.py`
  (invocado por `scripts/check_norito_bindings_sync.sh` y con gate en CI vía
  `ci/check_norito_bindings_sync.sh`) bloquea builds cuando los artefactos de
  esquema Rust, Java o Python se desvían.
- **Watchdog de cadencia:** `scripts/check_fixture_cadence.py` lee los archivos
  `artifacts/*_fixture_regen_state.json` y aplica las ventanas de Mar/Vie
  (Android, Python) y Mié (Swift) para que los gates del roadmap tengan
  timestamps auditables.

## Matriz de bindings

| Binding | Puntos de entrada | Comando de fixtures/regeneración | Guardas de deriva | Evidencia |
|---------|-------------------|----------------------------------|-------------------|----------|
| Android (Java) | `java/iroha_android/` (`java/iroha_android/README.md`) | `scripts/android_fixture_regen.sh` → `artifacts/android_fixture_regen_state.json` | `scripts/check_android_fixtures.py`, `ci/check_android_fixtures.sh`, `java/iroha_android/run_tests.sh` | `artifacts/android/fixture_runs/` |
| Swift (iOS/macOS) | `IrohaSwift/` (`IrohaSwift/README.md`) | `scripts/swift_fixture_regen.sh` (opcionalmente `SWIFT_FIXTURE_ARCHIVE`) → `artifacts/swift_fixture_regen_state.json` | `scripts/check_swift_fixtures.py`, `ci/check_swift_fixtures.sh`, `scripts/swift_fixture_archive.py` | `docs/source/swift_parity_triage.md`, `docs/source/sdk/swift/ios2_fixture_cadence_brief.md` |
| Python | `python/iroha_python/` (`python/iroha_python/README.md`) | `scripts/python_fixture_regen.sh` → `artifacts/python_fixture_regen_state.json` | `scripts/check_python_fixtures.py`, `python/iroha_python/scripts/run_checks.sh` | `docs/source/norito_binding_regen_playbook.md`, `docs/source/sdk/python/connect_end_to_end.md` |
| JavaScript | `javascript/iroha_js/` (`docs/source/sdk/js/publishing.md`) | `npm run release:provenance`, `scripts/js_sbom_provenance.sh`, `scripts/js_signed_staging.sh` | `npm run test`, `javascript/iroha_js/scripts/verify-release-tarball.mjs`, `javascript/iroha_js/scripts/record-release-provenance.mjs` | `artifacts/js-sdk-provenance/`, `artifacts/js/npm_staging/`, `artifacts/js/verification/`, `artifacts/js/sbom/` |

## Detalles de bindings

### Android (Java)
El SDK de Android vive en `java/iroha_android/` y consume los fixtures Norito
canónicos generados por `scripts/android_fixture_regen.sh`. Ese helper exporta
blobs `.norito` frescos desde la toolchain Rust, actualiza
`artifacts/android_fixture_regen_state.json` y registra metadatos de cadencia
que consumen `scripts/check_fixture_cadence.py` y los dashboards de gobernanza.
La deriva se detecta con `scripts/check_android_fixtures.py` (también conectado a
`ci/check_android_fixtures.sh`) y con `java/iroha_android/run_tests.sh`, que
prueba los bindings JNI, la reproducción de cola WorkManager y los fallbacks de
StrongBox. La evidencia de rotación, notas de fallos y transcripciones de rerun
viven en `artifacts/android/fixture_runs/`.

### Swift (macOS/iOS)
`IrohaSwift/` refleja las mismas cargas Norito mediante
`scripts/swift_fixture_regen.sh`. El script registra el propietario de la
rotación, la etiqueta de cadencia y el origen (`live` vs `archive`) dentro de
`artifacts/swift_fixture_regen_state.json` y alimenta al verificador de cadencia.
`scripts/swift_fixture_archive.py` permite a los mantenedores ingerir archivos
Rust generados; `scripts/check_swift_fixtures.py` y `ci/check_swift_fixtures.sh`
aplican paridad a nivel de bytes y límites de edad de SLA, mientras
`scripts/swift_fixture_regen.sh` admite `SWIFT_FIXTURE_EVENT_TRIGGER` para
rotaciones manuales. El flujo de escalamiento, los KPIs y los dashboards están
en `docs/source/swift_parity_triage.md` y los briefs de cadencia bajo
`docs/source/sdk/swift/`.

### Python
El cliente Python (`python/iroha_python/`) comparte los fixtures de Android.
Ejecutar `scripts/python_fixture_regen.sh` descarga los últimos payloads
`.norito`, refresca `python/iroha_python/tests/fixtures/`, y emitirá metadatos de
cadencia en `artifacts/python_fixture_regen_state.json` una vez que se capture la
primera rotación post-roadmap. `scripts/check_python_fixtures.py` y
`python/iroha_python/scripts/run_checks.sh` bloquean pytest, mypy, ruff y la
paridad de fixtures localmente y en CI. La documentación end-to-end
(`docs/source/sdk/python/…`) y el playbook de regeneración describen cómo
coordinar las rotaciones con los dueños de Android.

### JavaScript
`javascript/iroha_js/` no depende de archivos `.norito` locales, pero WP1-E
rastrea su evidencia de releases para que los lanes de CI de GPU hereden
provenance completo. Cada release captura provenance vía `npm run release:provenance`
(alimentado por `javascript/iroha_js/scripts/record-release-provenance.mjs`),
genera y firma bundles SBOM con `scripts/js_sbom_provenance.sh`, ejecuta el
staging firmado (`scripts/js_signed_staging.sh`) y verifica el artefacto del
registro con `javascript/iroha_js/scripts/verify-release-tarball.mjs`. Los
metadatos resultantes quedan en `artifacts/js-sdk-provenance/`,
`artifacts/js/npm_staging/`, `artifacts/js/sbom/` y `artifacts/js/verification/`,
aportando evidencia determinista para los runs de roadmap JS5/JS6 y WP1-F. El
playbook de publicación en `docs/source/sdk/js/` une toda la automatización.
