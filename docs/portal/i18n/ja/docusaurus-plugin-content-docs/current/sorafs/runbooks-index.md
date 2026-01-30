---
id: runbooks-index
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: オペレーター向けランブック索引
sidebar_label: ランブック索引
description: 移行済み SoraFS オペレーターランブックの正規エントリーポイント。
---

> `docs/source/sorafs/runbooks/` にあるオーナー台帳を反映します。
> 新しい SoraFS 運用ガイドは、ポータルビルドに公開されたら必ずここにリンクしてください。

このページで、レガシーのドキュメントツリーからポータルに移行済みのランブックを確認できます。
各項目には担当、正規のソースパス、ポータル版が記載され、ベータプレビュー中にレビュアーが
目的のガイドへすぐに移動できます。

## ベータプレビューホスト

DocOps の波は、レビュアー承認済みのベータプレビューホストを `https://docs.iroha.tech/` に
昇格しました。移行済みランブックをオペレーターやレビュアーに案内する際は、このホスト名を
参照してチェックサムで保護されたポータルスナップショットを利用してもらってください。公開/
ロールバック手順は
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)
にあります。

| ランブック | 担当 | ポータル版 | ソース |
|-----------|------|-----------|--------|
| Gateway と DNS のキックオフ | Networking TL, Ops Automation, Docs/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS 運用プレイブック | Docs/DevRel | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
| 容量リコンシリエーション | Treasury / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
| ピンレジストリ運用 | Tooling WG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
| ノード運用チェックリスト | Storage Team, SRE | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
| 紛争・失効ランブック | Governance Council | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
| ステージング・マニフェストのプレイブック | Docs/DevRel | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Taikai アンカーの可観測性 | Media Platform WG / DA Program / Networking TL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## 確認チェックリスト

- [x] ポータルビルドからこの索引にリンクされている（サイドバー項目）。
- [x] すべての移行済みランブックが正規のソースパスを記載し、ドキュメントレビュー中の
  レビュアーの認識を揃える。
- [x] DocOps のプレビューパイプラインは、一覧にあるランブックがポータル出力にない場合は
  マージをブロックする。

将来の移行（例: 新しいカオス演習やガバナンスの付録）は、上の表に行を追加し、
`docs/examples/docs_preview_request_template.md` に埋め込まれた DocOps チェックリストを
更新してください。
