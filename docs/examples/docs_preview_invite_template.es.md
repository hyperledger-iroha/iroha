---
lang: es
direction: ltr
source: docs/examples/docs_preview_invite_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6c819c8d2a9517f1235a66a4661efd061a166ea89c953fd599e102b3cfd9157b
source_last_modified: "2025-11-10T18:08:48.050596+00:00"
translation_last_reviewed: 2026-01-01
---

# Invitacion a vista previa del portal de docs (Plantilla)

Usa esta plantilla cuando envies instrucciones de acceso de preview a revisores. Reemplaza
los placeholders (`<...>`) con los valores relevantes, adjunta los artefactos de descriptor +
archivo mencionados en el mensaje, y guarda el texto final dentro del ticket de intake
correspondiente.

```text
Asunto: [DOCS-SORA] invitacion de vista previa del portal de docs <preview_tag> para <reviewer/org>

Hola <name>,

Gracias por ofrecerte a revisar el portal de docs antes de GA. Estas aprobado
para la ola <wave_id>. Sigue los pasos abajo antes de explorar la vista previa:

1. Descarga los artefactos verificados desde CI o SoraFS:
   - Descriptor: <descriptor_url> (`sha256:<descriptor_sha256>`)
   - Archivo: <archive_url> (`sha256:<archive_sha256>`)
2. Ejecuta el gate de checksum:

   ./docs/portal/scripts/preview_verify.sh      --descriptor <path-to-descriptor>      --archive <path-to-archive>      --build-dir <path-to-extracted-build>

3. Sirve la vista previa con enforcement de checksum habilitado:

   DOCS_RELEASE_TAG=<preview_tag> npm run --prefix docs/portal serve

4. Lee las notas de uso aceptable, seguridad y observabilidad:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Envia feedback via <request_ticket> y marca cada hallazgo con `<preview_tag>`.

Soporte disponible en <contact_channel>. Incidentes o temas de seguridad deben ser
reportados de inmediato via <incident_channel>. Si necesitas tokens de la API Torii,
solicitalos por el ticket; nunca reutilices credenciales de produccion.

El acceso de vista previa expira el <end_date> salvo extension por escrito. Registramos
checksums y metadatos de la invitacion para governance; avisanos cuando termines
para poder retirarte de manera limpia.

Gracias de nuevo por ayudar a estabilizar el portal!

- Equipo DOCS-SORA
```
