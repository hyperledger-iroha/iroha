---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-bootstrap-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: nexus-bootstrap-plan
title: ブートストラップと観測可能性 Sora Nexus
説明: クラスタの中央の検証のための運用計画 Nexus サービス SoraFS と SoraNet。
---

:::note ソースカノニク
Cette ページは `docs/source/soranexus_bootstrap_plan.md` を反映します。 Gardez les deux は、ポータル上に到着したバージョンのローカライズされたバージョンをコピーします。
:::

# ブートストラップと観察の計画 Sora Nexus

## 目的
- 基本検証者/観察者ソラ Nexus の管理、API Torii およびコンセンサスの監視。
- Valider les services coeur (Torii、コンセンサス、永続化) avant d'activer les deployments piggyback SoraFS/SoraNet。
- ワークフロー CI/CD およびダッシュボード/監視アラートは、安全性を保証します。

## 前提条件
- HSM および Vault に対して管理されるマテリアル (multisig du conseil、cles de comite)。
- リージョンのプライマリ/セカンダリのインフラストラクチャ (クラスタ Kubernetes またはノード ベアメタル)。
- 構成ブートストラップは 1 時間かかります (`configs/nexus/bootstrap/*.toml`) コンセンサスの詳細パラメータを参照。

## 環境調査
- 動作環境 Nexus の詳細な接頭辞の詳細:
- **Sora Nexus (メインネット)** - プレフィックス reseau deproduction `nexus`、hebergeant la gouvernance canonique et les services piggyback SoraFS/SoraNet (チェーン ID `0x02F1` / UUID) `00000000-0000-0000-0000-000000000753`)。
- **Sora Testus (テストネット)** - プレフィックス reseau de staging `testus`、メインネット構成のミロワールは、テストと統合および検証プレリリースを提供します (チェーン UUID `809574f5-fee7-5e69-bfcf-52451e42d50f`)。
- インフラストラクチャの起源、管理および運用環境を分離します。 Testus sert de terrain de preuve pour tous les rollouts SoraFS/SoraNet アバント プロモーション 対 Nexus。
- パイプライン CI/CD は、デプロイ担当者がテストを実行し、実行担当者がスモーク テストを自動化し、要求者がプロモーション マニュアルと Nexus のパスをチェックします。
- `configs/soranexus/nexus/` (メインネット) と `configs/soranexus/testus/` (テストネット) の構成および参照セットのバンドル、例 `config.toml`、`genesis.json` および入場 Torii のレパートリー。

## Etape 1 - 構成のレビュー
1. 監査担当者の文書が存在します:
   - `docs/source/nexus/architecture.md` (コンセンサス、レイアウト Torii)。
   - `docs/source/nexus/deployment_checklist.md` (緊急インフラ)。
   - `docs/source/nexus/governance_keys.md` (ギャルド・デ・クレの手順)。
2. Valider que les fichiersgenesis (`configs/nexus/genesis/*.json`) は、名簿の実際の検証とステーキングの整合性を確保します。
3. パラメータの確認:
   - コンセンサスと定足数に関する委員会。
   - ブロック間/最終戦。
   - サービス Torii および TLS 証明書のポート。

## Etape 2 - クラスターのブートストラップの展開
1. プロビジョナー レ ヌード検証者:
   - インスタンス `irohad` (検証者) の avec ボリューム永続化のデプロイ者。
   - ファイアウォールの自動トラフィック コンセンサスと Torii エントリの規則を検証します。
2. TLS を有効に検証してサービス Torii (REST/WebSocket) を解除します。
3. Deployer des noeuds observateurs (lecture seule) は、回復力の追加に注力します。
4. スクリプト実行者ブートストラップ (`scripts/nexus_bootstrap.sh`) は、ディストリビューターの生成、合意の解除、および登録の登録を行います。
5. 煙テストの実行者:
   - Torii (`iroha_cli tx submit`) によるトランザクションのテストの詳細。
   - テレメトリ経由で生産/ブロックの最終段階を検証します。
   - 検証者/監視者による台帳の複製の検証者。

## Etape 3 - 統治と統治
1. 充電器のマルチシグ・デュ・コンセイル構成;確認者は、統治者に対する提案と批准を求めます。
2. コンセンサス/コミテの安全性を保証するストッカー ド マニエール。自動的にジャーナリゼーションを行う設定者。
3. 緊急ローテーションの手順 (`docs/source/nexus/key_rotation.md`) と検証者ファイルのランブックを定めます。## Etape 4 - 統合 CI/CD
1. パイプラインの構成:
   - イメージ検証ツール/Torii のビルドと公開 (GitHub アクションまたは GitLab CI)。
   - 構成の自動検証 (lint の生成、署名の検証)。
   - デプロイメントのパイプライン (Helm/KusTOMize) はクラスターのステージングと実稼働を行います。
2. CI でのスモーク テストの実装者 (クラスタの一時性を判定する者、トランザクションの標準を実行する者)。
3. 導入レートとランブックの文書化を行うロールバックのスクリプトを作成します。

## Etape 5 - 観測可能性と警報
1. リージョンごとのスタック監視 (Prometheus + Grafana + Alertmanager) のデプロイヤー。
2.コレクター・レ・メトリック・クール：
  - `nexus_consensus_height`、`nexus_finality_lag`、`torii_request_duration_seconds`、`validator_peer_count`。
   - Loki/ELK プール サービス Torii およびコンセンサスを介したログ。
3. ダッシュボード:
   - サンテ・デュ・コンセンサス (hauteur de bloc、finalite、statut des Peers)。
   - API の遅延/エラー Torii。
   - 統治と法定の取引。
4. 警告:
   - ブロックの生産を停止します (ブロックの間隔が 2 を超える)。
   - 貴族の定足数。
   - 写真 Torii。
   - 政府の提案に関するバックログ。

## Etape 6 - 検証と引き継ぎ
1. 実行者によるエンドツーエンドの検証:
   - Soumettre une proposition de gouvernance (p. ex.changement de parametre)。
   - 統治機能のパイプラインを保証するための承認を与えるラ・フェアの可決者。
   - 執行者は、一貫性を保証する元帳の差分を作成します。
2. オンコールでのランブックの文書化 (インシデントへの対応、フェイルオーバー、スケーリング)。
3. SoraFS/SoraNet を装備する通信者。確認者は展開をピギーバック プヴァン ポインター対デ ヌード Nexus にします。

## 実装のチェックリスト
- [ ] 監査の生成/構成の終了。
- [ ] Noeuds validateurs と observateurs は、平均的なコンセンサスを維持します。
- [ ] クレス・ド・ガバナンス責任者、提案被疑者。
- [ ] パイプライン CI/CD en Marche (ビルド + デプロイ + スモーク テスト)。
- [ ] ダッシュボードは、アベック アラートを監視します。
- [ ] ダウンストリームに装備されるハンドオフライブラリのドキュメント。