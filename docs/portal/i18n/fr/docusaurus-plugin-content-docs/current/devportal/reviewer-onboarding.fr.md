---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Onboarding des rélecteurs de prévisualisation

## Vue d'ensemble

DOCS-SORA suit un lancement par étapes du portail développeur. Les builds avec gate de checksum
(`npm run serve`) et les flux Try it durcis débloquent le prochain jalon :
onboarding de rélecteurs vérifie avant que la prévisualisation publique ne s'ouvre largement. Ce guide
décrit comment collecter les demandes, vérifier l'éligibilité, provisionner l'accès et offboarder
les participants en sécurité. Reportez-vous au
[preview invitation flow](./preview-invite-flow.md) pour la planification des cohortes, la cadence
d'invitation et les exportations de télémétrie; les étapes ci-dessous se concentrent sur les actions
a prendre une fois qu'un releteur a ete selectionne.

- **Périmètre :** rélecteurs qui ont besoin d'accéder aux documents en avant-première (`docs-preview.sora`,
  build GitHub Pages ou bundles SoraFS) avant GA.
- **Hors périmètre :** opérateurs Torii ou SoraFS (couverts par leurs propres kits d'onboarding)
  et déploiements de portail en production (voir
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Rôles et prérequis| Rôle | Objectifs typiques | Artefacts requis | Remarques |
| --- | --- | --- | --- |
| Responsable du noyau | Vérifier les nouveaux guides, exécuteur des smoke tests. | Gérer GitHub, contacter Matrix, CLA signé au dossier. | Souvent déjà dans l'équipe GitHub `docs-preview`; deposer quand meme une demande pour que l'acces soit auditable. |
| Réviseur partenaire | Valider des snippets SDK ou du contenu de gouvernance avant sortie publique. | Email corporate, POC légal, termes aperçu signes. | Doit reconnaître les exigences de télémétrie + traitement des données. |
| Bénévole communautaire | Fournir du feedback d'utilisabilité sur les guides. | Gérer GitHub, contact préféré, fuseau horaire, acceptation du CoC. | Garder les cohortes petites; prioriser les rélecteurs ayant signé l'accord de contribution. |

Tous les types de réviseurs doivent :

1. Reconnaitre la politique d'usage acceptable pour les artefacts de prévisualisation.
2. Lire les annexes sécurité/observabilité
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. S'engager un exécuteur `docs/portal/scripts/preview_verify.sh` avant de servir un
   instantané localement.

## Workflow d'admission1. Demander au demandeur de remplir le
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulaire (ou le copier/coller dans un numéro). Capturer au minimum: identité, moyen de contact,
   gérer GitHub, dates prévues de revue, et confirmation que les documents de sécurité ont été lues.
2. Enregistrer la demande dans le tracker `docs-preview` (issue GitHub ou ticket de gouvernance)
   et assigner un approbateur.
3. Vérifier les prérequis :
   - CLA / accord de contribution au dossier (ou référence de contrat partenaire).
   - Accusé d'usage acceptable stocke dans la demande.
   - Évaluation des risques terminés (exemple : reviewers Partner approuves par Legal).
4. L'approbateur signe la demande et lie l'issue de suivi à toute entrée de change-management
   (exemple : `DOCS-SORA-Preview-####`).

## Approvisionnement et outillage

1. **Partager les artefacts** - Fournir le dernier descripteur + archive de prévisualisation depuis
   le workflow CI ou le pin SoraFS (artefact `docs-portal-preview`). Rappeler aux reviewers
   d'exécuteur:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir avec application de checksum** - Orienter les reviewers vers la commande gatee :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Cela réutilise `scripts/serve-verified-preview.mjs` pour qu'aucun build non vérifié
   ne soit pas lancé par accident.3. **Donner l'accès à GitHub (optionnel)** - Si les reviewers ont besoin de branches non publiées,
   les ajouter à l'équipe GitHub `docs-preview` pour la durée de la revue et consigner le changement
   de l'adhésion dans la demande.

4. **Communiquer les canaux de support** - Partager le contact d'astreinte (Matrix/Slack) et la
   procédure d'incident de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetrie + feedback** - Rappeler aux reviewers que des analytiques anonymisées sont collectées
   (voir [`observability`](./observability.md)). Fournir le formulaire de feedback ou le modèle
   d'issue mentionné dans l'invitation et journaliste l'événement avec l'helper
   [`preview-feedback-log`](./preview-feedback-log) pour que le résumé de vague reste un jour.

## Checklist du réviseur

Avant d'accéder à l'aperçu, les évaluateurs doivent compléter :

1. Vérifier les artefacts télécharges (`preview_verify.sh`).
2. Lancer le portail via `npm run serve` (ou `serve:verified`) pour s'assurer que la somme de contrôle de garde est active.
3. Lire les notes de sécurité et d'observabilité références ci-dessus.
4. Testez la console OAuth/Try it via device-code login (si applicable) et évitez de réutiliser les tokens de production.
5. Déposer les constats dans le tracker convenu (issue, doc partage ou formulaire) et les tagger
   avec le tag de release preview.

## Responsabilités des mainteneurs et offboarding| Phases | Actions |
| --- | --- |
| Coup d'envoi | Confirmer que la checklist d'intake est jointe à la demande, partager les artefacts + instructions, ajouter une entrée `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), et planifier un point mi-parcours si la revue dure plus d'une semaine. |
| Surveillance | Surveiller la télémétrie aperçu (trafic Try it inhabituel, echec de sonde) et suivre le runbook d'incident si quelque chose parait suspect. Journaliser les événements `feedback-submitted`/`issue-opened` au fur et à mesure que les constats arrivent pour que les métriques de vague restent exactes. |
| Débarquement | Revoquer l'accès temporaire GitHub ou SoraFS, consigner `access-revoked`, archiver la demande (inclure CV de feedback + actions en attente), et mettre à jour le registre des évaluateurs. Demander au reviewer de purger les builds locaux et joindre le digest générique à partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilisez le meme processus lors de la rotation des reviewers entre vagues. Garder la trace dans le repo
(issue + templates) aide DOCS-SORA à rester auditable et permet a la gouvernance de confirmer que l'accès
prévisualiser un suivi des documents de contrôle.

## Modèles d'invitation et de suivi- Commencer chaque sensibilisation avec le
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  fichier. Il capture le minimum de langage légal, les instructions de checksum preview et l'attente
  que les reviewers recommandent la politique d'usage acceptable.
- Lors de l'édition du template, remplacer les placeholders pour `<preview_tag>`, `<request_ticket>`
  et les canaux de contact. Stocker une copie du message final dans le ticket d'intake pour que reviewers,
  les approbateurs et auditeurs peuvent référencer le texte exact envoyé.
- Après l'envoi de l'invitation, mettre à jour le tableur de suivi ou l'émission avec le timestamp
  `invite_sent_at` et la date de fin attendue pour que le rapport
  [preview invitation flow](./preview-invite-flow.md) peut capturer la cohorte automatiquement.