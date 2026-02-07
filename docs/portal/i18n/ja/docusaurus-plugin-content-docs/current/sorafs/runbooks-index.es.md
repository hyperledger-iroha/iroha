---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ランブックインデックス
タイトル: オペラドールのランブックの説明
サイドバー ラベル: Runbook のインデックス
説明: SoraFS migrados のオペラドールの Runbook を実行できます。
---

> `docs/source/sorafs/runbooks/` で生きる責任を登録します。
> SoraFS の新しい操作方法は、公共の場で公開されています
> ポータルを構築します。

米国の移行手順を検証するためのページを作成し、移行を完了します
アルボル・デ・ドキュメンタシオン・ヘレダド・アル・ポータル。数え切れないほどの列挙、ラ
プエダンのポータルでのオリジナルのオリジナルとコピアのルール
ベータ版の先の見通しを維持するための指示を与えます。

## ホスト デ ビスタ プレビア ベータ版

DocOps やプロモーションのホスト デ ビスタ プレビア ベータ版を改訂し、
`https://docs.iroha.tech/`。アル・ディリギルはオペラドール、あるいは国連の運用計画を改訂する。
これらのホスト名は、チェックサムを使用してポータルのインスタントなデータを参照します。
公開手順のロス/ロールバックの有効化
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)。

|ランブック |プロピエタリオ |コピア・エン・エル・ポータル |フエンテ |
|----------|----------------|--------|----------|
|ゲートウェイと DNS の配置 |ネットワーキング TL、運用自動化、ドキュメント/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS の操作ガイド |ドキュメント/開発リリース | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
|容量調整 |財務 / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
|ピン登録業務 |ツーリングWG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
|ノード操作のチェックリスト | SRE ストレージ チーム | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
|紛争と取り消しの手順書 |ガバナンス評議会 | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
|ステージングにおけるマニフィストのプレイブック |ドキュメント/開発リリース | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
|大海観測 |メディアプラットフォームWG / DAプログラム / ネットワーキングTL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## 検証チェックリスト

- [x] ポータルを構築し、横方向のポータルを構築します。
- [x] Cada Runbook は、改訂版の情報を列挙します。
  ドキュメントの改訂期間の延長。
- [x] DocOps ブロックの前のパイプラインが Runbook とクアンド ファルタをマージします
  ポータルのリスト。

Las migraciones futuras (p. ej., nuevos simulacros de caos or apéndices de gobernanza)
DocOps 組み込みのチェックリストを事前に作成し、実際に作成する
`docs/examples/docs_preview_request_template.md`。