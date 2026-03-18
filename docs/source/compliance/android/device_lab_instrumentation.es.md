---
lang: es
direction: ltr
source: docs/source/compliance/android/device_lab_instrumentation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9d384e21d09f3c4f57b7fc5181d69dc0da739dd6ed4dcb89a57ea58fd29bb898
source_last_modified: "2026-01-03T18:07:59.259775+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Ganchos de instrumentación de laboratorio para dispositivos Android (AND6)

Esta referencia cierra la acción de la hoja de ruta “preparar el dispositivo-laboratorio restante /
ganchos de instrumentación antes del inicio de AND6”. Explica cómo cada reservado
La ranura del laboratorio del dispositivo debe capturar artefactos de telemetría, cola y certificación para que el
La lista de verificación de cumplimiento de AND6, el registro de evidencia y los paquetes de gobernanza comparten lo mismo
flujo de trabajo determinista. Vincula esta nota con el procedimiento de reserva
(`device_lab_reservation.md`) y el runbook de conmutación por error al planificar los ensayos.

## Objetivos y alcance

- **Evidencia determinista**: todas las salidas de instrumentación se encuentran bajo
  `artifacts/android/device_lab/<slot-id>/` con manifiestos SHA-256 para que los auditores
  puede diferenciar paquetes sin volver a ejecutar las sondas.
- **Flujo de trabajo basado en secuencias de comandos**: reutilice los asistentes existentes
  (`ci/run_android_telemetry_chaos_prep.sh`,
  `scripts/android_keystore_attestation.sh`, `scripts/android_override_tool.sh`)
  en lugar de comandos adb personalizados.
- **Las listas de verificación permanecen sincronizadas**: cada ejecución hace referencia a este documento del
  Lista de verificación de cumplimiento AND6 y adjunta los artefactos a
  `docs/source/compliance/android/evidence_log.csv`.

## Diseño de artefacto

1. Elija un identificador de espacio único que coincida con el boleto de reserva, p.
   `2026-05-12-slot-a`.
2. Siembra los directorios estándar:

   ```bash
   export ANDROID_DEVICE_LAB_SLOT=2026-05-12-slot-a
   export ANDROID_DEVICE_LAB_ROOT="artifacts/android/device_lab/${ANDROID_DEVICE_LAB_SLOT}"
   mkdir -p "${ANDROID_DEVICE_LAB_ROOT}"/{telemetry,attestation,queue,logs}
   ```

3. Guarde cada registro de comando dentro de la carpeta correspondiente (p. ej.
   `telemetry/status.ndjson`, `attestation/pixel8pro.log`).
4. Capture los manifiestos SHA-256 una vez que se cierre la ranura:

   ```bash
   find "${ANDROID_DEVICE_LAB_ROOT}" -type f -print0 | sort -z \
     | xargs -0 shasum -a 256 > "${ANDROID_DEVICE_LAB_ROOT}/sha256sum.txt"
   ```

## Matriz de instrumentación

| Flujo | Comando(s) | Ubicación de salida | Notas |
|------|------------|-----------------|-------|
| Redacción de telemetría + paquete de estado | `scripts/telemetry/check_redaction_status.py --status-url <collector> --json-out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/status.ndjson` | `telemetry/status.ndjson`, `telemetry/status.log` | Ejecute al principio y al final de la ranura; adjunte la salida estándar CLI a `status.log`. |
| Cola pendiente + preparación del caos | `ANDROID_PENDING_QUEUE_EXPORTS="pixel8=${ANDROID_DEVICE_LAB_ROOT}/queue/pixel8.bin" ci/run_android_telemetry_chaos_prep.sh --status-only` | `queue/*.bin`, `queue/*.json`, `queue/*.sha256` | Espejos ScenarioD de `readiness/labs/telemetry_lab_01.md`; extienda la var env para cada dispositivo en la ranura. |
| Anular el resumen del libro mayor | `scripts/android_override_tool.sh digest --out ${ANDROID_DEVICE_LAB_ROOT}/telemetry/override_digest.json` | `telemetry/override_digest.json` | Se requiere incluso cuando no hay anulaciones activas; demostrar el estado cero. |
| Certificación StrongBox / TEE | `scripts/android_keystore_attestation.sh --device pixel8pro-strongbox-a --out "${ANDROID_DEVICE_LAB_ROOT}/attestation/pixel8pro"` | `attestation/<device>/*.{json,zip,log}` | Repita para cada dispositivo reservado (coincida con los nombres en `android_strongbox_device_matrix.md`). |
| Regresión de certificación de arnés de CI | `scripts/android_strongbox_attestation_ci.sh --output "${ANDROID_DEVICE_LAB_ROOT}/attestation/ci"` | `attestation/ci/*` | Captura la misma evidencia que carga CI; Incluir en ejecuciones manuales para simetría. |
| Línea base de pelusa/dependencia | `ANDROID_LINT_SUMMARY_OUT="${ANDROID_DEVICE_LAB_ROOT}/logs/jdeps-summary.txt" make android-lint` | `logs/jdeps-summary.txt`, `logs/lint.log` | Ejecutar una vez por ventana de congelación; cite el resumen en los paquetes de cumplimiento. |

