---
lang: es
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo_2026-02.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: eb92b77765ced36213a0bde55581f29d59c262f398c658f35a1fb43a182fe296
source_last_modified: "2026-01-03T18:07:59.201100+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Memorándum de aprobación legal de la UE AND6: 2026.1 GA (SDK de Android)

## Resumen

- **Lanzamiento/Entrenamiento:** 2026.1 GA (SDK de Android)
- **Fecha de revisión:** 2026-04-15
- **Abogada/Revisora:** Sofia Martins — Cumplimiento y Asuntos Legales
- **Alcance:** Objetivo de seguridad ETSI EN 319 401, resumen GDPR DPIA, certificación SBOM, evidencia de contingencia entre dispositivo y laboratorio AND6
- **Tickets asociados:** `_android-device-lab` / AND6-DR-202602, rastreador de gobernanza AND6 (`GOV-AND6-2026Q1`)

## Lista de verificación de artefactos

| Artefacto | SHA-256 | Ubicación / Enlace | Notas |
|----------|---------|-----------------|-------|
| `security_target.md` | `385d17a55579d2b0b365e21090ee081ded79e44655690b2abfbf54068c9b55b0` | `docs/source/compliance/android/eu/security_target.md` | Coincide con los identificadores de versión 2026.1 GA y los deltas de modelos de amenazas (adiciones Torii NRPC). |
| `gdpr_dpia_summary.md` | `8ef338a20104dc5d15094e28a1332a604b68bdcfef1ff82fea784d43fdbd10b5` | `docs/source/compliance/android/eu/gdpr_dpia_summary.md` | Referencias a la política de telemetría AND7 (`docs/source/sdk/android/telemetry_redaction.md`). |
| `sbom_attestation.md` | `c2e0de176d4bb8c8e09329e2b9ee5dd93228d3f0def78225c1d8b777a5613f2d` | Paquete `docs/source/compliance/android/eu/sbom_attestation.md` + Sigstore (`android-sdk-release#4821`). | Revisión de procedencia de CycloneDX +; coincide con el trabajo de Buildkite `android-sdk-release#4821`. |
| Registro de pruebas | `0b2d2f9eddada06faa70620f608c3ad1ec38f378d2cbddc24b15d0a83fcc381d` | `docs/source/compliance/android/evidence_log.csv` (fila `android-device-lab-failover-20260220`) | Confirma los hashes del paquete de registros capturados + la instantánea de capacidad + la entrada de notas. |
| Paquete de contingencia dispositivo-laboratorio | `faf32356dfc0bbca1459b14d75f3306ea1c10cb40f3180fe1758ac5105016f85` | `artifacts/android/device_lab_contingency/20260220-failover-drill/` | Hash tomado de `bundle-manifest.json`; El ticket AND6-DR-202602 registró la transferencia a Legal/Cumplimiento. |

## Hallazgos y excepciones

- No se identificaron problemas de bloqueo. Los artefactos se alinean con los requisitos de ETSI/GDPR; La paridad de telemetría AND7 se indica en el resumen de DPIA y no se requieren mitigaciones adicionales.
- Recomendación: monitorear el simulacro DR-2026-05-Q2 programado (ticket AND6-DR-202605) y agregar el paquete resultante al registro de evidencia antes del siguiente punto de control de gobernanza.

## Aprobación

- **Decisión:** Aprobada
- **Firma/Marca de tiempo:** _Sofia Martins (firmada digitalmente a través del portal de gobernanza, 2026-04-15 14:32 UTC)_
- **Propietarios de seguimiento:** Device Lab Ops (entregue el paquete de evidencia DR-2026-05-Q2 antes del 2026-05-31)