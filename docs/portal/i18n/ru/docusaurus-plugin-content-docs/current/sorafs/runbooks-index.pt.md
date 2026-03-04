---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-runbooks
title: Índice de runbooks deoperadores
Sidebar_label: Список модулей Runbook
описание: Канонический переход для рабочих книг операций SoraFS migrados.
---

> Зарегистрируйтесь в реестре ответов, который указан под номером `docs/source/sorafs/runbooks/`.
> Новое руководство по эксплуатации SoraFS должно быть использовано для публикации
> построить портал.

Используйте эту страницу для проверки Runbooks и перехода к поиску документов.
Альтернатива для портала. Cada entrada lista a responsabilidade, or caminho de origem canônico
И у нас нет портала, который позволит вам внести правки прямо в нужное место во время предыдущей бета-версии.

## Хост предыдущей бета-версии

Он продвигает DocOps или хост предварительной бета-версии, одобренный для внесения в них изменений.
`https://docs.iroha.tech/`. В качестве руководства для операций или изменений для мигрированного runbook,
Ссылка — это имя хоста для использования или снимка портала, защищающего контрольную сумму.
Порядок публикации/отката остался на месте
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Ранбук | Ответ(есть) | Копия без портала | Фонте |
|---------|-----------------|-----------------|-------|
| Начало шлюза и DNS | Сетевой TL, автоматизация операций, документация/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Пособие по эксплуатации для SoraFS | Документы/Разработчики | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Согласование возможностей | Казначейство / СРИ | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Операции по регистрации булавок | Инструментальная рабочая группа | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Контрольный список текущих операций | Группа хранения, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Сборник споров и отзывов | Совет управления | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Пособие по постановочному манифесту | Документы/Разработчики | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдение за временем Тайкай | Рабочая группа по медиа-платформе / Программа DA / Сеть TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Контрольный список проверки

- [x] Сборка портала для этого указателя (вход на боковой барьер).
- [x] Cada runbook перенесенный список или исходный канонический список для внесения изменений
  в течение длительного времени вносить изменения в документацию.
- [x] Предварительный конвейер блокировки DocOps объединяет то, что есть в списке Runbook.
  откройте портал.

Будущие миграции (например, новые модели управления хаосом или приложениями к управлению) разрабатываются
добавить строку в таблицу и настроить контрольный список для внедрения DocOps
`docs/examples/docs_preview_request_template.md`.