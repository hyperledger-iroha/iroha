---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ストレージ容量マーケットプレイス
タイトル: 在庫容量のマーケットプレイス SoraFS
サイドバー_ラベル: 容量のあるマーケットプレイス
説明: 計画 SF-2c は、市場の容量を確保し、複製を行い、統治と統治を強化します。
---

:::note ソースカノニク
Cette ページは `docs/source/sorafs/storage_capacity_marketplace.md` を参照します。 Gardez les deux emplacements は、ドキュメントの安全性を維持するために有効です。
:::

# 容量のある在庫のマーケットプレイス SoraFS (Brouillon SF-2c)

プロバイダーのマーケットプレイスに関するロードマップの SF-2c の項目
ストックケージは、複製などの容量を保証します。
ディスポニビリテ・リヴレの手数料比例配分。 Ce文書幹部
居住可能なものは、プレミアリリースとゲレンデの実行可能性を要求します。

## 目的

- 容量の確認 (バイト数、レーンの制限、有効期限)
  sous une forme は、政府、輸送、SoraNet および Torii によって検証可能な消費者向けの製品です。
- Allouer les pins entre Provides selon la capacité déclarée、le stake et les
  政治的制約は、決定的な維持を維持することを目的としています。
- 在庫の管理 (複製の成功、稼動時間、統合の準備)
  輸出業者は手数料の分配を行います。
- 不正なプロバイダーに関する取り消しの手続きと紛争の処理
  罰則も退職者も。

## コンセプト・ド・ドメーヌ

|コンセプト |説明 |住みやすいイニシャル |
|----------|---------------|------|
| `CapacityDeclarationV1` |ペイロード Norito プロバイダーの ID、プロファイル チャンカーのサポート、GiB エンゲージメント、レーンの制限、価格設定のヒント、ステーキングと有効期限の詳細。 |スキーマ + 検証者は `sorafs_manifest::capacity` です。 |
| `ReplicationOrder` |指示は、CID のマニフェストを管理するための指定者であり、追加のプロバイダー、および SLA の基準を含みます。 |スキーマ Norito の平均 Torii + API のスマート コントラクト。 |
| `CapacityLedger` |オンチェーン/オフチェーンのレジストリは、アクティブな容量の宣言、複製の順序、パフォーマンスの評価、および手数料の累積を監視します。 |スマート コントラクトのモジュールまたはサービスのスタブ オフチェーンのアベック スナップショットを決定します。 |
| `MarketplacePolicy` |最低賭け金の決定、監査および罰則の規定を定める政治。 | `sorafs_manifest` の構成構造 + ガバナンス文書。 |

### スキーマ実装 (法令)

## デコパージュ デュ トラベル

### 1. Couche スキーマとレジストリ

|ターシュ |所有者 |メモ |
|------|----------|------|
|定義 `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1`。 |ストレージ チーム / ガバナンス |利用者 Norito ;バージョンの意味と容量の参照が含まれます。 |
| `sorafs_manifest` によるモジュール パーサー + 検証の実装。 |ストレージチーム |インポーザー ID は単調、能力の限界、賭け金の限界。 |
|チャンカー レジストリの平均 `min_capacity_gib` プロファイルのメタデータ。 |ツーリングWG |ハードウェア プロファイルの最小限の要求を課すクライアントの支援。 |
|文書 `MarketplacePolicy` のガードレールの入場と刑罰のカレンダーを確認します。 |ガバナンス評議会 |出版社は政治のデフォルトを文書化します。 |

