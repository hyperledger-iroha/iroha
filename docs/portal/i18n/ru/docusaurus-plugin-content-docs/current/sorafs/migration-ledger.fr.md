---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
заголовок: Регистр миграции SoraFS
описание: Канонический журнал изменений, который соответствует требованиям миграции, ответственности и необходимым требованиям.
---

> Адаптируйте [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Регистр миграции SoraFS

Ce registre reprend le Journal des Migrations capture в RFC d'architecture
SoraFS. Les entrees sont groupees par jalon et indiquent la fenetre эффективно,
оборудование, на которое будет воздействовать, и необходимые действия. Les mises a jour du plan de
Модификатор миграции DOIVENT на этой странице и в RFC
(`docs/source/sorafs_architecture_rfc.md`) для обеспечения безопасности потребителей
выравнивает.

| Джалон | Фенетре эффективен | Резюме изменений | Оснащает пострадавших | Действия | Статут |
|-------|-------------------|---------------------|------------------|---------|--------|
| М1 | Семайны 7–12 | Le CI навязывает детерминированные приспособления; les preuves d'alias, которые можно использовать в постановке; Инструменты предоставляют явные флаги внимания. | Документы, хранение, управление | Убедитесь, что все остальные устройства подписаны, зарегистрируйте псевдонимы в промежуточном реестре, ознакомьтесь с контрольными списками выпуска с `--car-digest/--root-cid`. | ⏳ Внимание |

«Минуты плана контроля над управлением, на которые ссылаются эти жалюзи, вивент су»
`docs/source/sorafs/`. Les Equipes doivent ajouter des puces datees sous chaque ligne
lorsque des Evenements Notables Surviennent (например: nouveaux enregistrements d'alias,
ретроспективы происшествий в реестре) afin de Fournir une Trace, поддающийся проверке.

## Недавние ошибки

- 01 ноября 2025 г. — Распространение `migration_roadmap.md` в совете по управлению и во многих списках.
  операторы заливают ревизию; en attente de validation lors de la prochaine session du
  совет (ссылка: suivi `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — L'ISI d'enregistrement du Pin Registry applique desormais la validation
  Partagee chunker/politique через les helpers `sorafs_manifest`, Gardant les chemins
  он-чейн выравнивает все проверки Torii.
- 13 февраля 2026 г. — Описание этапов развертывания рекламы поставщика (R0–R3) при регистрации и
  Публикация информационных панелей и руководств ассоциаций операторов
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).