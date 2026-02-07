---
lang: ru
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: индекс-runbooks
Название: Индекс операторских ранбуков
sidebar_label: Индекс ранбуков
описание: Каноническая точка входа для мигрированных операторских ранбуков SoraFS.
---

> Отражает реестр владельцев, который находится в `docs/source/sorafs/runbooks/`.
> Каждое новое руководство по эксплуатации SoraFS должно быть связано здесь после публикации в
> сборка портала.

Используйте эту страницу, чтобы проверить, завершились ли какие-то ранбуки миграцией из контекста
деревянная документация на портале. каждая запись содержит владельцев, канонический путь источника
и закрепите на портале, чтобы рецензенты могли сразу перейти к нужному месту во время бета-превью.

## Хост бета‑превью

Волна DocOps уже повысила одобрение рецензентов на хосте бета-превью на `https://docs.iroha.tech/`.
Когда направляете операторов или ревьюеров к мигрированному ранбуку, воспользуйтесь этим именем
хоста, чтобы они работали со снимком портала, защищённым контрольной суммой. Процедуры
публикация/отката находится в
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Ранбук | Владельцы | Копия на портале | Источник |
|--------|-----------|-----------------|----------|
| Запуск шлюза и DNS | Сетевой TL, автоматизация операций, документация/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Плейбук операций SoraFS | Документы/Разработчики | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Сверка ёмкости | Казначейство / СРИ | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Операции реестра пинов | Инструментальная рабочая группа | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Чеклист операций узла | Группа хранения, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Ранбук споров и отзывов | Совет управления | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Плейбук постановок‑манифестов | Документы/Разработчики | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдаемость якоря Тайкай | Рабочая группа по медиа-платформе / Программа DA / Сеть TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Чеклист проверок

- [x] Сборка портала ссылается на этот индекс (элемент боковой панели).
- [x] Каждый мигрированный ранбук указывает канонический путь источника, который нужно сохранить.
  ревьюеров, согласованных во время пересмотра документации.
- [x] Пайплайн предварительное просмотр DocOps блокирует слияния, когда упоминается
  Ранбук отсутствует в выводе портала.

Будущие события (например, новые хаос‑дрили или приложения по управлению) следует добавить
текст в таблице выше и обновление контрольного списка DocOps, встроенного в
`docs/examples/docs_preview_request_template.md`.