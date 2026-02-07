---
lang: es
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Vista previa pública del libro de jugadas de invitación

## Objetivos del programa

Ce playbook comentario explique annoncer et faire tourner le previa public une fois que le
El flujo de trabajo de incorporación de revisores está activo. Guarde la hoja de ruta DOCS-SORA honnete es
s'assurant que cada invitación parte avec des artefactos verificables, des consignas de seguridad
et un chemin clair pour le feedback.

- **Audiencia:** liste curae de membres de la communaute, partners et mantenedores qui ont
  Firme la política de uso aceptable de la vista previa.
- **Plafones:** taille de vague par defaut <= 25 reseñas, acceso de 14 días,
  respuesta aux incidencias sous 24 h.

## Lista de verificación de puerta de lanzamiento

Terminez ces taches avant d'envoyer una invitación:

1. Derniers artefactos de cargos previos en CI (`docs-portal-preview`,
   manifiesto de suma de comprobación, descriptor, paquete SoraFS).
2. `npm run --prefix docs/portal serve` (puerta por suma de comprobación) prueba la etiqueta meme.
3. Los boletos de incorporación de los revisores aprueban y se encuentran a la vaga invitación.
4. Documentos de seguridad, observabilité e incidentes válidos
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulario de comentarios o plantilla de preparación de emisión (incluye los campos de gravedad,
   etapas de reproducción, capturas de pantalla e información medioambiental).
6. Copia del anuncio de revisión de Docs/DevRel + Governance.## Paquete de invitación

Cada invitación incluye:

1. **Artefactos verifica** - Fournir les gravámenes vers le manifest/plan SoraFS ou les artefactos
   GitHub, además del manifiesto de suma de comprobación y el descriptor. Referencia explícita a la orden
   de verificación para que los revisores puedan ejecutar el ejecutor antes de iniciar el sitio.
2. **Instrucciones de servicio** - Incluir la puerta de vista previa del comando por suma de verificación:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels securite** - Indica que los tokens expiran automáticamente, que los gravámenes
   ne doivent pas etre partages, et que les incidents doivent etre signales inmediatamente.
4. **Canal de retroalimentación** - Lier the template/formulaire and clarifier les attentes de temps de reponse.
5. **Dates du program** - Seleccione las fechas de debut/fin, horarios de oficina o sincronizaciones y la cadena
   ventana de refresco.

El ejemplo de correo electrónico en
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
cubra estas exigencias. Mettre a jour les placeholders (fechas, URL, contactos)
antes del envío.

## Vista previa del hotel Exposer

Ne promouvoir l'hote vista previa qu'une fois l'onboarding termine et le ticket de changement approuve.
Vea la [guía de exposición del hotel vista previa](./preview-host-exposure.md) para las etapas de un extremo a otro
Los usuarios de compilación/publicación/verificación en esta sección.

1. **Construcción y embalaje:** Marque la etiqueta de liberación y produzca los artefactos determinados.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```El script de pin ecrit `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   y `portal.dns-cutover.json` en `artifacts/sorafs/`. Joindre ces fichiers a la vague
   Invitación para que cada revisor pueda verificar los memes bits.

2. **Vista previa del alias del editor:** Relancer el comando sin `--skip-submit`
   (fournir `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y la preuve d'alias emise
   par la gobernancia). El script se encuentra en el manifiesto `docs-preview.sora` y emet
   `portal.manifest.submit.summary.json` plus `portal.pin.report.json` para el paquete de preuves.

3. **Probar la implementación:** Confirme que el alias tiene resultados y que la suma de comprobación corresponde a la etiqueta
   avant d'envoyer les invitaciones.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) sous la main como respaldo para
   Para que los revisores puedan realizar una copia local si la vista previa del borde está abierta.

## Cronograma de comunicación| Diario | Acción | Propietario |
| --- | --- | --- |
| D-3 | Finalizar la copia de invitación, sacar los artefactos, realizar un simulacro de verificación | Documentos/DevRel |
| D-2 | Gobernanza de aprobación + cambio de ticket | Documentos/DevRel + Gobernanza |
| D-1 | Envíe las invitaciones a través de la plantilla, realice un seguimiento diario con la lista de destinos | Documentos/DevRel |
| D | Llamada inicial/horario de oficina, monitor de paneles de telemetría | Documentos/DevRel + De guardia |
| D+7 | Digest de feedback a mi-vague, triage des issues bloquantes | Documentos/DevRel |
| D+14 | Clore la vague, revoquer l'acces temporaire, publier un resume dans `status.md` | Documentos/DevRel |

## Suivi d'acces et telemetrie

1. Registrar cada destino, marca de tiempo de invitación y fecha de revocación con le
   registrador de comentarios de vista previa (ver
   [`preview-feedback-log`](./preview-feedback-log)) afin que chaque vague partage la meme
   rastro de preuves:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Los eventos cargados son `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened` y `access-revoked`. Le log se trouve a
   `artifacts/docs_portal_preview/feedback_log.json` por defecto; joignez-le au ticket de
   vague d'invitation avec les formulaires de consentement. Utilice el ayudante de resumen
   Para producir un roll-up auditable antes de la nota de cierre:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```El resumen JSON enumera las invitaciones vagas, los destinos abiertos, los
   contadores de retroalimentación y la marca de tiempo del último evento. El ayudante de reposo sobre
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Donc le meme flowflow puede girar localmente o en CI. Utilizar la plantilla de resumen
   en [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   Lors de la publicación du recap de la vague.
2. Etiquetar los paneles de telemetría con el `DOCS_RELEASE_TAG` utilizar para la vaga después que
   les pics puissent etre correles aux cohortes d'invitation.
3. Ejecutor `npm run probe:portal -- --expect-release=<tag>` después del despliegue para confirmar que
   La vista previa del entorno anuncia los buenos metadatos de lanzamiento.
4. Consigner tout incident dans le template de runbook et le lier a la cohorte.

## Comentarios y cierre1. Agregue sus comentarios en un documento compartido o en una junta de temas. Etiquetar elementos con
   `docs-preview/<wave>` para que los propietarios de la hoja de ruta puedan retroceder fácilmente.
2. Utilice la salida de resumen del registrador de vista previa para completar la relación vaga, luego
   resumer la cohorte dans `status.md` (participantes, principaux constats, fixes prevus) et
   Mettre a jour `roadmap.md` si le jalon DOCS-SORA un cambio.
3. Suivre les etapes d'offboarding depuis
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoquer l'acces, archiver les demandes et
   remercier les participantes.
4. Preparer la prochaine vague en rafraichissant les artefactos, en relancant les gates de checksum
   et en mettant a jour le template d'invitation avec de nouvelles date.

Aplicar este libro de jugadas de facon consistente garde le programa de vista previa auditable et donne a
Docs/DevRel es un medio repetible de hacer grandes invitaciones a medida que el portal se acerca a GA.