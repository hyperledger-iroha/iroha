---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/summary.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-summary
titre : Сводка отзывов и закрытие W1
sidebar_label : Suède W1
description : Vous avez créé et fourni des documents pour les partenaires/intégrateurs de prévisualisation Torii.
---

| Пункт | Détails |
| --- | --- |
| Volna | W1 - partenaires et intégrateurs Torii |
| Окно приглашений | 2025-04-12 -> 2025-04-26 |
| Objets d'art | `preview-2025-04-12` |
| Trekker | `DOCS-SORA-Preview-W1` |
| Участники | sorafs-op-01...03, torii-int-01...02, sdk-partner-01...02, gateway-ops-01 |

## Moments agréables

1. **Somme de contrôle du flux de travail** - Les réviseurs ont vérifié le descripteur/l'archive par `scripts/preview_verify.sh` ; Les logs sont adaptés à la configuration.
2. **Телеметрия** - Les États-Unis `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` s'occupent de chaque vol ; Les incidents ou les pages d'alerte ne sont pas disponibles.
3. **Feedback sur docs (`docs-preview/w1`)** - Les informations concernant les principales fonctionnalités :
   - `docs-preview/w1 #1` : уточнить навигационную формулировку в разделе Essayez-le (закрыто).
   - `docs-preview/w1 #2` : обновить скриншот Essayez-le (закрыто).
4. **Parcourir le runbook** - Les opérateurs SoraFS ont été mis à jour, de nouveaux liens croisés étant `orchestrator-ops` et `multi-source-rollout` ajoutés à la configuration W0.

## Пункты действий| ID | Description | Владелец | Statut |
| --- | --- | --- | --- |
| W1-A1 | Ouvrir le formulaire de navigation Essayez-le en utilisant `docs-preview/w1 #1`. | Docs-core-02 | ✅ Завершено (2025-04-18). |
| W1-A2 | Обновить скриншот Essayez-le sur `docs-preview/w1 #2`. | Docs-core-03 | ✅ Завершено (2025-04-19). |
| W1-A3 | Vous avez des partenaires et des téléphones dans la feuille de route/le statut. | Responsable Docs/DevRel | ✅ Завершено (avec tracker + status.md). |

## Ce résumé (2025-04-26)

- Vos évaluateurs ont mis à jour les informations concernant les heures de bureau finales, les œuvres d'art locales et les offres ouvertes.
- Телеметрия оставалась зеленой до выхода; Les instantanés finaux sont utilisés avec `DOCS-SORA-Preview-W1`.
- Лог приглашений обновлен подтверждениями выхода; tracker отметил W1 как 🈴 и добавил points de contrôle.
- Bundle доказательств (descripteur, journal de somme de contrôle, sortie de sonde, transcription proxy Try it, captures d'écran de télémétrie, résumé de commentaires) архивирован в `artifacts/docs_preview/W1/`.

## Следующие шаги

- Подготовить план communauté d'admission W2 (approbation de gouvernance + modèle de demande de правки).
- Afficher la balise d'artefact d'aperçu pour les versions W2 et afficher le script de contrôle en amont après la finalisation des données.
- Les présentations de W1 dans la feuille de route/le statut, la vague de la communauté a présenté des recommandations actuelles.