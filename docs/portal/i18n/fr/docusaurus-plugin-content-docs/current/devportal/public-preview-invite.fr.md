---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook d'invitation en avant-première publique

## Objectifs du programme

Ce playbook explique comment annoncer et faire tourner le preview public une fois que le
le workflow d'intégration des évaluateurs est actif. Il garde la feuille de route DOCS-SORA honnête en
s'assurant que chaque partie d'invitation avec des artefacts vérifiables, des consignes de sécurité
et un chemin clair pour le feedback.

- **Audience :** liste curée des membres de la communauté, partenaires et mainteneurs qui ont
  signez la politique d'usage acceptable du aperçu.
- **Plafonds:** taille de vague par défaut <= 25 reviewers, fenêtre d'accès de 14 jours,
  réponse aux incidents sous 24 h.

## Checklist de porte de lancement

Terminez ces taches avant d'envoyer une invitation :

1. Derniers artefacts de prévisualisation charges en CI (`docs-portal-preview`,
   manifeste de somme de contrôle, descripteur, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (gate par checksum) teste sur le tag meme.
3. Les tickets d'onboarding des reviewers approuvent et se trouvent à la vague d'invitation.
4. Documents sécurisés, observabilité et incidents validés
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Formulaire de feedback ou template d'issue prepare (incluant des champs de sévérité,
   étapes de reproduction, captures d'écran et info d'environnement).
6. Copie d'annonce révisée par Docs/DevRel + Governance.## Paquet d'invitation

Chaque invitation doit inclure :

1. **Artefacts verifies** - Fournir les liens vers le manifeste/plan SoraFS ou les artefacts
   GitHub, plus le manifeste de checksum et le descripteur. Referencer explicitant la commande
   de vérification pour que les évaluateurs puissent l'exécuter avant de lancer le site.
2. **Instructions de service** - Inclure la commande preview gatee par checksum :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels sécurisé** - Indiquer que les tokens expirent automatiquement, que les liens
   ne doivent pas être partagés, et que les incidents doivent être signalés immédiatement.
4. **Canal de feedback** - Lier le template/formulaire et clarifier les attentes de temps de réponse.
5. **Dates du programme** - Fournir les dates de début/fin, office hours ou syncs, et la prochaine
   fenêtre de rafraîchissement.

L'email exemple dans
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
couvre ces exigences. Mettre à jour les espaces réservés (dates, URL, contacts)
avant l'envoi.

## Exposer l'hôte aperçu

Ne promouvoir l'hôte qu'une fois l'onboarding terminé et le ticket de changement approuvé.
Voir le [guide d'exposition de l'hote preview](./preview-host-exposure.md) pour les étapes de bout en bout
de build/publish/verify utilisé dans cette section.

1. **Build et packaging:** Marquer le release tag et produire des artefacts déterministes.

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
   ```Le script de pin écrit `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   et `portal.dns-cutover.json` sous `artifacts/sorafs/`. Joindre ces fichiers à la vague
   d'invitation afin que chaque reviewer puisse vérifier les memes bits.

2. **Publier l'alias preview:** Relancer la commande sans `--skip-submit`
   (fournir `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, et la preuve d'alias émise
   par la gouvernance). Le script lie le manifeste à `docs-preview.sora` et emet
   `portal.manifest.submit.summary.json` plus `portal.pin.report.json` pour le bundle de preuves.

3. **Prober le deploiement:** Confirmer que l'alias est résultat et que la somme de contrôle correspond au tag
   avant d'envoyer les invitations.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) sous la main comme repli pour
   que les réviseurs peuvent lancer une copie locale si le bord aperçu flanche.

## Chronologie de communication| Jour | Actions | Propriétaire |
| --- | --- | --- |
| J-3 | Finaliser la copie d'invitation, rafraichir les artefacts, dry-run de vérification | Docs/DevRel |
| J-2 | Signature gouvernance + ticket de changement | Docs/DevRel + Gouvernance |
| J-1 | Envoyer les invitations via le template, mettre à jour le tracker avec la liste des destinataires | Docs/DevRel |
| D | Kickoff call / office hours, surveiller les tableaux de bord de télémétrie | Docs/DevRel + Sur appel |
| J+7 | Digest de feedback à mi-vague, triage des problèmes bloquantes | Docs/DevRel |
| J+14 | Clore la vague, revoquer l'accès temporaire, publier un CV dans `status.md` | Docs/DevRel |

## Suivi d'accès et télémétrie

1. Enregistrer chaque destinataire, timestamp d'invitation et date de révocation avec le
   aperçu de l'enregistreur de commentaires (voir
   [`preview-feedback-log`](./preview-feedback-log)) afin que chaque vague partage la meme
   trace de preuves :

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   Les événements pris en charge sont `invite-sent`, `acknowledged`,
   `feedback-submitted`, `issue-opened`, et `access-revoked`. Le log se trouve à
   `artifacts/docs_portal_preview/feedback_log.json` par défaut ; rejoignez-le au ticket de
   vague d'invitation avec les formulaires de consentement. Utilisez l'helper de résumé
   pour produire un roll-up auditable avant la note de clôture :

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```Le résumé JSON énumére les invitations par vague, les destinataires ouverts, les
   compteurs de feedback et le timestamp du dernier événement. L'aide repose sur
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   donc le même workflow peut tourner localement ou en CI. Utiliser le modèle de résumé
   dans [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   lors de la publication du récapitulatif de la vague.
2. Tagger les tableaux de bord de télémétrie avec le `DOCS_RELEASE_TAG` utiliser pour la vague afin que
   les photos pourraient être correles aux cohortes d'invitation.
3. Executer `npm run probe:portal -- --expect-release=<tag>` après le déploiement pour confirmer que
   l'environnement preview annonce la bonne métadonnée de release.
4. Consigner tout incident dans le template de runbook et le lier à la cohorte.

## Feedback et clôture1. Agréger les retours dans un doc partage ou un board d'issues. Tagger les éléments avec
   `docs-preview/<wave>` pour que les propriétaires du roadmap puissent les retrouver facilement.
2. Utiliser la sortie summary du preview logger pour remplir le rapport de vague, puis
   reprendre la cohorte dans `status.md` (participants, principaux constats, fixes prevus) et
   mettre à jour `roadmap.md` si le jalon DOCS-SORA a change.
3. Suivre les étapes d'offboarding depuis
   [`reviewer-onboarding`](./reviewer-onboarding.md) : révoquer l'accès, archiver les demandes et
   merci aux participants.
4. Préparer la prochaine vague en rafraichissant les artefacts, en relancant les portes de contrôle
   et en mettant à jour le modèle d'invitation avec de nouvelles dates.

Appliquer ce playbook de facon cohérent garder le programme aperçu auditable et donne a
Docs/DevRel un moyen répétable de faire grandir les invitations à mesure que le portail approche de GA.