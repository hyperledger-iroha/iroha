---
lang: es
direction: ltr
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# FISC Security Controls Checklist — Android SDK

| Campo | Valor |
|-------|-------|
| Versión | 0.1 (2026-02-12) |
| Alcance | Herramientas de operador Android SDK + utilizadas en implementaciones financieras japonesas |
| Propietarios | Cumplimiento y Asuntos Legales (Daniel Park), líder del programa Android |

## Matriz de control

| Control FISC | Detalle de implementación | Evidencia / Referencias | Estado |
|--------------|-----------------------|-----------------------|--------|
| **Integridad de la configuración del sistema** | `ClientConfig` aplica hash de manifiesto, validación de esquema y acceso de tiempo de ejecución de solo lectura. Los errores de recarga de configuración emiten eventos `android.telemetry.config.reload` documentados en el runbook. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ Implementado |
| **Control de acceso y autenticación** | El SDK respeta las políticas TLS Torii y las solicitudes firmadas `/v1/pipeline`; referencia de flujos de trabajo del operador Guía de soporte §4–5 para escalamiento y anulación de control a través de artefactos Norito firmados. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (anular el flujo de trabajo). | ✅ Implementado |
| **Gestión de claves criptográficas** | Los proveedores preferidos de StrongBox, la validación de certificaciones y la cobertura de la matriz de dispositivos garantizan el cumplimiento de KMS. Salidas del arnés de certificación archivadas en `artifacts/android/attestation/` y rastreadas en la matriz de preparación. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ Implementado |
| **Registro, seguimiento y retención** | La política de redacción de telemetría codifica datos confidenciales, agrupa los atributos del dispositivo y aplica la retención (períodos de 7/30/90/365 días). El Manual de estrategias de soporte §8 describe los umbrales del panel; anulaciones registradas en `telemetry_override_log.md`. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ Implementado |
| **Operaciones y gestión de cambios** | El procedimiento de transición de GA (Manual de estrategias de soporte §7.2) más las actualizaciones `status.md` rastrean la preparación para el lanzamiento. Evidencia de publicación (SBOM, paquetes Sigstore) vinculada a través de `docs/source/compliance/android/eu/sbom_attestation.md`. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ Implementado |
| **Respuesta e informes de incidentes** | El libro de estrategias define la matriz de gravedad, las ventanas de respuesta del SLA y los pasos de notificación de cumplimiento; Las anulaciones de telemetría + ensayos de caos garantizan la reproducibilidad ante los pilotos. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ Implementado |
| **Residencia/localización de datos** | Los recolectores de telemetría para implementaciones de JP se ejecutan en la región aprobada de Tokio; Paquetes de atestación de StrongBox almacenados en la región y referenciados desde tickets de socios. El plan de localización garantiza que los documentos estén disponibles en japonés antes de la versión beta (AND5). | `docs/source/android_support_playbook.md` §9; `docs/source/sdk/android/developer_experience_plan.md` §5; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 En progreso (localización en curso) |

## Notas del revisor

- Verifique las entradas de la matriz de dispositivos para Galaxy S23/S24 antes de la incorporación de socios regulados (consulte las filas de documentos de preparación `s23-strongbox-a`, `s24-strongbox-a`).
- Garantizar que los recopiladores de telemetría en las implementaciones de JP apliquen la misma lógica de retención/anulación definida en la DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- Obtener la confirmación de los auditores externos una vez que los socios bancarios revisen esta lista de verificación.