#### スキーマの定義 (実装)- `CapacityDeclarationV1` プロバイダーごとの容量サインのエンゲージメントをキャプチャし、インクルーントはチャンカー正規表現、容量の参照、レーンごとのオプションの上限、価格設定のヒント、有効性とメタデータの制限を処理します。検証は NUL 以外のステークを保証し、canoniques のハンドル、dédupliqués のエイリアス、および GiB モノトーンの完全な互換性を確認します。【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` マニフェストと割り当ての関連付け、管理対象の管理、割り当てによる SLA および保証の保証。レジストリの Torii に対する期限前摂取の制限は、正規のハンドルの検証、プロバイダの固有の制限、および制限を課します。【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` 時代のスナップショットの有効期限 (GiB の宣言と利用率、レプリケーションの計算、稼働率/PoR) の配布料金の支払い。 0 ～ 100% の使用率を維持するための宣言と注力。【crates/sorafs_manifest/src/capacity.rs:476】
- ヘルパー パート (`CapacityMetadataEntry`、`PricingScheduleV1`、検証レーン/割り当て/SLA) 4 つの検証、決定決定、および CI およびツールに関するエラーの再利用可能レポートの作成下流。【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` は、`/v1/sorafs/capacity/state` 経由でオンチェーンのスナップショットの詳細を公開します。Norito JSON によるプロバイダーとエントリの組み合わせ宣言決定.【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- 正規のアプリケーションを処理するための検証実験、二重検査の検出、レーンごとのボーンズ検査、レプリケーションの割り当ておよびプラージュの検査の保護者による検査の即時検査。 CI.【crates/sorafs_manifest/src/capacity.rs:792】
- ツール操作: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` ペイロードの仕様ファイルを変換 Norito 正規化、BLOB Base64 および JSON を使用して操作を実行し、フィクスチャの準備を行う `/v1/sorafs/capacity/declare`、 `/v1/sorafs/capacity/telemetry` et des ordres de replication avec validation locale.【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`, `order_v1.to`) などは、`cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` 経由で生成されます。

### 2. 計画管理の統合

|ターシュ |所有者 |メモ |
|------|----------|------|
|ハンドラー Torii `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、`/v1/sorafs/capacity/orders` avec ペイロード Norito JSON。 | Torii チーム |検証論理の複製。 réutiliser les helpers Norito JSON。 |
|スナップショット `CapacityDeclarationV1` とスコアボードのメタデータ、オーケストレーター、フェッチ ゲートウェイの計画を管理します。 |ツーリング WG / オーケストレーター チーム | Étendre `provider_metadata` は、レーンごとにマルチソースのスコアリングを考慮した容量の参照を可能にします。 |
|クライアントのオーケストレーター/ゲートウェイからパイロットの割り当てとフェイルオーバーのヒントを取得して、レプリケーションを実行します。 |ネットワーキング TL / ゲートウェイ チーム |スコアボードのコンソメ デ オードレ シグネ パル ラ ガヴァナンスを作成します。 |
|ツール CLI : étendre `sorafs_cli` avec `capacity declare`、`capacity telemetry`、`capacity orders import`。 |ツーリングWG | Fournir JSON 決定 + 出撃スコアボード。 |

### 3. 政治市場と統治

|ターシュ |所有者 |メモ |
|------|----------|------|
|批准者 `MarketplacePolicy` (最低賭け金、ペナリテの乗数、監査の頻度)。 |ガバナンス評議会 |出版者はドキュメントを発行し、改訂履歴をキャプチャーします。 |
|議会は承認し、再宣言を行い、統治を行います。 |ガバナンス評議会 / スマートコントラクトチーム | Norito + マニフェストの取り込みを使用します。 |
| SLA 違反に対するペナリテ (料金の減額、保証金の削減) を実施します。 |ガバナンス評議会 / 財務 | `DealEngine` の決済結果をアライナが出力します。 |
|紛争処理とエスカレードの記録。 |ドキュメント / ガバナンス |紛争用ランブック + ヘルパー CLI。 |

### 4. 料金の計量と分配|ターシュ |所有者 |メモ |
|------|----------|------|
|計量器 Torii 注ぎ口 `CapacityTelemetryV1` の摂取量。 | Torii チーム | GiB 時間の有効性、PoR の成功、稼働時間。 |
|パイプラインの計量 `sorafs_node` によるレポーターの使用率の標準 + 統計 SLA を時間単位で測定します。 |ストレージチーム |レプリケーションのアラインメントとチャンカーの処理。 |
|決済パイプライン: 変換と XOR での支払いの複製、管理の再開、台帳の登録。 |財務/保管チーム |コネクター→ディールエンジン/財務省輸出。 |
|エクスポータのダッシュボード/アラートは、大量の計測を提供します (バックログの取り込み、古い情報)。 |可観測性 | SF-6/SF-7 のパック Grafana 参照。 |

- Torii は、`/v1/sorafs/capacity/telemetry` と `/v1/sorafs/capacity/state` (JSON + Norito) を公開し、運用状況と検査結果のスナップショットを公開します。 le ledger canonique pour Audit ouPackaging de preuves.【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- L'intégration `PinProviderRegistry` は、le meme エンドポイント経由でアクセスできる複製ソントの保証を保証します。ヘルパー CLI (`sorafs_cli capacity telemetry --from-file telemetry.json`) は、自動化の有効性と公開性を検証し、ハッシュ化の決定と解決策を提供します。
- メータリング製品のスナップショット `CapacityTelemetrySnapshot` スナップショットの修正 `metering`、および輸出 Prometheus 食品ボード Grafana プレ輸入業者 `docs/source/grafana_sorafs_metering.json` GiB 時間の累積、料金 nano-SORA プロジェクトと SLA の適合性に関する詳細な説明。【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- `smoothed_gib_hours` と `smoothed_por_success_bps` を含むスナップショットのスムージングとメーターの実行、EMA aux compteurs bruts utilisés par la gouvernance pour les payouts.[crates/sorafs_node/src/metering.rs:401]

