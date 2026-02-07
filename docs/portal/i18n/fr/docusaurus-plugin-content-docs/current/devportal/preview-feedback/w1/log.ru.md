---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w1/log.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w1-log
titre : Лог отзывов и телеметрии W1
sidebar_label : Journal W1
description: Liste nationale, points de contrôle télémétriques et réviseurs pour les premiers partenaires de prévisualisation.
---

Ce journal contient la liste des points de contrôle télémétriques et les évaluateurs sélectionnés pour
**aperçu des partenaires W1**, сопровождающей задачи приема в
[`preview-feedback/w1/plan.md`](./plan.md) et installez les vols de trek dans
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Обновляйте его, когда отправлено приглашение,
Les instantanés télémétriques ou le triage ont été trouvés à la suite de critiques, parmi lesquels les réviseurs de gouvernance
воспроизвести доказательства без поиска внешних тикетов.

## Рoster когорты| ID du partenaire | Billet d'entrée | NDA publié | Affichage automatique (UTC) | Accusé de réception/connexion initiale (UTC) | Statut | Première |
| --- | --- | --- | --- | --- | --- | --- |
| partenaire-w1-01 | `DOCS-SORA-Preview-REQ-P01` | ✅ 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | ✅ Date du 26/04/2025 | sorafs-op-01 ; сфокусирован на доказательствах parité для orchestrator docs. |
| partenaire-w1-02 | `DOCS-SORA-Preview-REQ-P02` | ✅ 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | ✅ Date du 26/04/2025 | sorafs-op-02 ; проверил liens croisés Norito/télémétrie. |
| partenaire-w1-03 | `DOCS-SORA-Preview-REQ-P03` | ✅ 04/04/2025 | 2025-04-12 15:06 | 2025-04-12 15:18 | ✅ Date du 26/04/2025 | sorafs-op-03 ; провел exercices de basculement multi-sources. |
| partenaire-w1-04 | `DOCS-SORA-Preview-REQ-P04` | ✅ 04/04/2025 | 2025-04-12 15:09 | 2025-04-12 15:21 | ✅ Date du 26/04/2025 | torii-int-01 ; ревью livre de recettes Torii `/v1/pipeline` + Essayez-le. |
| partenaire-w1-05 | `DOCS-SORA-Preview-REQ-P05` | ✅ 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | ✅ Date du 26/04/2025 | torii-int-02 ; участвовал в обновлении скриншота Essayez-le (docs-preview/w1 #2). |
| partenaire-w1-06 | `DOCS-SORA-Preview-REQ-P06` | ✅ 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | ✅ Date du 26/04/2025 | sdk-partenaire-01 ; commentaires sur le livre de recettes JS/Swift + contrôles d'intégrité pour le pont ISO. || partenaire-w1-07 | `DOCS-SORA-Preview-REQ-P07` | ✅ 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | ✅ Date du 26/04/2025 | sdk-partenaire-02 ; conformité закрыт 2025-04-11, focus sur заметках Connect/télémétrie. |
| partenaire-w1-08 | `DOCS-SORA-Preview-REQ-P08` | ✅ 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | ✅ Date du 26/04/2025 | passerelle-ops-01 ; аудит ops гайда gateway + анонимизированный поток Essayez-le proxy. |

Заполните **Приглашение отправлено** и **Ack** сразу после отправки письма.
Connectez-vous à l'heure UTC pour rejoindre l'avion W1.

## Postes de contrôle télémétriques

| Londres (UTC) | Tableaux de bord / sondes | Владелец | Résultat | Artefact |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals` | Docs/DevRel + Ops | ✅ Tout le monde | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | Transcription `npm run manage:tryit-proxy -- --stage preview-w1` | Opérations | ✅ Suivi | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | Nous vous proposons + `probe:portal` | Docs/DevRel + Ops | ✅ Instantané de pré-invitation, sans inscription | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | Дашbordы выше + diff по латентности Essayez-le proxy | Responsable Docs/DevRel | ✅ Vérification à mi-parcours (0 alerte; latente Essayez-le p95 = 410 ms) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | Дашbordы выше + sonde de sortie | Docs/DevRel + liaison gouvernance | ✅ Quittez l'instantané, aucune alerte n'est activée | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |Horaires de bureau des travailleurs (2025-04-13 -> 2025-04-25) selon NDJSON + PNG exports под
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` avec les noms des familles
`docs-preview-integrity-<date>.json` et les étiquettes correspondantes.

## Journal des problèmes et des problèmes

Utilisez ce tableau pour résumer les réviseurs. Recherchez l'élément principal sur GitHub/discuss
billet et formulaire structuré, disponible ici
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| Référence | Gravité | Propriétaire | Statut | Remarques |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | Faible | Docs-core-02 | ✅ Résolu le 18/04/2025 | Уточнены формулировка навигации Essayez-le + cliquez sur la barre latérale (`docs/source/sorafs/tryit.md` обновлен новым étiquette). |
| `docs-preview/w1 #2` | Faible | Docs-core-03 | ✅ Résolu le 19/04/2025 | Обновлены скриншот Essayez-le et suivez-le ; artefact `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | Informations | Responsable Docs/DevRel | 🟢 Fermé | Les commentaires ont été entièrement consacrés aux questions et réponses ; Les composants du formulaire de contact sont le module `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## Contrôle des connaissances et enquêtes

1. Запишите результаты quiz (цель >=90%) для каждого reviewer ; Прикрепите экспорт CSV рядом с артефактами приглашений.
2. Suivez les résultats de l'enquête, répondez aux commentaires sous forme de formulaire et analysez votre situation.
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. Planifiez les tâches de remédiation pour les techniciens qui ne sont pas en mesure de le faire et supprimez-les dans cette zone.Tous les évaluateurs ont obtenu >=94 % lors de la vérification des connaissances (CSV :
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). l'assainissement n'est pas possible ;
enquête sur les exportations pour chaque partenaire
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## Recherche d'artéfacts

- Descripteur/somme de contrôle de l'aperçu du bundle : `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- Sonde récapitulative + link-check : `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Лог изменений Essayez-le proxy : `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- Exports télémétrie : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- Forfait télémétrie journalier aux heures de bureau : `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- Exportations de commentaires + d'enquêtes : envoi de documents par période d'évaluation
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- Contrôle des connaissances CSV et résumé : `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Vous pouvez inventer une synchronisation avec le problème du voyageur. При копировании ARTEFACTOVS в тикет gouvernance
N'oubliez pas que les auditeurs peuvent vérifier facilement sans shell-stop.