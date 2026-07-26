---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-plan
titre : Plan de contrôle en amont pour les partenaires W1
sidebar_label : Plan W1
description : Liste des documents, des listes et des listes de vérification pour les aperçus des colis des partenaires.
---

| Пункт | Détails |
| --- | --- |
| Volna | W1 - partenaires et intégrateurs Torii |
| Целевое окно | T2 2025 pas 3 |
| Objets d'art (plan) | `preview-2025-04-12` |
| Trekker | `DOCS-SORA-Preview-W1` |

## Celi

1. Aperçu de la gouvernance et de la gouvernance des partenaires.
2. Ajoutez Try it proxy et les images télémétriques pour la configuration du paquet.
3. Ouvrir l'aperçu de l'aperçu de la somme de contrôle et la sonde des résultats.
4. Finalisez les partenaires et les fournisseurs de services pour les opérations de démarrage.

## Разбивка задач| ID | Задача | Владелец | Срок | Statut | Première |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | Afficher l'évolution de la situation de votre entreprise aperçu | Responsable Docs/DevRel -> Juridique | 2025-04-05 | ✅ Завершено | Billet original `DOCS-SORA-Preview-W1-Legal` du 05/04/2025 ; PDF приложен к трекеру. |
| W1-P2 | Connectez-vous au processus de mise en scène Essayez-le proxy (2025-04-10) et vérifiez le processus | Docs/DevRel + Ops | 2025-04-06 | ✅ Завершено | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` publié le 06/04/2025 ; Transcription CLI et archives `.env.tryit-proxy.bak`. |
| W1-P3 | Собрать preview-artefakt (`preview-2025-04-12`), запустить `scripts/preview_verify.sh` + `npm run probe:portal`, archivage du descripteur/sommes de contrôle | Portail TL | 2025-04-08 | ✅ Завершено | Objets et journaux de vérification relatifs à `artifacts/docs_preview/W1/preview-2025-04-12/` ; вывод sonde приложен к трекеру. |
| W1-P4 | Vérifier les formulaires d'admission des partenaires (`DOCS-SORA-Preview-REQ-P01...P08`), confirmer les contacts et les NDA | Liaison gouvernance | 2025-04-07 | ✅ Завершено | Vous avez des problèmes à faire (le 2025-04-11) ; ссылки на одобрения в трекере. |
| W1-P5 | Ajoutez le texte de configuration (sur la base `docs/examples/docs_preview_invite_template.md`), envoyez `<preview_tag>` et `<request_ticket>` pour votre partenaire | Responsable Docs/DevRel | 2025-04-08 | ✅ Завершено | La mise à jour récente s'est déroulée le 2025-04-12 15:00 UTC avec la création de l'art. |

## Liste de contrôle en amont> Solution : installez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json`, qui effectue automatiquement les étapes 1 à 5 (build, vérification de la somme de contrôle, sonde de portail, vérificateur de liens et mise à jour Essayez-le proxy). Le script contient le journal JSON qui peut être utilisé pour le suivi du problème.

1. `npm run build` (avec `DOCS_RELEASE_TAG=preview-2025-04-12`) pour la période `build/checksums.sha256` et `build/release.json`.
2. `docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3. `PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` et archiver `build/link-report.json` directement avec le descripteur.
5. `npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (ou rechercher une cible proche de `--tryit-target`) ; installez le `.env.tryit-proxy` et supprimez `.bak` pour la restauration.
6. Identifiez le problème W1 avec les logos (descripteur de somme de contrôle, sonde sélectionnée, proxy Try it et instantanés Grafana).

## Liste de contrôle des documents

- [x] Подписанное юридическое одобрение (PDF ou ссылка на тикет) приложено к `DOCS-SORA-Preview-W1`.
- [x] Grafana autocollants pour `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] Descripteur et somme de contrôle `preview-2025-04-12` correspondant à `artifacts/docs_preview/W1/`.
- [x] Таблица roster приглашений с заполненными `invite_sent_at` (avec journal W1 dans трекере).
- [x] Les articles sont actuellement disponibles dans [`preview-feedback/w1/log.md`](./log.md) avec le premier contact avec le partenaire (déjà 2025-04-26 liste/télémétrie/problèmes).

Veuillez consulter ce plan pour votre simple production ; Le treker ssылается на него, чтобы сохранить feuille de route d'auditabilité.

## Processus de travail régulier1. Pour chaque critique, doublez le morceau
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md),
   заполнить метаданные и сохранить готовую копию в
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. Suivez les étapes, les points de contrôle télémétriques et la résolution des problèmes dans votre journal
   [`preview-feedback/w1/log.md`](./log.md), que les réviseurs de gouvernance могли воспроизвести волну
   полностью, не покидая репозиторий.
3. Lorsque vous achetez des produits de contrôle des connaissances ou des entreprises qui achètent des articles en vente dans le journal,
   и связывать с issue трекера.