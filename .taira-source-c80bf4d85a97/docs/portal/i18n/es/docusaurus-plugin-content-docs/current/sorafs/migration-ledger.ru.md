---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/migration-ledger.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
título: Журнал миграции SoraFS
descripción: Канонический журнал изменений, отслеживающий каждую веху миграции, владельцев и требуемые действия.
---

> Adaptado a [`docs/source/sorafs/migration_ledger.md`](https://github.com/hyperledger-iroha/iroha/blob/master/docs/source/sorafs/migration_ledger.md).

# Журнал миграции SoraFS

Este diario contiene registros de migración personalizados, actualizados en RFC Architects
SoraFS. Cómo agrupar y colocar comandos en el dispositivo
и требуемые действия. Обновления плана миграции ДОЛЖНЫ менять эту страницу и RFC
(`docs/source/sorafs_architecture_rfc.md`), чтобы держать aguas abajo-потребителей
в согласовании.

| Веха | Окно действия | Сводка изменений | Затронутые команды | Действия | Estado |
|------|--------------|-----------------|--------------------|----------|--------|
| M1 | Números 7–12 | CI принуждает детерминированные accesorios; alias pruebas доступны в puesta en escena; herramientas показывает явные banderas de expectativa. | Documentos, almacenamiento, gobernanza | Tenga en cuenta que se pueden instalar accesorios, registrar alias en el registro de preparación y actualizar listas de verificación de versiones con el sistema `--car-digest/--root-cid`. | ⏳ Ожидается |

Протоколы контрольного плана Governance, ссылающиеся на эти вехи, находятся в
`docs/source/sorafs/`. Команды должны добавлять датированные пункты под каждой строкой
при возникновении заметных событий (например, новые регистрации alias, ретроспективы
инцидентов registro), чтобы предоставить аудируемый след.## Недавние обновления

- 2025-11-01 — `migration_roadmap.md` разослан совету gobernancia y спискам операторов
  для ревью; ожидается утверждение на следующей сессии совета (ref:
  `docs/source/sorafs/council_minutes_2025-10-29.md`).
- 2025-11-02 — ISI регистрации Pin Registry теперь применяет совместную валидацию
  chunker/политики через helpers `sorafs_manifest`, сохраняя пути en cadena
  согласованными с проверками Torii.
- 2026-02-13 — В журнал добавлены фазы anuncio de proveedor de implementación (R0–R3) y опубликованы
  соответствующие paneles de control y операторское руководство
  (`provider_advert_rollout.md`, `grafana_sorafs_admission.json`).