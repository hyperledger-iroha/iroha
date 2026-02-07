---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ランブックインデックス
タイトル: オペラドールのランブックの説明
サイドバー ラベル: Runbook のインデックス
説明: SoraFS migrados を実行するオペラドール用の OS Runbook を実行できます。
---

> `docs/source/sorafs/runbooks/` に関する応答の登録を参照してください。
> SoraFS の新しいオペラの情報を公開するための開発者はいません
> ポータルを構築します。

Runbook の検証ページを使用して、ドキュメントの移行を完了します。
オルタナティブパラオポータル。責任を持って、自分自身の行動を報告する必要があります
ポータルでは、ベータ版を使用せずに、継続的なアップデートを行うことができます。

## ホスト ド プレビア ベータ版

DocOps を事前ベータ版のホストへの移行を促進し、承認をペロスで改訂する
`https://docs.iroha.tech/`。運用手順書の移行を指示するオペラドールと改訂者、
スナップショットを使用するためにホスト名を参照し、ポータルのプロテクションとチェックサムを実行します。
公開/ロールバックの手順
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)。

|ランブック |応答 |コピアのポータル |フォンテ |
|-------|------|------|------|
|ゲートウェイと DNS のキックオフ |ネットワーキング TL、運用自動化、ドキュメント/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
| SoraFS を実行するためのプレイブック |ドキュメント/開発リリース | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
|容量の調整 |財務 / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
|ピンを登録する操作 |ツーリングWG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
|オペラのチェックリスト | SRE ストレージ チーム | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
|紛争と調査の実行手順書 |ガバナンス評議会 | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
|ステージングのマニフェストのハンドブック |ドキュメント/開発リリース | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observabilidade da âncora Taikai |メディアプラットフォームWG / DAプログラム / ネットワーキングTL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## 検証用チェックリスト

- [x] ポータルを構築し、横方向にポータルを構築します。
- [x] Cada Runbook migrado lista o caminho de origem canônico para manter OS revisores
  ドキュメントの改訂としてのアリニャドス・デュランテ。
- [x] DocOps ブロックのパイプラインが、Quando um Runbook listado estiver をマージします
  オーセンテ・ダ・サイダ・ド・ポータル。

未来の移行 (例、政府の最新シミュレーション) 開発
DocOps を使用してチェックリストを作成し、実際に使用できるようになりました
`docs/examples/docs_preview_request_template.md`。