---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-runbooks
title: Índice de runbooks deoperadores
Sidebar_label: Список модулей Runbook
описание: Канонический пункт ввода для рабочих книг операций SoraFS migrados.
---

> Отобразите реестр ответственных лиц, которые живут в `docs/source/sorafs/runbooks/`.
> Новое руководство по работе с SoraFS должно быть опубликовано здесь
> построить портал.

Используйте эту страницу для проверки того, что runbooks завершена миграция после этого
Сообщение с документацией передается на портал. Cada entrada enumera la titularidad, la
Первоначальный канонический путь и копия на портале, чтобы можно было пересмотреть
перейдите прямо к нужному пользователю во время предварительной бета-версии.

## Хост предварительной бета-версии

Олеада DocOps и продвижение хоста предварительной бета-версии, одобренной для пересмотров
`https://docs.iroha.tech/`. Аль-диригир операдорес или ревизия un runbook migrado,
Это имя хоста используется для мгновенной защиты портала контрольной суммой.
Процедуры публикации/отката viven en
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Ранбук | Владелец(и) | Копирование на портале | Фуэнте |
|---------|----------------|-------------------|--------|
| Организация шлюза и DNS | Сетевой TL, автоматизация операций, документация/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Сборник инструкций по работе с SoraFS | Документы/Разработчики | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Согласование возможностей | Казначейство / СРИ | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Операции по регистрации булавок | Инструментальная рабочая группа | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Контрольный список операций на сайте | Группа хранения, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Сборник споров и отзывов | Совет управления | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Сборник манифестов в постановке | Документы/Разработчики | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдение за Анкла Тайкай | Рабочая группа по медиа-платформе / Программа DA / Сеть TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Контрольный список проверки

- [x] Построить портал enlaza este índice (вход в боковой проход).
- [x] Cada runbook migrado перечисляет исходный канонический путь для сохранения изменений
  alineados во время пересмотра документации.
- [x] Конвейер просмотра предварительного просмотра DocOps блокирует слияния, когда он проваливается в runbook
  список на портале Salida del.

Las migraciones futuras (стр. ej., nuevos simulacros de caos o apéndices de gobernanza)
сначала нужно создать предыдущую таблицу и актуализировать контрольный список включенного DocOps в
`docs/examples/docs_preview_request_template.md`.