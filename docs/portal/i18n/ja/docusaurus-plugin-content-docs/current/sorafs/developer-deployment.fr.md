---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者デプロイメント
title: 開発ノート SoraFS
Sidebar_label: 導入メモ
説明: CI 対生産のパイプライン SoraFS のチェックリスト。
---

:::note ソースカノニク
:::

# デプロイメントのメモ

パッケージング SoraFS のワークフローは、決定主義を強化し、生産に必要な保守的な操作を必要とする CI 合格者を受け入れます。ゲートウェイや在庫の管理に必要なチェックリストを活用してください。

## プレボリューム

- **登録によるアライメント** — チャンカーのプロファイルとマニフェストの参照タプル `namespace.name@semver` (`docs/source/sorafs/chunker_registry.md`) を確認します。
- **入学許可** — `manifest submit` (`docs/source/sorafs/provider_admission_policy.md`) を注ぐ必要な書類および別名証明を再提出します。
- **Runbook du pin registry** — gardez `docs/source/sorafs/runbooks/pin_registry_ops.md` à portée pour les scénarios de reprise (ローテーション ダリア、エチェック デ レプリケーション)。

## 環境構成

- ゲートウェイは、アクティブなエンドポイントの証明ストリーミング (`POST /v1/sorafs/proof/stream`) を使用して、CLI の履歴を確認します。
- `iroha_config` およびヘルパー CLI (`sorafs_cli manifest submit --alias-*`) のデフォルト値のポリシー `sorafs_alias_cache` を構成します。
- Fournissez のストリーム トークン (識別子 Torii) は、秘密セキュリティの管理を通じて取得されます。
- Activez lesexporters de télémétrie (`torii_sorafs_proof_stream_*`、`torii_sorafs_chunk_range_*`) および envoyez-les vers votre stack Prometheus/OTel。

## ロールアウト戦略

1. **青/緑で表示されます**
   - `manifest submit --summary-out` はアーカイバ les réponses de Chaque ロールアウトを利用します。
   - 容量の不一致を監視する `torii_sorafs_gateway_refusals_total` 検出器。
2. **証拠の検証**
   - Traitez les échecs de `sorafs_cli proof stream` comme des bloqueurs de déploiement ;スロットリングの遅延や階層のマル構成の写真を撮ります。
   - `proof verify` doit Faire party duスモークテスト後のピンを安全に保証するために、CAR hébergé par les fournisseurs はマニフェストのトゥージュールに対応します。
3. **テレメトリのダッシュボード**
   - Importez `docs/examples/sorafs_proof_streaming_dashboard.json` と Grafana。
   - ピン レジストリ (`docs/source/sorafs/runbooks/pin_registry_ops.md`) とチャンク範囲の統計情報。
4. **マルチソースのアクティベーション**
   - `docs/source/sorafs/runbooks/multi_source_rollout.md` でロールアウトの進行状況を確認し、オーケストラの活性化、アーティファクトのスコアボード/監査のアーカイブを行います。

## 事件の発生

- Suivez les chemins d'escalade dans `docs/source/sorafs/runbooks/` :
  - `sorafs_gateway_operator_playbook.md` ゲートウェイのパンとストリーム トークンを注ぎます。
  - `dispute_revocation_runbook.md` 複製の権利。
  - `sorafs_node_ops.md` メンテナンスを行ってください。
  - `multi_source_rollout.md` は、オーケストラのオーバーライド、ピアのブラックリスト、およびテープのロールアウトをオーバーライドします。
- API を介して PoR トラッカーの存在を確認し、パフォーマンスのパフォーマンスを評価するために、GovernanceLog の証拠と遅延の異常を登録します。

## プロシャイン・エテープ

- Intégrez l'automatisation de l'orchestrateur (`sorafs_car::multi_fetch`) lorsque l'orchestrateur マルチソース フェッチ (SF-6b) の血清。
- Suivez les misses à jour PDP/PoTR sous SF-13/SF-14 ; CLI およびドキュメントの大量のエクスポージャーと期限および段階の選択により、証明の安定性が向上します。

クイックスタートと CI の開発ノートを組み合わせたもので、パイプライン SoraFS の開発環境でのプロセスと観察可能な実験のロケールを組み合わせたものです。