---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

---
id: runbooks-index
title: Индекс операторских ранбуков
sidebar_label: Индекс ранбуков
description: Каноническая точка входа для мигрированных операторских ранбуков SoraFS.
---

> Отражает реестр владельцев, который находится в `docs/source/sorafs/runbooks/`.
> Каждое новое руководство по операциям SoraFS должно быть связано здесь после публикации в
> сборке портала.

Используйте эту страницу, чтобы проверить, какие ранбуки завершили миграцию из устаревшего
дерева документации в портал. Каждая запись содержит владельцев, канонический путь источника
и копию в портале, чтобы ревьюеры могли сразу перейти к нужному руководству во время бета‑превью.

## Хост бета‑превью

Волна DocOps уже повысила одобренный ревьюерами хост бета‑превью на `https://docs.iroha.tech/`.
Когда направляете операторов или ревьюеров к мигрированному ранбуку, используйте это имя
хоста, чтобы они работали со снимком портала, защищённым контрольной суммой. Процедуры
публикации/отката находятся в
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).

| Ранбук | Владельцы | Копия в портале | Источник |
|--------|-----------|-----------------|----------|
| Запуск gateway и DNS | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Плейбук операций SoraFS | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Сверка ёмкости | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Операции реестра пинов | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Чеклист операций узла | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Ранбук споров и отзывов | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Плейбук staging‑манифестов | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдаемость якоря Taikai | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Чеклист проверки

- [x] Сборка портала ссылается на этот индекс (элемент боковой панели).
- [x] Каждый мигрированный ранбук указывает канонический путь источника, чтобы держать
  ревьюеров согласованными во время ревизии документации.
- [x] Пайплайн предварительного просмотра DocOps блокирует слияния, когда перечисленный
  ранбук отсутствует в выводе портала.

Будущие миграции (например, новые хаос‑дрили или приложения по управлению) должны добавить
строку в таблицу выше и обновить чеклист DocOps, встроенный в
`docs/examples/docs_preview_request_template.md`.
