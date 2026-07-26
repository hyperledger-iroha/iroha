---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/developer-deployment.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: 開発者デプロイメント
タイトル: SoraFS 導入ノート
Sidebar_label: 導入メモ
説明: CI の製造プロセス SoraFS パイプラインのプロモート チェックリスト
---

:::note メモ
:::

# 導入メモ

SoraFS パッケージング ワークフローの決定性 مضبوط کرتا ہے، اس لئے CI سے 生産 پر جانا بنیادی طور پر 運用上のガードレール مانگتا ہے۔ゲートウェイ ストレージ プロバイダー ロールアウト チェックリスト チェックリスト

## 飛行前

- **レジストリの調整** — チャンカー プロファイルのマニフェスト `namespace.name@semver` タプルは、(`docs/source/sorafs/chunker_registry.md`) を参照します。
- **アドミッション ポリシー** — `manifest submit` 署名付きプロバイダー広告、エイリアス証明、 (`docs/source/sorafs/provider_admission_policy.md`)。
- **ピン レジストリ Runbook** — 回復シナリオ (エイリアスのローテーション、レプリケーションの失敗)

## 環境設定

- ゲートウェイのプルーフ ストリーミング エンドポイント (`POST /v1/sorafs/proof/stream`) により、CLI テレメトリ サマリの出力が有効になります。
- `sorafs_alias_cache` ポリシー `iroha_config` のデフォルト CLI ヘルパー (`sorafs_cli manifest submit --alias-*`) の構成
- ストリーム トークン (Torii 資格情報) 秘密マネージャーの秘密マネージャー
- テレメトリ エクスポータ (`torii_sorafs_proof_stream_*`、`torii_sorafs_chunk_range_*`) は、Prometheus/OTel スタックの出荷を有効にします。

## ロールアウト戦略

1. **青/緑のマニフェスト**
   - ロールアウト 回答 アーカイブ 回答 `manifest submit --summary-out` 回答 回答 アーカイブ
   - `torii_sorafs_gateway_refusals_total` 機能が不一致です 機能が一致しません。
2. **証拠の検証**
   - `sorafs_cli proof stream` 失敗と展開ブロッカーの発生レイテンシのスパイク、プロバイダーのスロットリング、層の構成ミス、および問題。
   - ポストピンスモークテスト `proof verify` 検査結果 検査結果 プロバイダー ホストされた CAR 検査結果 マニフェスト ダイジェスト 検査結果 マッチ検査結果 ہے۔
3. **テレメトリ ダッシュボード**
   - `docs/examples/sorafs_proof_streaming_dashboard.json` Grafana インポートする
   - ピン レジストリの健全性 (`docs/source/sorafs/runbooks/pin_registry_ops.md`) チャンク範囲の統計情報 パネルの情報
4. **マルチソースの有効化**
   - オーケストレーターの評価 `docs/source/sorafs/runbooks/multi_source_rollout.md` 段階的なロールアウト ステップの評価 監査の評価 スコアボード/テレメトリ アーティファクトのアーカイブの評価

## インシデント処理

- `docs/source/sorafs/runbooks/` エスカレーション パス:
  - `sorafs_gateway_operator_playbook.md` ゲートウェイの停止とストリーム トークンの枯渇
  - `dispute_revocation_runbook.md` レプリケーションに関する紛争 ہوں۔
  - `sorafs_node_ops.md` ノードレベルのメンテナンス
  - `multi_source_rollout.md` オーケストレーターはピアのブラックリストをオーバーライドし、段階的ロールアウトをオーバーライドします。
- プルーフ障害、レイテンシ異常、ガバナンスログ、PoR トラッカー API、記録、ガバナンスプロバイダーのパフォーマンス評価、およびガバナンスプロバイダーのパフォーマンス評価

## 次のステップ

- マルチソース フェッチ オーケストレーター (SF-6b) とオーケストレーター自動化 (`sorafs_car::multi_fetch`) の統合
- PDP/PoTR アップグレード SF-13/SF-14 の追跡プルーフの安定化 テスト CLI ドキュメントの期限 ティア選択サーフェス テスト

導入ノート クイックスタート CI レシピ ローカル実験 プロダクショングレード SoraFS パイプライン 再現性のある観察可能なプロセスやあ