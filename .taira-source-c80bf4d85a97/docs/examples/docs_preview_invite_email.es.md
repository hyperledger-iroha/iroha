---
lang: es
direction: ltr
source: docs/examples/docs_preview_invite_email.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4e3856058310e40649d5394996b2bcbfde99effb9e706be87f284e1812d5bdbd
source_last_modified: "2025-11-15T04:49:30.881970+00:00"
translation_last_reviewed: 2026-01-01
---

# Invitacion a vista previa del portal de docs (Email de muestra)

Usa este ejemplo al redactar el mensaje de salida. Captura el texto exacto enviado a los
revisores de la comunidad W2 (`preview-2025-06-15`) para que olas futuras puedan imitar el
tono, la guia de verificacion y el rastro de evidencia sin tener que reconstruir tickets
antiguos. Actualiza los enlaces de artefactos, hashes, IDs de solicitud y fechas antes de
enviar una nueva invitacion.

```text
Asunto: [DOCS-SORA] invitacion de vista previa del portal de docs preview-2025-06-15 para Horizon Wallet

Hola Sam,

Gracias de nuevo por ofrecer Horizon Wallet para la preview comunitaria W2. La ola
W2 ya esta aprobada, asi que puedes iniciar tu revision tan pronto completes los
pasos abajo. Manten los artefactos y tokens de acceso privados: cada invitacion
queda registrada en DOCS-SORA-Preview-W2 y los auditores revisaran los acuses.

1. Descarga los artefactos verificados (los mismos bits que enviamos a SoraFS y CI):
   - Descriptor: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/descriptor.json (`sha256:a1f41cfb02a5f34f2a0e6535f0b079dbb645c1b5dcdbcb36f953ef5c418260ad`)
   - Archive: https://sorafs-gateway.sora/docs-preview/preview-2025-06-15/docs-portal-preview.tar.zst (`sha256:5bc30261fa3c0db032ac2b3c4b56651bebcd309d69a2634ebc9a6f0da3435399`)
2. Verifica el bundle antes de extraer:

   ./docs/portal/scripts/preview_verify.sh      --descriptor ~/Downloads/descriptor.json      --archive ~/Downloads/docs-portal-preview.tar.zst      --build-dir ~/sora-docs/preview-2025-06-15

3. Sirve la vista previa con enforcement de checksum:

   DOCS_RELEASE_TAG=preview-2025-06-15 npm run --prefix docs/portal serve

4. Revisa los runbooks endurecidos antes de probar:
   - docs/portal/docs/devportal/security-hardening.md
   - docs/portal/docs/devportal/observability.md
   - docs/portal/docs/devportal/reviewer-onboarding.md

5. Envia feedback via DOCS-SORA-Preview-REQ-C04 y etiqueta cada hallazgo con
   `docs-preview/w2`. Usa el formulario de feedback si prefieres un intake estructurado:
   docs/examples/docs_preview_feedback_form.md.

Soporte disponible en Matrix (`#docs-preview:matrix.org`) y tendremos office hours
el 2025-06-18 15:00 UTC. Para escalaciones de seguridad o incidentes, pagina al
alias on-call de docs via ops@sora.org o +1-555-0109 de inmediato; no esperes a
office hours.

El acceso de vista previa para Horizon Wallet corre 2025-06-15 -> 2025-06-29. Avisanos
en cuanto termines para revocar las llaves de acceso temporales y registrar el cierre
en el tracker.

Gracias por ayudar a llevar el portal a GA!

- Equipo DOCS-SORA
```