### 5. 紛争および取り消しの申し立て

|ターシュ |所有者 |メモ |
|------|----------|------|
|ペイロード `CapacityDisputeV1` の定義 (plaignant、preuve、プロバイダー シブレ)。 |ガバナンス評議会 |スキーマ Norito + 検証者。 |
| CLI による紛争の解決と対応 (avec pièces de preuve) をサポートします。 |ツーリングWG | Assurer un hashing Deterministe du Bundle de Preuves。 |
| Ajouter des checks automatiques pour 違反 SLA repétées (紛争中の自動エスカレード)。 |可観測性 |管理と管理。 |
|失効の戦略を文書化します (猶予期間、ドネピンの退避)。 |ドキュメント / ストレージ チーム |政治に関する文書と運用手順書を作成します。 |

## テストと CI の例外

- スキーマのヌーボー検証を行う単位をテストします (`sorafs_manifest`)。
- 統合シミュレートのテスト: 宣言 → 反復順序 → 計量 → 支払い。
- ワークフロー CI は、宣言/テレメトリーの容量を確認し、署名の同期を保証します (`ci/check_sorafs_fixtures.sh`)。
- API レジストリを充電するテスト (シミュレーター 10,000 プロバイダー、100,000 注文)。

## テレメトリーとダッシュボード

- ダッシュボードのパノラマ:
  - プロバイダーごとの容量宣言と実用性。
  - 複製および割り当てのバックログ。
  - SLA に準拠 (稼働率 %、成功した PoR)。
  - 手数料とペナルティの累積。
- アラート:
  - 最小容量のプロバイダー。
  - 複製ブロック > SLA。
  - パイプラインの計量技術。

## ドキュメントのライブラリー

- 容量の管理、業務の再構築、および使用状況の監視を行うためのガイド。
- 宣言の承認、紛争の解決に関する統治ガイド。
- エンドポイントの容量と複製のフォーマットを提供する API を参照します。
- 開発者向けの FAQ マーケットプレイス。

## 一般提供準備チェックリスト

**SF-2c** のロードマップの項目、具体的なロールアウト生産の条件
互換性、紛争およびオンボーディングの解決。人工物を活用する
ci-dessous pour garder les critères d'acceptation alignés avec l'implémentation.### 夜想曲と和解 XOR の互換性
- 容量のスナップショットと元帳 XOR の輸出者、フェネトル、ピュイランサー :
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  Le ヘルパーは、Prometheus au 形式のテキストファイルをソートし、非ヌルの和解/罰金を解決します。
- `SoraFSCapacityReconciliationMismatch` (`dashboards/alerts/sorafs_capacity_rules.yml` による) 和解信号の指標を決定します。ダッシュボードは `dashboards/grafana/sorafs_capacity_penalties.json` です。
- 履歴書 JSON とハッシュ `docs/examples/sorafs_capacity_marketplace_validation/` のアーカイブ。

### 論争と斬り込みのプルーヴ
- `sorafs_manifest_stub capacity dispute` 経由の紛争の保管庫 (テスト:
  `cargo test -p sorafs_car --test capacity_cli`) ペイロードの標準を注ぎます。
- Lancer `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` et les suites de pénalité (`record_capacity_telemetry_penalises_persistent_under_delivery`) は、論争を引き起こし、マニエールの決定を拒否するものです。
- Suivre `docs/source/sorafs/dispute_revocation_runbook.md` は、preuves と l'escalade をキャプチャします。信頼関係を評価するための承認は必要ありません。

### プロバイダーとスモーク テストのオンボーディング
- `sorafs_manifest_stub capacity ...` および CLI の事前テスト (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`) を参照してください。
- Torii (`/v1/sorafs/capacity/declare`) キャプチャー `/v1/sorafs/capacity/state` 経由のスーメットレと Grafana のキャプチャー。 `docs/source/sorafs/capacity_onboarding_runbook.md` を実行します。
- 調停に関する成果物や成果物をアーカイブする
  `docs/examples/sorafs_capacity_marketplace_validation/`。

## 依存性と順序付け

1. Terminer SF-2b (politique d'admission) — プロバイダーの有効性によってマーケットプレイスが決まります。
2. 統合 Torii を使用して、スキーマ + レジストリ (CE ドキュメント) を実装します。
3. 支払いの前にパイプラインを計量するファイナライザー。
4. フィナーレの演出: ステージングでの計測検証の費用パイロットの分配をアクティブにします。

ロードマップを参照しながらドキュメントを作成し、進行状況を確認します。不可抗力セクション (スキーマ、管理計画、統合、計量、紛争の発生) に関するロードマップの計画が完了しました。