## Procedimiento de ranura estándar1. **Previo al vuelo (T-24h)** – Confirma que el billete de reserva hace referencia a este
   documento, actualice la entrada de la matriz del dispositivo y genere la raíz del artefacto.
2. **Durante el horario**
   - Primero ejecute el paquete de telemetría + comandos de exportación de cola. Pase
     `--note <ticket>` a `ci/run_android_telemetry_chaos_prep.sh` para que el registro
     hace referencia al ID del incidente.
   - Activar los scripts de atestación por dispositivo. Cuando el arnés produce un
     `.zip`, cópielo en la raíz del artefacto y registre el Git SHA impreso en
     el final del guión.
   - Ejecute `make android-lint` con la ruta de resumen anulada incluso si CI
     ya corrió; Los auditores esperan un registro por ranura.
3. **Post-ejecución**
   - Generar `sha256sum.txt` e `README.md` (notas de formato libre) dentro de la ranura
     carpeta que resume los comandos ejecutados.
   - Agregue una fila a `docs/source/compliance/android/evidence_log.csv` con el
     ID de ranura, ruta del manifiesto hash, referencias de Buildkite (si las hay) y la última versión
     porcentaje de capacidad del laboratorio del dispositivo a partir de la exportación del calendario de reservas.
   - Vincular la carpeta de slots en el ticket `_android-device-lab`, el AND6
     lista de verificación e informe de lanzamiento `docs/source/android_support_playbook.md`.

## Manejo y escalamiento de fallas

- Si algún comando falla, capture la salida stderr en `logs/` y siga las instrucciones
  escalera de ascenso en `device_lab_reservation.md` §6.
- Los déficits de cola o telemetría deben indicar inmediatamente el estado de anulación en
  `docs/source/sdk/android/telemetry_override_log.md` y haga referencia al ID de la ranura
  para que la gobernanza pueda rastrear el simulacro.
- Las regresiones de certificación deben registrarse en
  `docs/source/sdk/android/readiness/android_strongbox_attestation_bundle.md`
  con los números de serie del dispositivo defectuoso y las rutas de paquete registradas anteriormente.

## Lista de verificación de informes

Antes de marcar la ranura como completa, verifique que las siguientes referencias estén actualizadas:

- `docs/source/compliance/android/and6_compliance_checklist.md` — marcar el
  Complete la fila de instrumentación y anote el ID de la ranura.
- `docs/source/compliance/android/evidence_log.csv` — agregar/actualizar la entrada con
  el hash de la ranura y la lectura de capacidad.
- Ticket `_android-device-lab`: adjunte enlaces de artefactos e ID de trabajos de Buildkite.
- `status.md`: incluya una breve nota en el próximo resumen de preparación de Android para que
  Los lectores de la hoja de ruta saben qué espacio produjo la evidencia más reciente.

Seguir este proceso mantiene los “ganchos de instrumentación y laboratorio de dispositivos” de AND6.
hito auditable y evita la divergencia manual entre la reserva, la ejecución,
y presentación de informes.