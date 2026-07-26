---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-runbooks
title: Индекс операторов Runbook
Sidebar_label: Индекс модулей Runbook
описание: Канонический пункт входа для операторов Runbook SoraFS migrés.
---

> Отразите реестр ответственных, который найден в `docs/source/sorafs/runbooks/`.
> Новое руководство по эксплуатации SoraFS doit être Lié Ici dès Qu’il est publié dans
> la build du portail.

Используйте эту страницу для проверки рунбуков, чтобы завершить миграцию древовидности
наследие документов от порта. Chaque entrée indique la responsabilité, le chemin source
Canonique et la Copie Portail Afin que les Relecteurs могут быть направлены в руководство
Сухая подвеска в открытую бета-версию.

## Открытие бета-версии

Расплывчатые DocOps вызывают дискомфорт в плане бета-версии, одобренной читателями
`https://docs.iroha.tech/`. Lorsque vous dirigez des opérateurs ou des relecteurs vers un
runbook migré, reférencez ce nom d’hôte afin qu’ils консультант l’instantané du portail
протеже по контрольной сумме. Процедуры публикации/отката выполняются в
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Ранбук | Ответственный(а) | Копировать портал | Источник |
|---------|----------------|-------------|--------|
| Lancement-шлюз и DNS | Сетевой TL, автоматизация операций, документация/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Сборник инструкций по эксплуатации SoraFS | Документы/Разработчики | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Согласование дееспособности | Казначейство / СРИ | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Операции по регистрации булавок | Инструментальная рабочая группа | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Контрольный список по использованию ресурсов | Группа хранения, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Runbook судебные разбирательства и отзыв | Совет управления | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Сборник манифестов постановки | Документы/Разработчики | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдательность в Анкре Тайкай | Рабочая группа по медиа-платформе / Программа DA / Сеть TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Контрольный список проверки

- [x] La build du portail renvoie vers cet index (entrée de la barre latérale).
- [x] Chaque runbook migré liste le chemin source canonique for garder les relecteurs
  соответствует обзорам документации.
- [x] Конвейер обработки DocOps блокирует слияния в списке команд Runbook, который находится в другом месте.
  Вылазка дю Портей.

Будущее миграции (например, новые упражнения по хаосу или приложениям к управлению)
doivent ajouter une ligne au tableau ci-dessus et mettre à jour la checklist DocOps intégrée dans
`docs/examples/docs_preview_request_template.md`.