---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-plan
titre : Plan de contrôle en amont de partenaires W1
sidebar_label : Plan W1
description : Tareas, responsables et liste de contrôle de preuves pour la cohorte de prévisualisation de partenaires.
---

| Article | Détails |
| --- | --- |
| Ola | W1 - Partenaires et intégrateurs de Torii |
| Vente objet | T2 2025 semaine 3 |
| Étiquette d'artefact (plané) | `preview-2025-04-12` |
| Problème du tracker | `DOCS-SORA-Preview-W1` |

## Objets

1. Asegurar aprobaciones legales y de gobernanza para los terminos de preview de partenaires.
2. Préparez le proxy Essayez-le et les instantanés de télémétrie utilisés dans le paquet d'invitation.
3. Actualisez l'artefact d'aperçu vérifié par la somme de contrôle et les résultats des sondes.
4. Finaliser la liste des partenaires et les plantes de sollicitude avant d'envoyer des invitations.

## Desglose de tareas| ID | Tarée | Responsable | Fecha limite | État | Notes |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtenir une autorisation légale pour l'annexe des fins de prévisualisation | Responsable Docs/DevRel -> Juridique | 2025-04-05 | Terminé | Billet légal `DOCS-SORA-Preview-W1-Legal` approuvé le 2025-04-05 ; PDF complémentaire au tracker. |
| W1-P2 | Capturer la fenêtre de mise en scène du proxy Essayez-le (2025-04-10) et valider la santé du proxy | Docs/DevRel + Ops | 2025-04-06 | Terminé | Voir l'exécution `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` le 2025-04-06 ; transcription de CLI et archives `.env.tryit-proxy.bak`. |
| W1-P3 | Construire un artefact d'aperçu (`preview-2025-04-12`), correr `scripts/preview_verify.sh` + `npm run probe:portal`, descripteur d'archivage/sommes de contrôle | Portail TL | 2025-04-08 | Terminé | Artefact et journaux de vérification gardés en `artifacts/docs_preview/W1/preview-2025-04-12/` ; sortie de sonde adjointe au tracker. |
| W1-P4 | Réviser les formulaires d'admission des partenaires (`DOCS-SORA-Preview-REQ-P01...P08`), confirmer les contacts et les NDA | Liaison gouvernance | 2025-04-07 | Terminé | Las ocho sollicitudes aprobadas (las ultimas dos el 2025-04-11) ; aprobaciones enlazadas en el tracker. |
| W1-P5 | Rédiger une copie de l'invitation (basée sur `docs/examples/docs_preview_invite_template.md`), fijar `<preview_tag>` et `<request_ticket>` pour chaque partenaire | Responsable Docs/DevRel | 2025-04-08 | Terminé | Le destinataire de l'invitation a envoyé le 2025-04-12 15:00 UTC avec les liens de l'artefact. |

## Checklist de contrôle en amont> Consigne : exécutez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` pour exécuter automatiquement les étapes 1 à 5 (build, vérification de la somme de contrôle, sonde du portail, vérificateur de liens et actualisation du proxy). Essayez-le). Le script enregistre un journal JSON qui peut être ajouté au problème du tracker.

1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) pour régénérer `build/checksums.sha256` et `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et archive `build/link-report.json` avec le descripteur.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (pour passer la cible adéquate via `--tryit-target`) ; commettez le `.env.tryit-proxy` à jour et conservez le `.bak` pour la restauration.
6. Actualisez le problème W1 avec les traces des journaux (somme de contrôle du descripteur, sortie de sonde, changement du proxy Try it et instantanés Grafana).

## Liste de contrôle des preuves

- [x] Aprobacion legal firmada (PDF ou à joindre au ticket) en complément du `DOCS-SORA-Preview-W1`.
- [x] Captures d'écran de Grafana pour `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descripteur et journal de la somme de contrôle de `preview-2025-04-12` gardés sous `artifacts/docs_preview/W1/`.
- [x] Tableau de liste d'invitations avec horodatages `invite_sent_at` complet (voir journal W1 du tracker).
- [x] Artefacts de feedback réfléchis sur [`preview-feedback/w1/log.md`](./log.md) avec un fil par partenaire (actualisé le 2025-04-26 avec les données de roster/télémétrie/issues).

Actualisez ce plan à mesure que vous avancez les tâches ; le tracker est la référence pour maintenir la feuille de route auditable.## Flux de commentaires

1. Para cada reviewer, duplicata la plantilla fr
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   complétez les métadonnées et gardez la copie terminée en bas
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Reprendre les invitations, les points de contrôle de télémétrie et les problèmes ouverts dans le journal vivo en
   [`preview-feedback/w1/log.md`](./log.md) pour que les réviseurs d'État puissent réviser toute la journée
   sans sortir du dépôt.
3. Lorsque vous effectuez des exportations de contrôle des connaissances ou d'enquêtes, complémentaires à l'itinéraire des artefacts indiqués dans le journal
   et enlaza el issue del tracker.