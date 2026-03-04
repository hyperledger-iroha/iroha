---
lang: es
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Incorporación de lectores de vista previa

## Vista del conjunto

DOCS-SORA es adecuado para un lanzamiento por etapas del desarrollador minorista. Les builds avec gate de checksum
(`npm run serve`) et les flux Try it durcis debloquent le prochain jalon:
La incorporación de rectores verifica antes de que la vista previa pública no tenga mayor tamaño. guía ce
Decrit Comment Collector les demandes, verifier l'eligibilite, provisionner l'acces et offboarder
les participantes en securité. Reportez-vous au
[vista previa del flujo de invitación](./preview-invite-flow.md) para la planificación de cohortes, la cadencia
d'invitación y exportaciones de telemetría; les etapas ci-dessous se concentran sur les acciones
a prendre une fois qu'un relecteur a ete selectne.

- **Perímetro:** lectores que necesitan acceso a los documentos de vista previa (`docs-preview.sora`,
  construye páginas de GitHub o paquetes SoraFS) antes de GA.
- **Hors perimetre:** operadores Torii o SoraFS (cubiertos por sus propios kits de incorporación)
  et deploiements de portail en producción (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Roles y requisitos previos| Rol | Objetos típicos | Requisitos de artefactos | Notas |
| --- | --- | --- | --- |
| Mantenedor principal | Verificador de nuevas guías, ejecutor de pruebas de humo. | Maneje GitHub, comuníquese con Matrix, firmante de CLA en el expediente. | Recuerdo dejar en el equipo GitHub `docs-preview`; Deposer quand meme une demande pour que l'acces soit auditable. |
| Revisor socio | Valide los fragmentos del SDK o el contenido de gobernanza antes de su publicación. | Correo electrónico corporativo, POC legal, términos de vista previa de firmas. | Doit reconnaitre les exigences de telemetrie +treatment des donnees. |
| Voluntario comunitario | Fournir du feedback d'utilisabilite sur les guías. | Manejar GitHub, contacto preferido, fusible horario, aceptación del CoC. | Garder les cohortes petites; Prioriser les relecteurs ayant signe l'accord de contribuido. |

Todos los tipos de revisores hacen:

1. Reconozca la política de uso aceptable para los artefactos de vista previa.
2. Leer los anexos de seguridad/observabilidad
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. S'engager a ejecutante `docs/portal/scripts/preview_verify.sh` antes de servir un
   ubicación de la instantánea.

## Flujo de trabajo de admisión1. Demander au demandeur de remplir le
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (ou le copier/coller dans une issues). Capturador mínimo: identite, moyen de contact,
   manejar GitHub, fechas anteriores a la revisión y confirmación de los documentos de seguridad en estos lues.
2. Registrar la demanda en el rastreador `docs-preview` (emitir GitHub o ticket de gobierno)
   et asignador de un aprobador.
3. Verificador de requisitos previos:
   - CLA / acuerdo de contribución al expediente (ou referencia de socio contratante).
   - Accuse d'usage aceptable stocke dans la demande.
   - Evaluación de riesgos terminados (ejemplo: el socio revisor aprueba por parte Legal).
4. L'approbateur signe la demande et lie l'issue de suivi a toute entree de change-management
   (ejemplo: `DOCS-SORA-Preview-####`).

## Aprovisionamiento y desabastecimiento

1. **Partager les artefactos** - Fournir le último descriptor + archivo de vista previa después
   el flujo de trabajo CI o el pin SoraFS (artefacto `docs-portal-preview`). Revisores de rappeler aux
   ejecutor:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir con cumplimiento de suma de verificación** - Orientar a los revisores hacia la puerta de enlace:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Cela reutiliza `scripts/serve-verified-preview.mjs` para que la compilación no se verifique
   ne soit lance par accident.3. **Donner l'acces GitHub (opcional)** - Si los revisores acceden a sucursales no publicadas,
   Les agregan al equipo GitHub `docs-preview` para la duración de la revista y consignan el cambio.
   de membresía en la demanda.

4. **Comunicar los canales de soporte** - Compartir el contacto de guardia (Matrix/Slack) y la
   procedimiento de incidente de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetría + comentarios** - Rappeler aux revisores que des análisis anónimos son recopilados
   (ver [`observability`](./observability.md)). Introduzca el formulario de comentarios o la plantilla
   d'issuemenne dans l'invitation et periodiser l'evenement avec l'helper
   [`preview-feedback-log`](./preview-feedback-log) para que el resumen vago reste un día.

## Lista de verificación del revisor

Antes de acceder a la vista previa, los revisores no deben completar:

1. Verificador de telecargas de artefactos (`preview_verify.sh`).
2. Abra el portal a través de `npm run serve` (o `serve:verified`) para asegurarse de que la suma de comprobación de seguridad esté activa.
3. Lire les notes securite et observabilite referencees ci-dessus.
4. Pruebe la consola OAuth/Pruébelo mediante el inicio de sesión con el código del dispositivo (si corresponde) y evite reutilizar los tokens de producción.
5. Deposer les constats dans le tracker convenu (issue, doc partage ou formuleire) et les tagger
   con la etiqueta de vista previa del lanzamiento.

## Responsabilidades de los mantenedores y baja| Fase | Acciones |
| --- | --- |
| Inicio | Confirme que la lista de verificación de admisión está conjuntada con la demanda, comparta los artefactos + instrucciones, agregue una entrada `invite-sent` a través de [`preview-feedback-log`](./preview-feedback-log), y planifique un punto de recorrido si la revisión dura más de una semana. |
| Monitoreo | Vigile la vista previa de la telemetría (tráfico Pruébelo habitualmente, echec de probe) y siga el runbook d'incident si el que eligió parait sospechoso. Journaliser les eventements `feedback-submitted`/`issue-opened` au fur et a mesure que les constats lending pour que les metrics de vague restent exactitudes. |
| Baja de embarque | Revoque el acceso temporal a GitHub o SoraFS, remitente `access-revoked`, archive la demanda (incluya currículum de comentarios + acciones en atención), y comience a registrar revisores en un día. Demander au reviewer de purger les builds locaux et joindre le digest genere a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilice el proceso de memes durante la rotación de revisores entre vagos. Garder la trace dans le repo
(edición + plantillas) ayuda DOCS-SORA a rester auditable et permet a la gouvernance de confirmer que l'acces
Obtenga una vista previa después de los controles documentados.

## Plantillas de invitación y seguimiento- Commencer chaque outreach avec le
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  archivo. Captura el idioma mínimo legal, las instrucciones de suma de verificación, vista previa y atención
  que los revisores reconozcan la política de uso aceptable.
- Después de la edición de la plantilla, reemplace los marcadores de posición por `<preview_tag>`, `<request_ticket>`.
  et les canaux de contact. Stocker una copia del mensaje final en el ticket de admisión para los revisores,
  approbateurs et auditeurs puissent referencer le texte exactitud enviado.
- Después del envío de la invitación, agregue cada día la hoja de cálculo o la emisión con la marca de tiempo.
  `invite_sent_at` y la fecha de fin de asistencia para que la relación
  [vista previa del flujo de invitación](./preview-invite-flow.md) puisse capturer la cohorte automatiquement.