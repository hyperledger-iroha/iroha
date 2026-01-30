---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/sorafs/developer-deployment.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 90fef8cfaf5551b40d6959a45e60a181d7e1fdb13914b0b8ac77f5ad3d576146
source_last_modified: "2026-01-03T18:08:03+00:00"
translation_last_reviewed: 2026-01-30
---

<!-- Auto-generated stub for Japanese (ja) translation. Replace this content with the full translation. -->

---
id: developer-deployment
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---


:::note 正規ソース
このページは `docs/source/sorafs/developer/deployment.md` を反映しています。レガシー docs が退役するまで両方を同期してください。
:::

# デプロイノート

SoraFS のパッケージングフローは決定性を強化するため、CI から本番へ移行する際に主に必要なのは運用上のガードレールです。実際の gateway とストレージプロバイダーへツールを展開する際は、このチェックリストを使ってください。

## 事前準備

- **レジストリアラインメント** — chunker プロファイルと manifests が同じ `namespace.name@semver` タプルを参照していることを確認します (`docs/source/sorafs/chunker_registry.md`)。
- **アドミッションポリシー** — `manifest submit` に必要な署名済みの provider advert と alias proof を確認します (`docs/source/sorafs/provider_admission_policy.md`)。
- **Pin registry runbook** — 復旧シナリオ（alias ローテーション、レプリケーション失敗）に備えて `docs/source/sorafs/runbooks/pin_registry_ops.md` を手元に置いておきます。

## 環境設定

- gateway は proof streaming エンドポイント（`POST /v1/sorafs/proof/stream`）を有効化し、CLI がテレメトリサマリーを出力できるようにします。
- `iroha_config` のデフォルト、または CLI ヘルパー（`sorafs_cli manifest submit --alias-*`）を使って `sorafs_alias_cache` ポリシーを設定します。
- stream token（または Torii の認証情報）は安全なシークレットマネージャで提供します。
- テレメトリエクスポーター（`torii_sorafs_proof_stream_*`, `torii_sorafs_chunk_range_*`）を有効化し、Prometheus/OTel スタックへ送信します。

## ロールアウト戦略

1. **Blue/green manifests**
   - `manifest submit --summary-out` を使って各ロールアウトのレスポンスをアーカイブします。
   - `torii_sorafs_gateway_refusals_total` を監視し、機能のミスマッチを早期に検知します。
2. **Proof 検証**
   - `sorafs_cli proof stream` の失敗はデプロイのブロッカーとして扱います。レイテンシのスパイクはプロバイダーのスロットリングや tier の誤設定を示すことがよくあります。
   - `proof verify` は post-pin のスモークテストに含め、プロバイダーがホストする CAR が manifest digest と一致していることを確認します。
3. **テレメトリダッシュボード**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` を Grafana に取り込みます。
   - pin registry の健全性（`docs/source/sorafs/runbooks/pin_registry_ops.md`）と chunk range 統計のパネルを追加します。
4. **マルチソース有効化**
   - orchestrator を有効化する際は `docs/source/sorafs/runbooks/multi_source_rollout.md` の段階的ロールアウト手順に従い、監査のために scoreboard/テレメトリのアーティファクトをアーカイブします。

## インシデント対応

- `docs/source/sorafs/runbooks/` のエスカレーションパスに従います:
  - gateway 障害と stream-token 枯渇には `sorafs_gateway_operator_playbook.md`。
  - レプリケーション争議が起きた場合は `dispute_revocation_runbook.md`。
  - ノードレベルのメンテナンスには `sorafs_node_ops.md`。
  - orchestrator の override、peer のブラックリスト化、段階的ロールアウトには `multi_source_rollout.md`。
- proof 失敗やレイテンシ異常は既存の PoR tracker API を通じて GovernanceLog に記録し、ガバナンスがプロバイダーのパフォーマンスを評価できるようにします。

## 次のステップ

- マルチソース fetch orchestrator（SF-6b）が導入されたら、orchestrator 自動化（`sorafs_car::multi_fetch`）を統合します。
- SF-13/SF-14 下の PDP/PoTR アップグレードを追跡します。これらの proofs が安定すると、CLI とドキュメントは期限と tier 選択を表面化するよう進化します。

これらのデプロイノートを quickstart と CI レシピと組み合わせることで、チームはローカル実験から本番グレードの SoraFS パイプラインへ、再現可能で可観測なプロセスで移行できます。
