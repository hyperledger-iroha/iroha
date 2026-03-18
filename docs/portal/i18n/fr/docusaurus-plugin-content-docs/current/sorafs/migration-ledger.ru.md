---
lang: fr
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
titre : Migration du journal SoraFS
description: Канонический журнал изменений, отслеживающий каждую веху миграции, владельцев и требуемые действия.
---

> Adapté à [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Migration du journal SoraFS

Ceci est actuellement disponible pour les migrations de journaux, spécifiées dans l'architecture RFC.
SoraFS. Записи сгруппированы по вехам и показывают окно действия, затронутые команды
и требуемые действия. La mise à jour du plan de migration du DOS vous amène à cette page et à RFC.
(`docs/source/sorafs_architecture_rfc.md`), que dois-je faire en aval-потребителей
в согласовании.

| Веха | Окно действия | Сводка изменений | Commandes | Fête | Statut |
|------|--------------|-------|------------------------|----------|--------|
| M1 | Jours 7–12 | CI принуждает детерминированные luminaires; les preuves d'alias sont fournies dans la mise en scène ; l'outillage показывает явные les drapeaux d'attente. | Documents, stockage, gouvernance | Pour que les appareils soient disponibles, enregistrez les alias dans le registre intermédiaire et ouvrez les listes de contrôle de publication pour le travail `--car-digest/--root-cid`. | ⏳ Ожидается |

Les protocoles de contrôle du plan de gouvernance, élaborés pour chacun d'entre eux, sont en place dans
`docs/source/sorafs/`. Les commandes permettent d'obtenir des points de données à chaque heure
при возникновении заметных событий (par exemple, новые регистрации alias, rétrospectives
инцидентов registre), чтобы предоставить аудируемый след.## Nouvelles nouveautés

- 2025-11-01 — `migration_roadmap.md` разослан совету gouvernance et спискам операторов
  pour la révision; ожидается утверждение на следующей сессии совета (réf:
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — L'ISI enregistre le registre des broches lors de la validation actuelle
  chunker/polyester pour les assistants `sorafs_manifest`, pour les machines en chaîne
  согласованными с проверками Torii.
- 2026-02-13 — Au cours de la même période, annonce du fournisseur de déploiement (R0 – R3) et annonces publiques
  соответствующие tableaux de bord et opérateur
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).