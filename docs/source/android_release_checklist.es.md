---
lang: es
direction: ltr
source: docs/source/android_release_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 5ee3613b544a847953f5ec152092cb2fe1da35279c5482486513d6b8d6dddf02
source_last_modified: "2026-01-04T11:42:43.398592+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Lista de verificación de lanzamiento de Android (AND6)

Esta lista de verificación captura las puertas **AND6: CI y refuerzo de cumplimiento** de
`roadmap.md` (§Prioridad 5). Alinea las versiones del SDK de Android con Rust
liberar las expectativas de RFC detallando los trabajos de CI, los artefactos de cumplimiento,
pruebas de laboratorio del dispositivo y paquetes de procedencia que deben adjuntarse ante una AG,
LTS, o tren de revisiones, avanza.

Utilice este documento junto con:

- `docs/source/android_support_playbook.md`: calendario de lanzamientos, SLA y
  árbol de escalada.
- `docs/source/android_runbook.md`: runbooks operativos del día a día.
- `docs/source/compliance/android/and6_compliance_checklist.md` — regulador
  inventario de artefactos.
- `docs/source/release_dual_track_runbook.md`: gobernanza de lanzamiento de doble vía.

## 1. Puertas del escenario de un vistazo

| Etapa | Puertas requeridas | Evidencia |
|-------|----------------|----------|
| **T-7 días (precongelación)** | Todas las noches `ci/run_android_tests.sh` verde durante 14 días; `ci/check_android_fixtures.sh`, `ci/check_android_samples.sh` y `ci/check_android_docs_i18n.sh` pasan; Análisis de pelusa/dependencia en cola. | Paneles de Buildkite, informe de diferencias de dispositivos, capturas de pantalla de muestra. |
| **T-3 días (promoción RC)** | Reserva de laboratorio de dispositivos confirmada; Ejecución de CI de certificación StrongBox (`scripts/android_strongbox_attestation_ci.sh`); Conjuntos roboeléctricos/instrumentados ejercitados en hardware programado; `./gradlew lintRelease ktlintCheck detekt dependencyGuard` limpio. | CSV de matriz de dispositivos, manifiesto del paquete de atestación, informes de Gradle archivados en `artifacts/android/lint/<version>/`. |
| **T-1 día (va/no va)** | Paquete de estado de redacción de telemetría actualizado (`scripts/telemetry/check_redaction_status.py --write-cache`); artefactos de cumplimiento actualizados según `and6_compliance_checklist.md`; ensayo de procedencia completado (`scripts/android_sbom_provenance.sh --dry-run`). | `docs/source/compliance/android/evidence_log.csv`, estado de telemetría JSON, registro de ejecución en seco de procedencia. |
| **T0 (transición GA/LTS)** | `scripts/publish_android_sdk.sh --dry-run` completado; procedencia + SBOM firmado; lista de verificación de liberación exportada y adjunta a las actas de ir/no ir; `ci/sdk_sorafs_orchestrator.sh` trabajo de humo verde. | Libere los archivos adjuntos RFC, paquete Sigstore, artefactos de adopción en `artifacts/android/`. |
| **T+1 día (posterior a la transición)** | Preparación de revisión verificada (`scripts/publish_android_sdk.sh --validate-bundle`); diferencias del tablero revisadas (`ci/check_android_dashboard_parity.sh`); Paquete de evidencia subido a `status.md`. | Exportación de diferencias del panel, enlace a la entrada `status.md`, paquete de lanzamiento archivado. |

## 2. Matriz de puerta de calidad y CI| Puerta | Comando(s) / Script | Notas |
|------|--------------------|-------|
| Pruebas unitarias + de integración | `ci/run_android_tests.sh` (envuelve `ci/run_android_tests.sh`) | Emite `artifacts/android/tests/test-summary.json` + registro de prueba. Incluye códec Norito, cola, respaldo de StrongBox y pruebas de arnés de cliente Torii. Requerido todas las noches y antes del etiquetado. |
| Paridad de partidos | `ci/check_android_fixtures.sh` (envuelve `scripts/check_android_fixtures.py`) | Garantiza que los dispositivos Norito regenerados coincidan con el conjunto canónico de Rust; adjunte la diferencia JSON cuando falle la puerta. |
| Aplicaciones de muestra | `ci/check_android_samples.sh` | Compila `examples/android/{operator-console,retail-wallet}` y valida capturas de pantalla localizadas a través de `scripts/android_sample_localization.py`. |
| Documentos/I18N | `ci/check_android_docs_i18n.sh` | Guards README + inicios rápidos localizados. Ejecute nuevamente después de que las ediciones del documento lleguen a la rama de lanzamiento. |
| Paridad del tablero | `ci/check_android_dashboard_parity.sh` | Confirma que las métricas de CI/exportadas se alinean con las contrapartes de Rust; requerido durante la verificación T+1. |
| humo adopción SDK | `ci/sdk_sorafs_orchestrator.sh` | Ejercita los enlaces del orquestador Sorafs de múltiples fuentes con el SDK actual. Requerido antes de cargar artefactos en escena. |
| Verificación de atestación | `scripts/android_strongbox_attestation_ci.sh --summary-out artifacts/android/attestation/ci-summary.json` | Agrega los paquetes de atestación StrongBox/TEE bajo `artifacts/android/attestation/**`; adjunte el resumen a los paquetes GA. |
| Validación de ranuras de dispositivo-laboratorio | `scripts/check_android_device_lab_slot.py --root artifacts/android/device_lab/<slot> --json-out artifacts/android/device_lab/summary.json` | Valida paquetes de instrumentación antes de adjuntar evidencia a los paquetes de liberación; CI se ejecuta en la ranura de muestra en `fixtures/android/device_lab/slot-sample` (telemetría/attestation/queue/logs + `sha256sum.txt`). |

