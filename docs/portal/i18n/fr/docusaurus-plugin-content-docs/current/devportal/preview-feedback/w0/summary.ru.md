---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-feedback/w0/summary.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : aperçu-feedback-w0-summary
titre : Сводка отзывов на середине W0
sidebar_label : Отзывы W0 (середина)
description : Contrôlez les tâches, les tâches et les tâches pour les responsables principaux des versions d'aperçu.
---

| Пункт | Détails |
| --- | --- |
| Volna | W0 - mainteneurs principaux |
| Données sur les eaux | 2025-03-27 |
| Окно ревью | 2025-03-25 -> 2025-04-08 |
| Участники | docs-core-01, sdk-rust-01, sdk-js-01, sorafs-ops-01, observabilité-01 |
| Objets d'art | `preview-2025-03-24` |

## Moments agréables

1. **Somme de contrôle du flux de travail** - Les réviseurs ont répondu par `scripts/preview_verify.sh`
   успешно прошел против общей пары descripteur/archive. Ручные override non
   потребовались.
2. **Навигационный фидбек** - Les détails du problème apparaissent dans la barre latérale
   (`docs-preview/w0 #1-#2`). Обе направлены в Docs/DevRel и не блокируют
   волну.
3. **Le runbook SoraFS** - sorafs-ops-01 est utilisé dans tous les domaines
   avec `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. Заведен
   problème de suivi ; решить до W1.
4. **Ревью телеметрии** - observability-01 подтвердил, что `docs.preview.integrity`,
   `TryItProxyErrors` et le processus de connexion Try-it оставались зелеными; alertes non
   срабатывали.

## Пункты действий| ID | Description | Владелец | Statut |
| --- | --- | --- | --- |
| W0-A1 | Ouvrez la barre latérale devportal pour que vous puissiez consulter les documents des réviseurs (`preview-invite-*` en vous connectant à votre site). | Docs-core-01 | Завершено - sidebar теперь показывает документы подряд (`docs/portal/sidebars.js`). |
| W0-A2 | Ajoutez votre lien croisé entre `sorafs/orchestrator-ops` et `sorafs/multi-source-rollout`. | Sorafs-ops-01 | Il est vrai que le Runbook est conçu pour les utilisateurs, les opérateurs qui sont présents lors du déploiement. |
| W0-A3 | Поделиться телеметрическими снимками + bundle fourni avec le tracker de gouvernance. | Observabilité-01 | Il s'agit d'un bundle fourni avec `DOCS-SORA-Preview-W0`. |

## Ce résumé (2025-04-08)

- Vous avez des avis sur les avis, les commentaires des clients locaux et ceux que vous avez trouvés
  окна aperçu; les faits ont été fournis par `DOCS-SORA-Preview-W0`.
- Les alertes et les alertes concernant des personnes qui ne le sont pas ; télémétrie des frontières
  зелеными весь период.
- Demandes de navigation + liens croisés (W0-A1/A2) réalisés et ajoutés à la documentation
  выше; Le document télémétrique (W0-A3) est utilisé avec le tracker.
- Les archives documentaires sur la sécurité : les écrans télémétriques, les options de configuration et
  Il s'agit d'un outil de suivi des problèmes.

## Следующие шаги- Выполнить пункты действий W0 avant le démarrage W1.
- Mettez en place votre agencement personnel et votre emplacement de mise en scène pour le processus, afin de le faire fonctionner.
  contrôle en amont pour tous les partenaires, décrit dans [flux d'invitation d'aperçu] (../../preview-invite-flow.md).

_Эта сводка связана из [aperçu des invitations] (../../preview-invite-tracker.md), ici
сохранять трассируемость feuille de route DOCS-SORA._