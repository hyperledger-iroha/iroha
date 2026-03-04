---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-plan
titre : Plan de vol en amont partenaires W1
sidebar_label : Plan W1
description : Taches, responsables et checklist de preuve pour la cohorte de prévisualisation partenaires.
---

| Élément | Détails |
| --- | --- |
| Vague | W1 - Partenaires et intégrateurs Torii |
| Fenêtre cible | T2 2025 semaine 3 |
| Tag d'artefact (planifier) ​​| `preview-2025-04-12` |
| Suivi des problèmes | `DOCS-SORA-Preview-W1` |

## Objectifs

1. Obtenir les approbations légales et de gouvernance pour les termes de prévisualisation des partenaires.
2. Préparer le proxy Try it et les instantanés de télémétrie utilisés dans le bundle d'invitation.
3. Rafraichir l'artefact de prévisualisation vérifié par checksum et les résultats de sondes.
4. Finaliser le roster des partenaires et les modèles de demande avant l'envoi des invitations.

## Découpage des taches| ID | Taché | Responsable | Écheance | Statuts | Remarques |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtenir l'approbation légale pour l'addendum des termes de prévisualisation | Responsable Docs/DevRel -> Juridique | 2025-04-05 | Terminer | Billet légal `DOCS-SORA-Preview-W1-Legal` valide le 2025-04-05 ; PDF joint au tracker. |
| W1-P2 | Capturer la fenêtre de staging du proxy Try it (2025-04-10) et valider la santé du proxy | Docs/DevRel + Ops | 2025-04-06 | Terminer | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` exécute le fichier 2025-04-06 ; transcription CLI + `.env.tryit-proxy.bak` archivées. |
| W1-P3 | Construire l'artefact de prévisualisation (`preview-2025-04-12`), exécuteur `scripts/preview_verify.sh` + `npm run probe:portal`, descripteur d'archiveur/sommes de contrôle | Portail TL | 2025-04-08 | Terminer | Artefact + logs de vérification stocks sous `artifacts/docs_preview/W1/preview-2025-04-12/`; sortie de sonde attachée au tracker. |
| W1-P4 | Revoir les formulaires d'admission partenaires (`DOCS-SORA-Preview-REQ-P01...P08`), confirmer contacts + NDAs | Liaison gouvernance | 2025-04-07 | Terminer | Les huit demandes approuvées (les deux dernières le 2025-04-11); approbations liées dans le tracker. |
| W1-P5 | Rediger le texte d'invitation (base sur `docs/examples/docs_preview_invite_template.md`), définir `<preview_tag>` et `<request_ticket>` pour chaque partenaire | Responsable Docs/DevRel | 2025-04-08 | Terminer | Brouillon d'invitation envoyé le 2025-04-12 15:00 UTC avec les liens d'artefact. |

## Liste de contrôle en amont> Astuce : lancez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` pour exécuter automatiquement les étapes 1-5 (build, checksum de vérification, sonde du portail, vérificateur de liens, et mise à jour du proxy Try it). Le script enregistre un log JSON à joindre au tracker.

1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) pour régénérer `build/checksums.sha256` et `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et archiveur `build/link-report.json` à la cote du descripteur.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou fournir la cible appropriée via `--tryit-target`) ; committer le `.env.tryit-proxy` mis à jour et conserver la `.bak` pour rollback.
6. Mettre à jour l'issue W1 avec les chemins de logs (checksum du descriptor, sortie probe, changement du proxy Try it et snapshots Grafana).

## Checklist de preuve

- [x] Approbation legale signée (PDF ou lien du ticket) jointe a `DOCS-SORA-Preview-W1`.
- [x] Captures d'écran Grafana pour `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descriptor et log de checksum `preview-2025-04-12` stockes sous `artifacts/docs_preview/W1/`.
- [x] Tableau de roster d'invitations avec renseignes `invite_sent_at` (voir le log W1 du tracker).
- [x] Artefacts de feedback repris dans [`preview-feedback/w1/log.md`](./log.md) avec une ligne par partenaire (mis à jour 2025-04-26 avec roster/telemetria/issues).

Mettre un jour ce plan à mesure de l'avancement; le tracker s'y réfère pour garder la feuille de route auditable.

## Flux de rétroaction1. Pour chaque reviewer, dupliquer le template dans
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   remplir les métadonnées et stocker la copie complète sous
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Invitations de reprise, points de contrôle de télémétrie et enjeux ouverts dans le log vivant
   [`preview-feedback/w1/log.md`](./log.md) pour que les reviewers gouvernance puissent rejouer la vague
   sans quitter le dépôt.
3. Quand les exports de knowledge-check ou de sondages arrivent, les joindre dans le chemin d'artefact note dans le log
   et lier l'issue du tracker.