---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w2/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w2-plan
titre : Planifier l'admission communautaire W2
sidebar_label : Plan W2
description : Admission, одобрения и чек-list доказательств для community preview когорты.
---

| Пункт | Détails |
| --- | --- |
| Volna | W2 - réviseurs de la communauté |
| Целевое окно | T3 2025 pas 1 (prévu) |
| Objets d'art (plan) | `preview-2025-06-15` |
| Trekker | `DOCS-SORA-Preview-W2` |

## Celi

1. Évaluez les critères d'admission de la communauté et la vérification des flux de travail.
2. Publier la liste de gouvernance et l'addendum d'utilisation acceptable.
3. Ouvrir l'aperçu de l'image de vérification de la somme de contrôle et l'ensemble télémétrique à ce moment-là.
4. Ajouter Essayez-le proxy et tableaux de bord pour l'installation.

## Разбивка задач| ID | Задача | Владелец | Срок | Statut | Première |
| --- | --- | --- | --- | --- | --- |
| W2-P1 | Ajouter des critères d'admission à la communauté (éligibilité, nombre maximum d'emplacements, certification CoC) et définir la gouvernance | Responsable Docs/DevRel | 2025-05-15 | ✅ Завершено | L'admission politique s'est déroulée dans le `DOCS-SORA-Preview-W2` et s'est déroulée le 20/05/2025. |
| W2-P2 | Créer un modèle de demande pour la communauté (motivation, disponibilité, besoins de localisation) | Docs-core-01 | 2025-05-18 | ✅ Завершено | `docs/examples/docs_preview_request_template.md` vous trouverez la Communauté et votre formulaire d'admission. |
| W2-P3 | Получить gouvernance-одобрение плана admission (голосование + протокол) | Liaison gouvernance | 2025-05-22 | ✅ Завершено | L'événement a eu lieu le 20/05/2025 ; Le protocole et l'appel sont effectués sur `DOCS-SORA-Preview-W2`. |
| W2-P4 | Planifier la mise en scène Essayez-le proxy + télémètre pour W2 (`preview-2025-06-15`) | Docs/DevRel + Ops | 2025-06-05 | ✅ Завершено | Changer le ticket `OPS-TRYIT-188` одобрен и выполнен 2025-06-09 02:00-04:00 UTC ; Grafana les écrans sont connectés au ticket. |
| W2-P5 | Ajouter/améliorer la nouvelle balise d'artefact d'aperçu (`preview-2025-06-15`) et archiver les journaux de descripteur/somme de contrôle/sonde | Portail TL | 2025-06-07 | ✅ Завершено | `scripts/preview_wave_preflight.sh --tag preview-2025-06-15 ...` publié le 10/06/2025 ; sorties сохранены в `artifacts/docs_preview/W2/preview-2025-06-15/`. || W2-P6 | Former la communauté de liste (<=25 évaluateurs, lots par étapes) avec les contacts et la gouvernance | Gestionnaire de communauté | 2025-06-10 | ✅ Завершено | Первая когорта из 8 évaluateurs de la communauté одобрена; IDs `DOCS-SORA-Preview-REQ-C01...C08` записаны в трекере. |

## Liste de contrôle des documents

- [x] Afficher l'approbation de la gouvernance (заметки встречи + ссылка на голосование) приложена к `DOCS-SORA-Preview-W2`.
- [x] Le modèle de demande Обновленный закоммичен под `docs/examples/`.
- [x] Descripteur `preview-2025-06-15`, journal de somme de contrôle, sortie de sonde, rapport de lien et essayez-le, transcription proxy dans `artifacts/docs_preview/W2/`.
- [x] Captures d'écran Grafana (`docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`) pour le contrôle en amont W2.
- [x] La liste des listes contient les identifiants des réviseurs, les demandes de tickets et les horodatages pour les opérations (dans la section W2 du voyage).

Vérifiez ce plan actuel ; Le treker s'est penché sur la feuille de route DOCS-SORA, qui est en cours d'exécution pour l'utilisation de W2.