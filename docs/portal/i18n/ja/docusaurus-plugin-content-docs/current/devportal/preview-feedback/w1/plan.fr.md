---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: preview-feedback-w1-plan
title: Plan de preflight partenaires W1
sidebar_label: Plan W1
description: Taches, responsables et checklist de preuve pour la cohorte de preview partenaires.
---

| Element | Details |
| --- | --- |
| Vague | W1 - Partenaires et integrateurs Torii |
| Fenetre cible | Q2 2025 semaine 3 |
| Tag d'artefact (planifie) | `preview-2025-04-12` |
| Issue tracker | `DOCS-SORA-Preview-W1` |

## Objectifs

1. Obtenir les approbations legales et governance pour les termes de preview partenaires.
2. Preparer le proxy Try it et les snapshots de telemetrie utilises dans le bundle d'invitation.
3. Rafraichir l'artefact de preview verifie par checksum et les resultats de probes.
4. Finaliser le roster des partenaires et les templates de demande avant l'envoi des invitations.

## Decoupage des taches

| ID | Tache | Responsable | Echeance | Statut | Notes |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtenir l'approbation legale pour l'addendum des termes de preview | Docs/DevRel lead -> Legal | 2025-04-05 | Termine | Ticket legal `DOCS-SORA-Preview-W1-Legal` valide le 2025-04-05; PDF attache au tracker. |
| W1-P2 | Capturer la fenetre de staging du proxy Try it (2025-04-10) et valider la sante du proxy | Docs/DevRel + Ops | 2025-04-06 | Termine | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` execute le 2025-04-06; transcription CLI + `.env.tryit-proxy.bak` archivees. |
| W1-P3 | Construire l'artefact de preview (`preview-2025-04-12`), executer `scripts/preview_verify.sh` + `npm run probe:portal`, archiver descriptor/checksums | Portal TL | 2025-04-08 | Termine | Artefact + logs de verification stockes sous `artifacts/docs_preview/W1/preview-2025-04-12/`; sortie de probe attachee au tracker. |
| W1-P4 | Revoir les formulaires d'intake partenaires (`DOCS-SORA-Preview-REQ-P01...P08`), confirmer contacts + NDAs | Governance liaison | 2025-04-07 | Termine | Les huit demandes approuvees (les deux dernieres le 2025-04-11); approbations liees dans le tracker. |
| W1-P5 | Rediger le texte d'invitation (base sur `docs/examples/docs_preview_invite_template.md`), definir `<preview_tag>` et `<request_ticket>` pour chaque partenaire | Docs/DevRel lead | 2025-04-08 | Termine | Brouillon d'invitation envoye le 2025-04-12 15:00 UTC avec les liens d'artefact. |

## Checklist preflight

> Astuce: lancez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` pour executer automatiquement les etapes 1-5 (build, verification checksum, probe du portal, link checker, et mise a jour du proxy Try it). Le script enregistre un log JSON a joindre au tracker.

1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) pour regenerer `build/checksums.sha256` et `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et archiver `build/link-report.json` a cote du descriptor.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou fournir la cible appropriee via `--tryit-target`); committer le `.env.tryit-proxy` mis a jour et conserver la `.bak` pour rollback.
6. Mettre a jour l'issue W1 avec les chemins de logs (checksum du descriptor, sortie probe, changement du proxy Try it et snapshots Grafana).

## Checklist de preuve

- [x] Approbation legale signee (PDF ou lien du ticket) attachee a `DOCS-SORA-Preview-W1`.
- [x] Screenshots Grafana pour `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor et log de checksum `preview-2025-04-12` stockes sous `artifacts/docs_preview/W1/`.
- [x] Tableau de roster d'invitations avec `invite_sent_at` renseignes (voir le log W1 du tracker).
- [x] Artefacts de feedback repris dans [`preview-feedback/w1/log.md`](./log.md) avec une ligne par partenaire (mis a jour 2025-04-26 avec roster/telemetria/issues).

Mettre a jour ce plan a mesure de l'avancement; le tracker s'y refere pour garder le roadmap auditable.

## Flux de feedback

1. Pour chaque reviewer, dupliquer le template dans
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   remplir les metadonnees et stocker la copie complete sous
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Resumer invitations, checkpoints de telemetrie et issues ouvertes dans le log vivant
   [`preview-feedback/w1/log.md`](./log.md) pour que les reviewers governance puissent rejouer la vague
   sans quitter le depot.
3. Quand les exports de knowledge-check ou de sondages arrivent, les joindre dans le chemin d'artefact note dans le log
   et lier l'issue du tracker.
