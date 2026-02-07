---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-plan
titre : Plan de contrôle en amont de parceiros W1
sidebar_label : Plano W1
description : Tarefas, responsaveis et checklist de evidencia para a coorte de preview de parceiros.
---

| Article | Détails |
| --- | --- |
| Onde | W1 - Parciers et intégrateurs Torii |
| Janela aussi | T2 2025 semaine 3 |
| Étiquette d'artefato (avion) ​​| `preview-2025-04-12` |
| Problème de suivi | `DOCS-SORA-Preview-W1` |

## Objets

1. Garantir les accords légaux et de gouvernance pour les termes de prévisualisation des colis.
2. Préparez le proxy Essayez-le et les instantanés de télémétrie utilisés dans le paquet de invitation.
3. Actualiser l'artéfact d'aperçu vérifié par la somme de contrôle et les résultats des sondes.
4. Finaliser la liste des colis et les modèles de sollicitation avant l'envoi des invités.

## Départ des tares| ID | Taréfa | Responsavel | Prazo | Statut | Notes |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Obtenir l'approbation légale pour l'ajout des termes de prévisualisation | Responsable Docs/DevRel -> Juridique | 2025-04-05 | Concluido | Billet légal `DOCS-SORA-Preview-W1-Legal` approuvé le 05/04/2025 ; PDF annexé au tracker. |
| W1-P2 | Capturer l'étape de staging du proxy Essayez-le (2025-04-10) et valider audacieusement le proxy | Docs/DevRel + Ops | 2025-04-06 | Concluido | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` exécuté le 06/04/2025 ; transcription de CLI et `.env.tryit-proxy.bak` archivées. |
| W1-P3 | Construire un artéfact de prévisualisation (`preview-2025-04-12`), suivre `scripts/preview_verify.sh` + `npm run probe:portal`, descripteur d'archive/sommes de contrôle | Portail TL | 2025-04-08 | Concluido | Artefato et journaux de vérification armazenados em `artifacts/docs_preview/W1/preview-2025-04-12/` ; Saida de la sonde anexada ao tracker. |
| W1-P4 | Réviser les formulaires d'admission de parceiros (`DOCS-SORA-Preview-REQ-P01...P08`), confirmer les contacts et les NDA | Liaison gouvernance | 2025-04-07 | Concluido | Comme oito sollicitacoes aprovadas (comme duas ultimas em 2025-04-11); aprovacoes linkadas pas de tracker. |
| W1-P5 | Rediger l'invité (basé sur `docs/examples/docs_preview_invite_template.md`), définir `<preview_tag>` et `<request_ticket>` pour chaque colis | Responsable Docs/DevRel | 2025-04-08 | Concluido | Rascunho do convite enviado em 2025-04-12 15:00 UTC avec les liens de l'artefato. |

## Checklist de contrôle en amont> Ici : j'ai utilisé `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` pour exécuter automatiquement les étapes 1 à 5 (build, vérification de la somme de contrôle, sonde du portail, vérificateur de liens et actualisation du proxy Essayez-le). Le script enregistre un journal JSON qui peut être ajouté au problème du tracker.

1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) pour régénérer `build/checksums.sha256` et `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et récupérer `build/link-report.json` à côté du descripteur.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou passer la cible adéquate via `--tryit-target`) ; commit do `.env.tryit-proxy` actualisé et protégé par `.bak` pour la restauration.
6. Actualisez le problème W1 avec les chemins de journaux (somme de contrôle du descripteur, dite de la sonde, ne modifiez pas le proxy, essayez-le et les instantanés Grafana).

## Liste de contrôle des preuves

- [x] Aprovacao legal assinada (PDF ou lien vers le ticket) anexada ao `DOCS-SORA-Preview-W1`.
- [x] Captures d'écran de Grafana pour `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descripteur et journal de la somme de contrôle de `preview-2025-04-12` armazenados em `artifacts/docs_preview/W1/`.
- [x] Tableau de liste des convites avec `invite_sent_at` preenchido (ver log W1 sans tracker).
- [x] Artefatos de feedback reflétidos em [`preview-feedback/w1/log.md`](./log.md) avec une ligne par colis (actualisé le 2025-04-26 avec les données de roster/télémétrie/issues).

Actualisez ce plan conforme à tarefas avancarem ; o tracker o reference para manter o roadmap auditavel.

## Flux de commentaires1. Pour chaque réviseur, dupliquer le modèle
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   préencher les métadonnées et armazenar une copie complète em
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Résumer les convocations, les points de contrôle de télémétrie et les problèmes ouverts à l'intérieur du journal vivo em
   [`preview-feedback/w1/log.md`](./log.md) pour que les réviseurs de gouvernance puissent revenir aujourd'hui à l'onde
   sans doute le dépôt.
3. Lorsque les exportations de vérification des connaissances ou d'enquêtes sont effectuées, annexer le chemin des œuvres d'art indiqué dans le journal
   Le lien vers le problème du tracker.