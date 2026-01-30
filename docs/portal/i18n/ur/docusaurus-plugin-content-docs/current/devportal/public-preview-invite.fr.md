---
lang: ur
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Playbook d'invitation preview public

## Objectifs du programme

Ce playbook explique comment annoncer et faire tourner le preview public une fois que le
workflow d'onboarding des reviewers est actif. Il garde la roadmap DOCS-SORA honnete en
s'assurant que chaque invitation part avec des artefacts verifiables, des consignes de securite
et un chemin clair pour le feedback.

- **Audience:** liste curee de membres de la communaute, partners et maintainers qui ont
  signe la politique d'usage acceptable du preview.
- **Plafonds:** taille de vague par defaut <= 25 reviewers, fenetre d'acces de 14 jours,
  reponse aux incidents sous 24 h.

## Checklist de gate de lancement

Terminez ces taches avant d'envoyer une invitation:

1. Derniers artefacts de preview charges en CI (`docs-portal-preview`,
   manifest de checksum, descriptor, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gate par checksum) teste sur le meme tag.
3. Tickets d'onboarding des reviewers approuves et lies a la vague d'invitation.
4. Docs securite, observabilite et incidents valides
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulaire de feedback ou template d'issue prepare (inclure des champs de severite,
   etapes de reproduction, screenshots et info d'environnement).
6. Copy d'annonce revise par Docs/DevRel + Governance.

## Paquet d'invitation

Chaque invitation doit inclure:

1. **Artefacts verifies** - Fournir les liens vers le manifest/plan SoraFS ou les artefacts
   GitHub, plus le manifest de checksum et le descriptor. Referencer explicitement la commande
   de verification pour que les reviewers puissent l'executer avant de lancer le site.
2. **Instructions de serve** - Inclure la commande preview gatee par checksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels securite** - Indiquer que les tokens expirent automatiquement, que les liens
   ne doivent pas etre partages, et que les incidents doivent etre signales immediatement.
4. **Canal de feedback** - Lier la template/formulaire et clarifier les attentes de temps de reponse.
5. **Dates du programme** - Fournir les dates de debut/fin, office hours ou syncs, et la prochaine
   fenetre de refresh.

L'email exemple dans
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
couvre ces exigences. Mettre a jour les placeholders (dates, URLs, contacts)
avant l'envoi.

## Exposer l'hote preview

Ne promouvoir l'hote preview qu'une fois l'onboarding termine et le ticket de changement approuve.
Voir le [guide d'exposition de l'hote preview](./preview-host-exposure.md) pour les etapes end-to-end
de build/publish/verify utilisees dans cette section.

1. **Build et packaging:** Marquer le release tag et produire des artefacts deterministes.

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
   ```

   Le script de pin ecrit `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   et `portal.dns-cutover.json` sous `artifacts/sorafs/`. Joindre ces fichiers a la vague
   d'invitation afin que chaque reviewer puisse verifier les memes bits.

2. **Publier l'alias preview:** Relancer la commande sans `--skip-submit`
   (fournir `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, et la preuve d'alias emise
   par la gouvernance). Le script lie le manifest a `docs-preview.sora` et emet
   `portal.manifest.submit.summary.json` plus `portal.pin.report.json` pour le bundle de preuves.

3. **Prober le deploiement:** Confirmer que l'alias se resout et que le checksum correspond au tag
   avant d'envoyer les invitations.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) sous la main comme fallback pour
   que les reviewers puissent lancer une copie locale si le edge preview flanche.

## Timeline de communication

| Jour | Action | Owner |
| --- | --- | --- |
| D-3 | Finaliser la copy d'invitation, rafraichir les artefacts, dry-run de verification | Docs/DevRel |
| D-2 | Sign-off gouvernance + ticket de changement | Docs/DevRel + Governance |
| D-1 | Envoyer les invitations via le template, mettre a jour le tracker avec la liste des destinataires | Docs/DevRel |
| D | Kickoff call / office hours, monitorer les dashboards de telemetrie | Docs/DevRel + On-call |
| D+7 | Digest de feedback a mi-vague, triage des issues bloquantes | Docs/DevRel |
| D+14 | Clore la vague, revoquer l'acces temporaire, publier un resume dans `status.md` | Docs/DevRel |

## Suivi d'acces et telemetrie

1. Enregistrer chaque destinataire, timestamp d'invitation et date de revocation avec le
   preview feedback logger (voir
   [`preview-feedback-log`](./preview-feedback-log)) afin que chaque vague partage la meme
   trace de preuves:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Les evenements pris en charge sont `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, et `access-revoked`. Le log se trouve a
   `artifacts/docs_portal_preview/feedback_log.json` par defaut; joignez-le au ticket de
   vague d'invitation avec les formulaires de consentement. Utilisez l'helper de summary
   pour produire un roll-up auditable avant la note de cloture:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   Le summary JSON enumere les invitations par vague, les destinataires ouverts, les
   compteurs de feedback et le timestamp du dernier evenement. L'helper repose sur
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   donc le meme workflow peut tourner localement ou en CI. Utiliser le template de digest
   dans [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   lors de la publication du recap de la vague.
2. Tagger les dashboards de telemetrie avec le `DOCS_RELEASE_TAG` utilise pour la vague afin que
   les pics puissent etre correles aux cohortes d'invitation.
3. Executer `npm run probe:portal -- --expect-release=<tag>` apres le deploy pour confirmer que
   l'environnement preview annonce la bonne metadata de release.
4. Consigner tout incident dans le template de runbook et le lier a la cohorte.

## Feedback et cloture

1. Agreger le feedback dans un doc partage ou un board d'issues. Tagger les items avec
   `docs-preview/<wave>` pour que les owners du roadmap puissent les retrouver facilement.
2. Utiliser la sortie summary du preview logger pour remplir le rapport de vague, puis
   resumer la cohorte dans `status.md` (participants, principaux constats, fixes prevus) et
   mettre a jour `roadmap.md` si le jalon DOCS-SORA a change.
3. Suivre les etapes d'offboarding depuis
   [`reviewer-onboarding`](./reviewer-onboarding.md): revoquer l'acces, archiver les demandes et
   remercier les participants.
4. Preparer la prochaine vague en rafraichissant les artefacts, en relancant les gates de checksum
   et en mettant a jour le template d'invitation avec de nouvelles dates.

Appliquer ce playbook de facon consistente garde le programme preview auditable et donne a
Docs/DevRel un moyen repetable de faire grandir les invitations a mesure que le portail approche de GA.
