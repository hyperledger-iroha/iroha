---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Intégration des réviseurs de prévisualisation

## CV

DOCS-SORA suit un lancement progressif du portail des développeurs. Los construit avec la porte de la somme de contrôle
(`npm run serve`) et les flux Essayez-le renforcés en débloquant le hito suivant :
intégration des réviseurs validés avant que l'aperçu public soit ouvert de forme étendue. C'est un guide
décrire comme sollicitudes recopilaires, vérifier l'élégibilité, fournir un accès et dar de baja
participants de forme sûre. Consultez le
[prévisualiser le flux d'invitations](./preview-invite-flow.md) pour la planification des cohortes, la
cadence des invitations et des exportations de télémétrie ; les étapes du bas se réalisent dans les actions
à ce moment-là, un réviseur a été sélectionné.

- **Chance :** réviseurs qui nécessitent l'accès à l'aperçu des documents (`docs-preview.sora`,
  builds les pages GitHub ou les bundles de SoraFS) avant GA.
- **Fuera de alcance:** opérateurs de Torii ou SoraFS (cubiertos por sus propios kits d'embarquement)
  et despliegues del portal de produccion (ver
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Rôles et prérequis| Rôle | Objets typiques | Artefacts requis | Notes |
| --- | --- | --- | --- |
| Responsable du noyau | Vérifiez de nouvelles directives, effectuez des tests de fumée. | Poignée GitHub, contact Matrix, CLA confirmé dans l'archive. | Habituellement, vous êtes dans l'équipe GitHub `docs-preview` ; j'ai également enregistré une sollicitude pour que l'accès à la mer soit vérifiable. |
| Réviseur partenaire | Valider les extraits du SDK ou le contenu de la gestion avant la sortie publique. | Email corporativo, POC legal, terminos de preview firmados. | Vous devez reconnaître les exigences de télémétrie + gestion des données. |
| Bénévole communautaire | Apporter des commentaires sur l'utilisabilité à propos de guides. | Poignée GitHub, contact préféré, horaire horaire, acceptation du CoC. | Mantener cohortes pequenas; prioriser les réviseurs qui ont confirmé l'acuerdo de contribution. |

Tous les types de réviseurs doivent :

1. Reconnaître la politique d'utilisation acceptable pour les artefacts de prévisualisation.
2. Lire les annexes de sécurité/observabilité
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Accepter d'exécuter `docs/portal/scripts/preview_verify.sh` avant de servir n'importe qui
   instantané localement.

## Flux d'admission1. Demandez au solliciteur qui complète le
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulario (o copier/pegar en una issue). Capturer le moins : identité, méthode de contact,
   Poignée GitHub, demandes de révision anticipées et confirmation que les documents de sécurité ont été perdus.
2. Enregistrer la sollicitude sur le tracker `docs-preview` (issue de GitHub ou ticket de gouvernement)
   et assigner un demandeur.
3. Conditions préalables à la validation :
   - CLA / acuerdo de contribution en archivo (ou référence de contrato partenaire).
   - Reconocimiento de uso acceptable almacenado en la sollicitud.
   - Evaluación de riesgo completa (par exemple, réviseurs partenaires approuvés par Legal).
4. El arobador firma en la sollicitud y enlaza la question de tracking con cualquier
   entrée de gestion du changement (exemple : `DOCS-SORA-Preview-####`).

## Provisionamiento et herramientas

1. **Comparer les artefacts** - Proposer le descripteur + le fichier d'aperçu le plus récent à partir de maintenant
   le workflow de CI ou la broche de SoraFS (artefact `docs-portal-preview`). Enregistrer les réviseurs
   exécuter:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir avec application de la somme de contrôle** - Indiquer aux réviseurs la commande avec la porte de la somme de contrôle :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Ceci réutilise `scripts/serve-verified-preview.mjs` pour ne pas lancer une build sans vérifier
   par accident.3. **Accéder à GitHub (facultatif)** - Si les réviseurs ont besoin d'être publiés, ajoutés
   sur l'équipe GitHub `docs-preview` pendant la révision et l'enregistrement du changement de membre dans la sollicitude.

4. **Comunicar canales de soporte** - Partager le contact d'astreinte (Matrix/Slack) et la procédure
   de incidents de [`incident-runbooks`](./incident-runbooks.md).

5. **Télémétrie + feedback** - Enregistrer les réviseurs qui se copient de manière anonymisée
   (version [`observability`](./observability.md)). Procéder au formulaire de feedback ou à la plante
   de issue referenciada en la invitación y registrar el evento con el helper
   [`preview-feedback-log`](./preview-feedback-log) pour que le CV soit maintenu dans le jour.

## Liste de contrôle du réviseur

Avant d’accéder à l’aperçu, les réviseurs doivent compléter le suivant :

1. Vérifiez les objets téléchargés (`preview_verify.sh`).
2. Lancez le portail via `npm run serve` (ou `serve:verified`) pour vous assurer que la garde de somme de contrôle est activée.
3. Lisez les notes de sécurité et d’observation en place.
4. Essayez la console OAuth/Try it en utilisant le code de l'appareil (si application) et évitez de réutiliser les jetons de production.
5. Registrar hallazgos en el tracker acordado (issue, doc compartimento o formulario) et étiquettetarlos
   avec la balise de sortie de l'aperçu.

## Responsabilités des mainteneurs et des départs| Phase | Accions |
| --- | --- |
| Coup d'envoi | Confirmez que la liste de contrôle d'admission est complémentaire à la demande, partagez les objets + les instructions, ajoutez une entrée `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), et programmez une synchronisation d'une période si la révision dure plus d'une semaine. |
| Moniteuréo | Surveillez la télémétrie de prévisualisation (buscar traffico Essayez-le de manière habituelle, erreurs de sonde) et suivez le runbook des incidents si vous survenez quelqu'un d'autre. Les événements du registraire `feedback-submitted`/`issue-opened` sont conformes aux règles pour que les mesures de l'ola soient précises. |
| Débarquement | Révoquez l'accès temporel de GitHub ou SoraFS, le registraire `access-revoked`, archivez la sollicitude (y compris le résumé des commentaires + les actions pendantes) et actualisez le registre des réviseurs. Demandez au réviseur d'éliminer les builds locales et d'ajouter le résumé généré à partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilisez le même processus lors de la rotation des réviseurs entre les heures. Mantener el rastro en el repo (issue + plantillas) ayuda
a que DOCS-SORA est si vérifiable et permet de confirmer que l'accès à l'aperçu est activé
les contrôles documentés.

## Plantes d'invitation et de suivi- Commencer tout ce qui touche à la sensibilisation
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  archives. Capturez le langage légal au minimum, les instructions de contrôle de la somme de contrôle et l'attente
  de sorte que les réviseurs reconnaissent que la politique d'utilisation est acceptable.
- Lors de l'édition de la plante, remplacez les espaces réservés pour `<preview_tag>`, `<request_ticket>` et les canaux
  de contact. Garder une copie du message final sur le ticket d'admission pour les réviseurs, arobadores
  et les auditeurs peuvent référencer le texte exact qui est envoyé.
- Après avoir envoyé l'invitation, actualisez la date de suivi ou d'émission avec l'horodatage `invite_sent_at`.
  et la fecha espérée de cierre pour que le rapport de
  [prévisualiser le flux d'invitation](./preview-invite-flow.md) peut détecter automatiquement la cohorte.