---
lang: es
direction: ltr
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 Plantilla de memorando de aprobación legal de la UE

Este memorando registra la revisión legal requerida por el elemento de la hoja de ruta **AND6** antes de la
El paquete de productos de la UE (ETSI/GDPR) se envía a los reguladores. El abogado debería clonar
esta plantilla por versión, complete los campos a continuación y almacene la copia firmada
junto con los artefactos inmutables a los que se hace referencia en el memorando.

## Resumen

- **Liberación / Tren:** `<e.g., 2026.1 GA>`
- **Fecha de revisión:** `<YYYY-MM-DD>`
- **Abogado/Revisor:** `<name + organisation>`
- **Alcance:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **Boletos asociados:** `<governance or legal issue IDs>`

## Lista de verificación de artefactos

| Artefacto | SHA-256 | Ubicación / Enlace | Notas |
|----------|---------|-----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + archivo de gobernanza | Confirme los identificadores de versiones y los ajustes del modelo de amenazas. |
| `gdpr_dpia_summary.md` | `<hash>` | Mismo directorio/espejos de localización | Asegúrese de que las referencias de la política de redacción coincidan con `sdk/android/telemetry_redaction.md`. |
| `sbom_attestation.md` | `<hash>` | Mismo directorio + paquete de aval en el depósito de pruebas | Verifique las firmas de procedencia de CycloneDX +. |
| Fila del registro de pruebas | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | Número de fila `<n>` |
| Paquete de contingencia dispositivo-laboratorio | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | Confirma el ensayo de conmutación por error vinculado a esta versión. |

> Adjunte filas adicionales si el paquete contiene más archivos (por ejemplo, privacidad
> apéndices o traducciones EIPD). Cada artefacto debe hacer referencia a su inmutable
> cargar el objetivo y el trabajo de Buildkite que lo produjo.

## Hallazgos y excepciones

- `None.` *(Reemplazar con una lista con viñetas que cubra los riesgos residuales, compensando
  controles o acciones de seguimiento requeridas.)*

## Aprobación

- **Decisión:** `<Approved / Approved with conditions / Blocked>`
- **Firma/Marca de tiempo:** `<digital signature or email reference>`
- **Propietarios de seguimiento:** `<team + due date for any conditions>`

Cargue la nota final en el depósito de evidencia de gobernanza, copie el SHA-256 en
`docs/source/compliance/android/evidence_log.csv` y vincule la ruta de carga en
`status.md`. Si la decisión está "Bloqueada", escale a la dirección AND6.
comité y documentar los pasos de remediación tanto en la lista caliente de la hoja de ruta como en la
registro de contingencia dispositivo-laboratorio.