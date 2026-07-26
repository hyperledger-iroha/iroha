---
lang: es
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: índice de runbooks
título: Индекс операторских ранбуков
sidebar_label: Индекс ранбуков
descripción: Каноническая точка входа для мигрированных операторских ранбуков SoraFS.
---

> Introduzca el código de fuente de alimentación en `docs/source/sorafs/runbooks/`.
> Una nueva versión de la operación SoraFS debe conectarse después de publicaciones en
> сборке портала.

Используйте эту страницу, чтобы проверить, какие ранбуки завершили миграцию из устаревшего
Hay documentos en el portal. Каждая запись содержит владельцев, канонический путь источника
и копию в ортале, чтобы ревьюеры могли сразу перейти к нужному руководству во время бета-превью.

## Хост бета‑превью

Todos los DocOps están disponibles en versiones beta anteriores del servidor `https://docs.iroha.tech/`.
Когда направляете операторов или ревьюеров к мигрированному ранбуку, используйте это имя
хоста, чтобы они работали со снимком портала, защищённым контрольной суммой. Procedimientos
публикации/отката находятся в
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md).| Ранбук | Владельцы | Copia en el portal | Источник |
|--------|-----------|-----------------|----------|
| Puerta de enlace de SAP y DNS | TL de redes, automatización de operaciones, documentos/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Плейбук операций SoraFS | Documentos/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Сверка ёмкости | Tesorería/SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Operaciones de registro de pinos | Grupo de Trabajo sobre Herramientas | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Чеклист операций узла | Equipo de Almacenamiento, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Ранбук споров y отзывов | Consejo de Gobierno | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| Плейбук puesta en escena-manifestantes | Documentos/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдаемость якоря Taikai | Media Platform WG / Programa DA / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Чеклист проверки

- [x] Сборка портала ссылается на этот индекс (element боковой панели).
- [x] Каждый мигрированный ранбук указывает канонический путь источника, чтобы держать
  ревьюеров согласованными во время ревизии документации.
- [x] Пайплайн предварительного просмотра DOCOPS блокирует слияния, когда перечисленный
  ранбук отсутствует в выводе портала.Будущие миграции (например, новые хаос-дрили или приложения по управлению) должны добавить
строку в таблицу выше обновить чеклист DocOps, встроенный в
`docs/examples/docs_preview_request_template.md`.