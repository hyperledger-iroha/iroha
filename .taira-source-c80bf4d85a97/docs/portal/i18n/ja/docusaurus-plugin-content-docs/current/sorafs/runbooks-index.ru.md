---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ランブックインデックス
タイトル: Индекс операторских ранбуков
サイドバーラベル: Индекс ранбуков
説明: Каноническая точка входа для мигрированных операторских ранбуков SoraFS.
---

> Отражает реестр владельцев, который находится в `docs/source/sorafs/runbooks/`.
> Каждое новое руководство по операциям SoraFS должно быть связано здесь после публикации в
> そうですね。

Используйте эту страницу, чтобы проверить, какие ранбуки заверсили миграцию из устаревлего
それは、私が知っていることです。 Каждая запись содержит владельцев, канонический путь источника
и копию в портале, чтобы ревьюеры могли сразу перейти к нужному руководству во время бета‑превью.

## Хост бета‑превью

DocOps は、`https://docs.iroha.tech/` を使用してテストされています。
Когда направляете операторов или ревьюеров к мигрированному ранбуку, используйте это имя
хоста、чтобы они работали со снимком портала、защищённым контрольной суммой. Процедуры
публикации/отката находятся в
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)。

| Ранбук | Владельцы | Копия в портале | Источник |
|----------|----------|------|----------|
|ゲートウェイとDNSの接続 |ネットワーキング TL、運用自動化、ドキュメント/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| Плейбук операций SoraFS |ドキュメント/開発リリース | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| Сверка ёмкости |財務 / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| Операции реестра пинов |ツーリングWG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| Чеклист операций узла | SRE ストレージ チーム | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| Ранбук споров и отзывов |ガバナンス評議会 | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
|ステージング‑манифестов |ドキュメント/開発リリース | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Наблюдаемость якоря Taikai |メディアプラットフォームWG / DAプログラム / ネットワーキングTL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## Чеклист проверки

- [x] Сборка портала ссылается на этот индекс (элемент боковой панели)。
- [x] Каждый мигрированный ранбук указывает канонический путь источника, чтобы держать
  Согласованными во время ревизии документации 。
- [x] Пайплайн предварительного просмотра DocOps блокирует слияния, когда перечисленный
  ранбук отсутствует выводе портала.

Будущие миграции (например, новые хаос‑дрили или приложения по управлению) должны добавить
DocOps のセキュリティとセキュリティの強化
`docs/examples/docs_preview_request_template.md`。