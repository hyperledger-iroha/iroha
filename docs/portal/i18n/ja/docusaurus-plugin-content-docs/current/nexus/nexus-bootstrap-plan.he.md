---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/nexus-bootstrap-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a0351611b53b99741fff3526a13e09779cfe3aa83ea28aa8a057ba2202005bcc
source_last_modified: "2025-11-14T04:43:20.363699+00:00"
translation_last_reviewed: 2026-01-30
---

:::note 正本
このページは `docs/source/soranexus_bootstrap_plan.md` を反映しています。ローカライズ版がポータルに届くまで両方のコピーを揃えてください。
:::

# Sora Nexus ブートストラップ & 可観測性計画

## 目的
- ガバナンスキー、Torii APIs、コンセンサス監視を備えた Sora Nexus の基盤バリデータ/オブザーバネットワークを立ち上げる。
- SoraFS/SoraNet の piggyback デプロイを有効化する前にコアサービス (Torii, consensus, persistence) を検証する。
- ネットワーク健全性を確保するための CI/CD ワークフローと可観測性ダッシュボード/アラートを整備する。

## 前提条件
- ガバナンスキー素材 (評議会 multisig, 委員会キー) が HSM または Vault に用意されている。
- ベースインフラ (Kubernetes クラスタまたは bare-metal ノード) がプライマリ/セカンダリ地域にある。
- 最新のコンセンサスパラメータを反映した bootstrap 設定 (`configs/nexus/bootstrap/*.toml`) が更新済み。

## ネットワーク環境
- 異なるネットワークプレフィックスを持つ 2 つの Nexus 環境を運用する:
- **Sora Nexus (mainnet)** - 本番ネットワークプレフィックス `nexus`。カノニカルなガバナンスと SoraFS/SoraNet の piggyback サービスをホスト (chain ID `0x02F1` / UUID `00000000-0000-0000-0000-000000000753`)。
- **Sora Testus (testnet)** - ステージングのネットワークプレフィックス `testus`。統合テストと pre-release 検証のために mainnet 設定をミラー (chain UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`)。
- 各環境ごとに genesis ファイル、ガバナンスキー、インフラフットプリントを分離する。Testus は Nexus への昇格前に SoraFS/SoraNet の rollouts を検証する場として機能する。
- CI/CD パイプラインはまず Testus へデプロイし、自動 smoke tests を実行し、チェック合格後に Nexus への手動プロモーションを要求する。
- 参照用の設定 bundle は `configs/soranexus/nexus/` (mainnet) と `configs/soranexus/testus/` (testnet) にあり、それぞれにサンプル `config.toml`、`genesis.json`、Torii admission ディレクトリが含まれる。

## ステップ 1 - 設定レビュー
1. 既存ドキュメントを監査:
   - `docs/source/nexus/architecture.md` (consensus, Torii レイアウト)。
   - `docs/source/nexus/deployment_checklist.md` (インフラ要件)。
   - `docs/source/nexus/governance_keys.md` (キー保管手順)。
2. genesis ファイル (`configs/nexus/genesis/*.json`) が現在のバリデータ roster と staking weights に一致していることを確認する。
3. ネットワークパラメータを確認する:
   - コンセンサス委員会サイズと quorum。
   - ブロック間隔 / finality 閾値。
   - Torii サービスポートと TLS 証明書。

## ステップ 2 - ブートストラップクラスタのデプロイ
1. バリデータノードをプロビジョニング:
   - 永続ボリューム付きの `irohad` インスタンス (validators) をデプロイ。
   - firewall ルールがノード間の consensus と Torii トラフィックを許可することを確認。
2. 各バリデータで Torii サービス (REST/WebSocket) を TLS 付きで起動する。
3. 追加の耐障害性のため read-only の observer ノードをデプロイ。
4. bootstrap スクリプト (`scripts/nexus_bootstrap.sh`) を実行して genesis を配布し、consensus を起動し、ノードを登録する。
5. smoke tests を実行:
   - Torii 経由でテストトランザクションを送信 (`iroha_cli tx submit`)。
   - テレメトリでブロック生成/finality を検証。
   - バリデータ/observer 間の ledger 複製を確認。

## ステップ 3 - ガバナンスとキー管理
1. 評議会 multisig 設定を読み込み、ガバナンス提案が提出/承認できることを確認。
2. consensus/committee キーを安全に保管し、アクセスログ付きの自動バックアップを構成。
3. 緊急キー回転手順 (`docs/source/nexus/key_rotation.md`) をセットアップし、runbook を検証。

## ステップ 4 - CI/CD 統合
1. パイプラインを構成:
   - validator/Torii イメージのビルドと公開 (GitHub Actions または GitLab CI)。
   - 設定の自動検証 (genesis lint, signature 検証)。
   - ステージング/本番クラスタ向けのデプロイパイプライン (Helm/Kustomize)。
2. CI に smoke tests を実装 (一時クラスタを起動し、カノニカルなトランザクションスイートを実行)。
3. 失敗したデプロイ用の rollback スクリプトを追加し、runbook を文書化。

## ステップ 5 - 可観測性とアラート
1. 監視スタック (Prometheus + Grafana + Alertmanager) を地域ごとにデプロイ。
2. コアメトリクスを収集:
  - `nexus_consensus_height`, `nexus_finality_lag`, `torii_request_duration_seconds`, `validator_peer_count`.
   - Torii と consensus サービスのログを Loki/ELK 経由で収集。
3. ダッシュボード:
   - コンセンサス健全性 (ブロック高、finality、peer 状態)。
   - Torii API レイテンシ/エラー率。
   - ガバナンス取引と提案ステータス。
4. アラート:
   - ブロック生成停止 (>2 ブロック間隔)。
   - peer 数が quorum を下回る。
   - Torii エラー率の急増。
   - ガバナンス提案キューの backlog。

## ステップ 6 - 検証と引き継ぎ
1. end-to-end 検証を実行:
   - ガバナンス提案を送信 (例: パラメータ変更)。
   - 評議会承認を通してガバナンスパイプラインが機能することを確認。
   - ledger state diff を実行して整合性を確認。
2. on-call 用 runbook を文書化 (インシデント対応、failover、スケーリング)。
3. SoraFS/SoraNet チームに準備完了を共有し、piggyback デプロイが Nexus ノードを参照できることを確認。

## 実装チェックリスト
- [ ] genesis/configuration 監査が完了。
- [ ] バリデータ/observer ノードがデプロイされ、コンセンサスが健全。
- [ ] ガバナンスキーをロードし、提案をテスト。
- [ ] CI/CD パイプラインが稼働 (build + deploy + smoke tests)。
- [ ] 可観測性ダッシュボードが稼働しアラートが有効。
- [ ] handoff ドキュメントが下流チームへ提供済み。
