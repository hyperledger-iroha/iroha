---
lang: ja
direction: ltr
source: docs/portal/i18n/ru/docusaurus-plugin-content-docs/current/sorafs/migration-ledger.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8a3b53c1f20a438b60760d2b9e0938a251dea6b6f064f52ba73483ee93fe7e9c
source_last_modified: "2026-01-04T10:50:53+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Russian (ru) translation. Replace this content with the full translation. -->

---
id: migration-ledger
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


> Адаптировано из [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Журнал миграции SoraFS

Этот журнал отражает лог изменений миграции, зафиксированный в RFC архитектуры
SoraFS. Записи сгруппированы по вехам и показывают окно действия, затронутые команды
и требуемые действия. Обновления плана миграции ДОЛЖНЫ менять эту страницу и RFC
(`docs/source/sorafs_architecture_rfc.md`), чтобы держать downstream-потребителей
в согласовании.

| Веха | Окно действия | Сводка изменений | Затронутые команды | Действия | Статус |
|------|--------------|-----------------|--------------------|----------|--------|
| M1 | Недели 7–12 | CI принуждает детерминированные fixtures; alias proofs доступны в staging; tooling показывает явные expectation flags. | Docs, Storage, Governance | Убедиться, что fixtures остаются подписанными, зарегистрировать aliases в staging registry, обновить release checklists с требованием `--car-digest/--root-cid`. | ⏳ Ожидается |

Протоколы контрольного плана governance, ссылающиеся на эти вехи, находятся в
`docs/source/sorafs/`. Команды должны добавлять датированные пункты под каждой строкой
при возникновении заметных событий (например, новые регистрации alias, ретроспективы
инцидентов registry), чтобы предоставить аудируемый след.

## Недавние обновления

- 2025-11-01 — `migration_roadmap.md` разослан совету governance и спискам операторов
  для ревью; ожидается утверждение на следующей сессии совета (ref:
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — ISI регистрации Pin Registry теперь применяет совместную валидацию
  chunker/политики через helpers `sorafs_manifest`, сохраняя on-chain пути
  согласованными с проверками Torii.
- 2026-02-13 — В журнал добавлены фазы rollout provider advert (R0–R3) и опубликованы
  соответствующие dashboards и операторское руководство
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).