> **Consejo:** agregue estos trabajos a la canalización `android-release` Buildkite para que
> Congele las semanas y vuelva a ejecutar automáticamente cada puerta con la punta de la rama de liberación.

El trabajo consolidado `.github/workflows/android-and6.yml` ejecuta el lint,
comprobaciones de ranuras de laboratorio de dispositivos, resumen de atestación y conjunto de pruebas en cada PR/push
tocando fuentes de Android, cargando evidencia bajo `artifacts/android/{lint,tests,attestation,device_lab}/`.

## 3. Escaneos de pelusa y dependencia

Ejecute `scripts/android_lint_checks.sh --version <semver>` desde la raíz del repositorio. el
el script se ejecuta:

```
lintRelease ktlintCheck detekt dependencyGuardBaseline \
:operator-console:lintRelease :retail-wallet:lintRelease
```

- Los informes y resultados de la guardia de dependencia se archivan en
  `artifacts/android/lint/<label>/` y un enlace simbólico `latest/` para su lanzamiento
  tuberías.
- Los hallazgos fallidos de pelusa requieren reparación o una entrada en el comunicado
  RFC que documenta el riesgo aceptado (aprobado por Release Engineering + Program
  Plomo).
- `dependencyGuardBaseline` regenera el bloqueo de dependencia; adjuntar la diferencia
  al paquete pasa/no pasa.

## 4. Cobertura de laboratorio de dispositivos y caja fuerte

1. Reserve dispositivos Pixel + Galaxy utilizando el rastreador de capacidad al que se hace referencia en
   `docs/source/compliance/android/device_lab_contingency.md`. Bloquea lanzamientos
   si ` para actualizar el informe de atestación.
3. Ejecute la matriz de instrumentación (documente la lista de suite/ABI en el dispositivo
   rastreador). Capture los errores en el registro de incidentes incluso si los reintentos tienen éxito.
4. Presentar un ticket si se requiere recurrir a Firebase Test Lab; vincular el billete
   en la lista de verificación a continuación.

## 5. Artefactos de cumplimiento y telemetría- Siga `docs/source/compliance/android/and6_compliance_checklist.md` para la UE
  y presentaciones de JP. Actualización `docs/source/compliance/android/evidence_log.csv`
  con hashes + URL de trabajo de Buildkite.
- Actualizar evidencia de redacción de telemetría a través de
  `scripts/telemetry/check_redaction_status.py --write-cache \
   --status-url https://android-observability.example/status.json`.
  Guarde el JSON resultante en
  `artifacts/android/telemetry/<version>/status.json`.
- Registre la salida del esquema diff de
  `scripts/telemetry/run_schema_diff.sh --android-config ... --rust-config ...`
  para demostrar la paridad con los exportadores de Rust.

## 6. Procedencia, SBOM y publicación

1. Ejecute en seco el proceso de publicación:

   ```bash
   scripts/publish_android_sdk.sh \
     --version <semver> \
     --repo-dir artifacts/android/maven/<semver> \
     --dry-run
   ```

2. Generar procedencia SBOM + Sigstore:

   ```bash
   scripts/android_sbom_provenance.sh \
     --version <semver> \
     --out artifacts/android/provenance/<semver>
   ```

3. Adjunte `artifacts/android/provenance/<semver>/manifest.json` y firmado
   `checksums.sha256` al RFC de lanzamiento.
4. Al promocionar al repositorio real de Maven, vuelva a ejecutar
   `scripts/publish_android_sdk.sh` sin `--dry-run`, captura la consola
   log y cargue los artefactos resultantes en `artifacts/android/maven/<semver>`.

## 7. Plantilla de paquete de envío

Cada versión GA/LTS/hotfix debe incluir:

1. **Lista de verificación completa**: copie la tabla de este archivo, marque cada elemento y vincule
   a artefactos de soporte (ejecución de Buildkite, registros, diferencias de documentación).
2. **Evidencia de laboratorio del dispositivo**: resumen del informe de certificación, registro de reserva y
   cualquier activación de contingencia.
3. **Paquete de telemetría**: estado de redacción JSON, diferencia de esquema, enlace a
   Actualizaciones `docs/source/sdk/android/telemetry_redaction.md` (si las hay).
4. **Artefactos de cumplimiento**: entradas agregadas/actualizadas en la carpeta de cumplimiento
   además del registro de evidencia actualizado CSV.
5. **Paquete de procedencia**: SBOM, firma Sigstore e `checksums.sha256`.
6. **Resumen de la versión**: descripción general de una página adjunta al resumen `status.md`
   lo anterior (fecha, versión, aspectos destacados de las puertas exentas).

Guarde el paquete en `artifacts/android/releases/<version>/` y consúltelo
en `status.md` y el RFC de lanzamiento.

- `scripts/run_release_pipeline.py --publish-android-sdk ...` automáticamente
  copia el último archivo lint (`artifacts/android/lint/latest`) y el
  evidencia de cumplimiento inicie sesión en `artifacts/android/releases/<version>/` para que
  El paquete de envío siempre tiene una ubicación canónica.

---

**Recordatorio:** actualice esta lista de verificación cada vez que haya nuevos trabajos de CI, artefactos de cumplimiento,
o se agregan requisitos de telemetría. El elemento AND6 de la hoja de ruta permanece abierto hasta que
La lista de verificación y la automatización asociada resultan estables durante dos lanzamientos consecutivos.
trenes.