---
lang: he
direction: rtl
source: docs/portal/docs/devportal/reviewer-onboarding.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Onboarding des relecteurs de preview

## Vue d'ensemble

DOCS-SORA suit un lancement par etapes du portail developpeur. Les builds avec gate de checksum
(`npm run serve`) et les flux Try it durcis debloquent le prochain jalon:
onboarding de relecteurs verifies avant que la preview publique ne s'ouvre largement. Ce guide
decrit comment collecter les demandes, verifier l'eligibilite, provisionner l'acces et offboarder
les participants en securite. Reportez-vous au
[preview invite flow](./preview-invite-flow.md) pour la planification des cohortes, la cadence
d'invitation et les exports de telemetrie; les etapes ci-dessous se concentrent sur les actions
a prendre une fois qu'un relecteur a ete selectionne.

- **Perimetre:** relecteurs qui ont besoin d'acces au preview docs (`docs-preview.sora`,
  builds GitHub Pages ou bundles SoraFS) avant GA.
- **Hors perimetre:** operateurs Torii ou SoraFS (couverts par leurs propres kits d'onboarding)
  et deploiements de portail en production (voir
  [`devportal/deploy-guide`](./deploy-guide.md)).

## Roles et prerequis

| Role | Objectifs typiques | Artefacts requis | Notes |
| --- | --- | --- | --- |
| Core maintainer | Verifier les nouveaux guides, executer des smoke tests. | Handle GitHub, contact Matrix, CLA signee au dossier. | Souvent deja dans l'equipe GitHub `docs-preview`; deposer quand meme une demande pour que l'acces soit auditable. |
| Partner reviewer | Valider des snippets SDK ou du contenu de gouvernance avant release publique. | Email corporate, POC legal, termes preview signes. | Doit reconnaitre les exigences de telemetrie + traitement des donnees. |
| Community volunteer | Fournir du feedback d'utilisabilite sur les guides. | Handle GitHub, contact prefere, fuseau horaire, acceptation du CoC. | Garder les cohortes petites; prioriser les relecteurs ayant signe l'accord de contribution. |

Tous les types de reviewers doivent:

1. Reconnaitre la politique d'usage acceptable pour les artefacts de preview.
2. Lire les annexes securite/observabilite
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. S'engager a executer `docs/portal/scripts/preview_verify.sh` avant de servir un
   snapshot localement.

## Workflow d'intake

1. Demander au demandeur de remplir le
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   formulaire (ou le copier/coller dans une issue). Capturer au minimum: identite, moyen de contact,
   handle GitHub, dates prevues de revue, et confirmation que les docs de securite ont ete lues.
2. Enregistrer la demande dans le tracker `docs-preview` (issue GitHub ou ticket de gouvernance)
   et assigner un approbateur.
3. Verifier les prerequis:
   - CLA / accord de contribution au dossier (ou reference de contrat partner).
   - Accuse d'usage acceptable stocke dans la demande.
   - Evaluation des risques terminee (exemple: reviewers partner approuves par Legal).
4. L'approbateur signe la demande et lie l'issue de suivi a toute entree de change-management
   (exemple: `DOCS-SORA-Preview-####`).

## Provisioning et outillage

1. **Partager les artefacts** - Fournir le dernier descriptor + archive de preview depuis
   le workflow CI ou le pin SoraFS (artefact `docs-portal-preview`). Rappeler aux reviewers
   d'executer:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **Servir avec enforcement de checksum** - Orienter les reviewers vers la commande gatee:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Cela reutilise `scripts/serve-verified-preview.mjs` pour qu'aucun build non verifie
   ne soit lance par accident.

3. **Donner l'acces GitHub (optionnel)** - Si les reviewers ont besoin de branches non publiees,
   les ajouter a l'equipe GitHub `docs-preview` pour la duree de la revue et consigner le changement
   de membership dans la demande.

4. **Communiquer les canaux de support** - Partager le contact on-call (Matrix/Slack) et la
   procedure d'incident de [`incident-runbooks`](./incident-runbooks.md).

5. **Telemetrie + feedback** - Rappeler aux reviewers que des analytics anonymisees sont collectees
   (voir [`observability`](./observability.md)). Fournir le formulaire de feedback ou le template
   d'issue mentionne dans l'invitation et journaliser l'evenement avec l'helper
   [`preview-feedback-log`](./preview-feedback-log) pour que le resume de vague reste a jour.

## Checklist du reviewer

Avant d'acceder au preview, les reviewers doivent completer:

1. Verifier les artefacts telecharges (`preview_verify.sh`).
2. Lancer le portail via `npm run serve` (ou `serve:verified`) pour s'assurer que le guard checksum est actif.
3. Lire les notes securite et observabilite referencees ci-dessus.
4. Tester la console OAuth/Try it via device-code login (si applicable) et eviter de reutiliser des tokens de production.
5. Deposer les constats dans le tracker convenu (issue, doc partage ou formulaire) et les tagger
   avec le tag de release preview.

## Responsabilites des maintainers et offboarding

| Phase | Actions |
| --- | --- |
| Kickoff | Confirmer que la checklist d'intake est jointe a la demande, partager les artefacts + instructions, ajouter une entree `invite-sent` via [`preview-feedback-log`](./preview-feedback-log), et planifier un point mi-parcours si la revue dure plus d'une semaine. |
| Monitoring | Surveiller la telemetrie preview (trafic Try it inhabituel, echec de probe) et suivre le runbook d'incident si quelque chose parait suspect. Journaliser les evenements `feedback-submitted`/`issue-opened` au fur et a mesure que les constats arrivent pour que les metriques de vague restent exactes. |
| Offboarding | Revoquer l'acces temporaire GitHub ou SoraFS, consigner `access-revoked`, archiver la demande (inclure resume de feedback + actions en attente), et mettre a jour le registre des reviewers. Demander au reviewer de purger les builds locaux et joindre le digest genere a partir de [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Utilisez le meme processus lors de la rotation des reviewers entre vagues. Garder la trace dans le repo
(issue + templates) aide DOCS-SORA a rester auditable et permet a la gouvernance de confirmer que l'acces
preview a suivi les controles documentes.

## Templates d'invitation et suivi

- Commencer chaque outreach avec le
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
  fichier. Il capture le minimum de langage legal, les instructions de checksum preview et l'attente
  que les reviewers reconnaissent la politique d'usage acceptable.
- Lors de l'edition du template, remplacer les placeholders pour `<preview_tag>`, `<request_ticket>`
  et les canaux de contact. Stocker une copie du message final dans le ticket d'intake pour que reviewers,
  approbateurs et auditeurs puissent referencer le texte exact envoye.
- Apres l'envoi de l'invitation, mettre a jour le spreadsheet de suivi ou l'issue avec le timestamp
  `invite_sent_at` et la date de fin attendue pour que le rapport
  [preview invite flow](./preview-invite-flow.md) puisse capturer la cohorte automatiquement.
