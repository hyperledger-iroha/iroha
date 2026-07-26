---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/runbooks-index.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: ランブックインデックス
タイトル: 運用手順書のインデックス
Sidebar_label: Runbook のインデックス
説明: Point d’entrée canonique pour les runbook opérateurs SoraFS migrés。
---

> `docs/source/sorafs/runbooks/` に関する責任の登録を参照してください。
> Chaque nouveau guide d’exploitation SoraFS doit être lié ici dès qu’il est publié dans
> ラ・ビルド・デュ・ポルテイル。

アーボレッセンスでの移行の終了時に検証者が実行ブックを確認するためのページを利用する
ドキュメントヘリテエ対ルポルテイル。 Chaque entrée indique la responsabilité、le chemin source
canonique et la copy portail afin que les reelecteurs puissent accéder directement au guide
souhaité ペンダント l’aperçu beta。

## ホテル ダペルス ベータ

承認者の承認を得るために、漠然とした DocOps を承認する必要があります
`https://docs.iroha.tech/`。オペレーターと選出者に対する責任を負う
ランブックの移行、参照番号、およびコンサルタントのインスタンス デュ ポルテイル
チェックサムの保護者。公開/ロールバックの手順
[`devportal/preview-host-exposure`](../devportal/preview-host-exposure.md)。

|ランブック |責任者 |ポータルのコピー |出典 |
|----------|-----|---------------|----------|
|ランセメントゲートウェイとDNS |ネットワーキング TL、運用自動化、ドキュメント/DevRel | [`sorafs/gateway-dns-runbook`](./gateway-dns-runbook.md) | `docs/source/sorafs_gateway_dns_design_runbook.md` |
|悪用のハンドブック SoraFS |ドキュメント/開発リリース | [`sorafs/operations-playbook`](./operations-playbook.md) | `docs/source/sorafs/operations_playbook.md` |
|容量の調整 |財務 / SRE | [`sorafs/capacity-reconciliation`](./capacity-reconciliation.md) | `docs/source/sorafs/runbooks/capacity_reconciliation.md` |
|ピン登録操作 |ツーリングWG | [`sorafs/pin-registry-ops`](./pin-registry-ops.md) | `docs/source/sorafs/pin_registry_ops.md` |
|不正利用のチェックリスト | SRE ストレージ チーム | [`sorafs/node-operations`](./node-operations.md) | `docs/source/sorafs/runbooks/sorafs_node_ops.md` |
|訴訟と失効に関するランブック |ガバナンス評議会 | [`sorafs/dispute-revocation-runbook`](./dispute-revocation-runbook.md) | `docs/source/sorafs/dispute_revocation_runbook.md` |
|ステージングのマニフェストのハンドブック |ドキュメント/開発リリース | [`sorafs/staging-manifest-playbook`](./staging-manifest-playbook.md) | `docs/source/sorafs/staging_manifest_playbook.md` |
| Observability de l’ancre Taikai |メディアプラットフォームWG / DAプログラム / ネットワーキングTL | [`sorafs/taikai-anchor-runbook`](./taikai-anchor-runbook.md) | `docs/source/taikai_anchor_monitoring.md` |

## 検証のチェックリスト

- [x] La build du portail renvoie vers cet インデックス (entrée de la barre latérale)。
- [x] Chaque runbook migré liste le chemin source canonique pour garder les reelecteurs
  ドキュメントのレビューを調整します。
- [x] DocOps ブロックのパイプラインが lorsqu'un runbook リストとマージされ、管理者が表示されます
  出撃デュポルテイル。

移民先物 (p. ex. nouveaux exercices de Chaos または annexes de gouvernance)
Doivent ajouter une ligne au tableau ci-dessus et metre à jour la checklist DocOps intégrée dans
`docs/examples/docs_preview_request_template.md`。