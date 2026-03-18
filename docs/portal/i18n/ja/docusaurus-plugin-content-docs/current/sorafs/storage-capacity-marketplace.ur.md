---
lang: ja
direction: ltr
source: docs/portal/docs/sorafs/storage-capacity-marketplace.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
id: ストレージ容量マーケットプレイス
title: SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس
サイドバーラベル: ٩یپیسٹی مارکیٹ پلیس
説明: کیپیسٹی مارکیٹ پلیس، 複製命令 ٹیلی میٹری اور گورننس ہکس کے لیے SF-2c پلان۔
---

:::note メモ
یہ صفحہ `docs/source/sorafs/storage_capacity_marketplace.md` کی عکاسی کرتا ہے۔ جب تک پرانا ڈاکیومنٹیشن فعال ہے دونوں لوکیشنز کو ہم آہنگ رکھیں۔
:::

# SoraFS اسٹوریج کیپیسٹی مارکیٹ پلیس (SF-2c ڈرافٹ)

SF-2c セキュリティ セキュリティ セキュリティ プロバイダー コミットメント キャパシティー複製の注文 ہیں، 利用可能性 利用可能性 手数料یہ دستاویز پہلی ریلیز کے لیے درکار 成果物 کا دائرہ کار طے کرتی ہے اور انہیں 実用的なトラック میںありがとうございます

## 大事

- プロバイダーの容量コミットメント (バイト数、レーン、制限、有効期限) と管理、管理、ガバナンス、SoraNet トランスポートの管理Torii 認証済み
- 宣言された容量、ステーク、ポリシー制約、ピン、プロバイダー、決定論的動作、および決定的動作
- ストレージの配信 (レプリケーションの成功、稼働時間、整合性の証明)、料金の分配、テレメトリのエクスポート、
- 取り消し、紛争、不誠実なプロバイダー、罰則、削除、削除。

## और देखें

|コンセプト |説明 |初期成果物 |
|----------|---------------|----------|
| `CapacityDeclarationV1` | Norito ペイロード、プロバイダー ID、チャンカー プロファイルのサポート、コミットされた GiB、レーン固有の制限、価格設定のヒント、ステーキング コミットメント、有効期限、期限切れ| `sorafs_manifest::capacity` スキーマ + バリデーター|
| `ReplicationOrder` |ガバナンス 管理 命令 マニフェスト CID プロバイダー 割り当て 冗長性レベル SLA メトリクス| Torii セキュリティ Norito スキーマ + スマート コントラクト API۔ |
| `CapacityLedger` |オンチェーン/オフチェーンのレジストリ、アクティブ キャパシティの宣言、レプリケーションの注文、パフォーマンス メトリクス、発生手数料の計算|スマート コントラクト モジュール オフチェーン サービス スタブ 決定論的スナップショット|
| `MarketplacePolicy` |ガバナンス ポリシー、ミニマム ステーク、監査要件、ペナルティ カーブなど| `sorafs_manifest` 構成構造体 + ガバナンス ドキュメント|

### 実装されたスキーマ (ステータス)

## और देखें

### 1. スキーマとレジストリ層

|タスク |所有者 |メモ |
|------|----------|------|
| `CapacityDeclarationV1`、`ReplicationOrderV1`、`CapacityTelemetryV1` |ストレージ チーム / ガバナンス | Norito 認証済みセマンティック バージョニング 機能参照|
| `sorafs_manifest` パーサー + バリデーター モジュール|ストレージチーム |モノトニック ID、キャパシティ境界、ステーク要件|
|チャンカー レジストリ メタデータ میں ہر プロファイル کے لیے `min_capacity_gib` شامل کریں۔ |ツーリングWG |クライアントのプロファイル 最小ハードウェア要件 必要なハードウェア要件|
| `MarketplacePolicy` ڈاکیومنٹ ڈرافٹ کریں جو 入場ガードレール اور ペナルティスケジュール بیان کرے۔ |ガバナンス評議会 |ドキュメントを公開する ポリシーのデフォルトを公開する|

#### スキーマ定義 (実装)- `CapacityDeclarationV1` プロバイダーの署名済み容量コミットメントのキャプチャ、正規チャンカー ハンドル、機能の参照、オプションのレーン キャップ、価格設定のヒント、有効性ウィンドウ、メタデータの確認検証 یقینی بناتا ہے کہ ステーク غیر صفر ہو، 正規ハンドル ہوں، 重複排除されたエイリアス ہوں، レーン キャップ 合計 600 GiB 会計単調な計算 ہو۔【crates/sorafs_manifest/src/capacity.rs:28】
- `ReplicationOrderV1` マニフェストでは、ガバナンス発行の割り当てと冗長性の目標、SLA しきい値、割り当てごとの保証を示します。バリデータ 正規チャンカー ハンドル 固有のプロバイダー 期限の制約 制限 時間制限 Torii レジストリ順序の取り込みکریں۔【crates/sorafs_manifest/src/capacity.rs:301】
- `CapacityTelemetryV1` エポック スナップショット (宣言された GiB と使用された GiB、レプリケーション カウンタ、稼働時間/PoR パーセンテージ) とフィードの料金分布境界チェック 宣言 パーセンテージ 0-100% میں رکھتے ہیں۔【crates/sorafs_manifest/src/capacity.rs:476】
- 共有ヘルパー (`CapacityMetadataEntry`、`PricingScheduleV1`、レーン/割り当て/SLA バリデータ) 決定論的キー検証、エラー報告、ダウンストリーム ツールの再利用、CI ダウンストリーム ツールの再利用ہیں۔【crates/sorafs_manifest/src/capacity.rs:230】
- `PinProviderRegistry` オンチェーン スナップショット `/v1/sorafs/capacity/state` 公開 Norito JSON プロバイダ宣言 手数料台帳エントリ 確定的 Norito JSON 公開پیچھے جوڑ کر۔【crates/iroha_torii/src/sorafs/registry.rs:17】【crates/iroha_torii/src/sorafs/api.rs:64】
- 検証カバレッジの正規ハンドルの強制、重複検出、レーンごとの境界、レプリケーション割り当てガード、テレメトリ範囲チェック、演習、回帰、CI の監視。 ہوں۔【crates/sorafs_manifest/src/capacity.rs:792】
- オペレータ ツール: `sorafs_manifest_stub capacity {declaration, telemetry, replication-order}` 人間が判読できる仕様、標準的な Norito ペイロード、base64 BLOB、JSON 概要、演算子 `/v1/sorafs/capacity/declare`、 `/v1/sorafs/capacity/telemetry` レプリケーション順序フィクスチャ、ローカル検証、ステージの説明【crates/sorafs_car/src/bin/sorafs_manifest_stub/capacity.rs:1】 参照フィクスチャ `fixtures/sorafs_manifest/replication_order/` (`order_v1.json`、`order_v1.to`) میں ہیں اور `cargo run -p sorafs_car --bin sorafs_manifest_stub -- capacity replication-order` سے 生成 ہوتی ہیں۔

### 2. コントロールプレーンの統合

|タスク |所有者 |メモ |
|------|----------|------|
| `/v1/sorafs/capacity/declare`、`/v1/sorafs/capacity/telemetry`、`/v1/sorafs/capacity/orders` Torii ハンドラー Norito JSON ペイロード| Torii チーム |バリデータロジックとミラーの組み合わせNorito JSON ヘルパーの再利用|
| `CapacityDeclarationV1` スナップショットとオーケストレーター スコアボードのメタデータとゲートウェイのフェッチ プランの伝播|ツーリング WG / オーケストレーター チーム | `provider_metadata` キャパシティ参照、マルチソース スコアリング レーンの制限、制限|
|レプリケーション命令 オーケストレーター/ゲートウェイ クライアント フィード 割り当て フェールオーバー ヒント フェールオーバー ヒント|ネットワーキング TL / ゲートウェイ チーム |スコアボード ビルダーのガバナンス署名付き複製命令|
| CLI ツール: `sorafs_cli` および `capacity declare`、`capacity telemetry`、`capacity orders import` および|ツーリングWG |確定的な JSON + スコアボード出力|

### 3. マーケットプレイスのポリシーとガバナンス

|タスク |所有者 |メモ |
|------|----------|------|
| `MarketplacePolicy` (最小ステーク、ペナルティ乗数、監査頻度) |ガバナンス評議会 |ドキュメントを公開する 改訂履歴をキャプチャする|
|ガバナンスフック 議会の宣言 承認 更新 取り消し|ガバナンス評議会 / スマートコントラクトチーム | Norito イベント + マニフェストの取り込み|
|ペナルティスケジュール (手数料減額、保証金削減) 遠隔測定による SLA 違反の監視|ガバナンス評議会 / 財務 | `DealEngine` 決済出力の調整|
|紛争プロセスエスカレーションマトリックス دستاویز بنائیں۔ |ドキュメント / ガバナンス |紛争ランブック + CLI ヘルパー|

### 4. メーターと料金の分配|タスク |所有者 |メモ |
|------|----------|------|
| Torii メータリング インジェスト `CapacityTelemetryV1` قبول کرنے کے لیے بڑھائیں۔ | Torii チーム | GiB 時間、PoR の成功、稼働時間の検証|
| `sorafs_node` メータリング パイプラインの注文ごとの使用率 + SLA 統計情報|ストレージチーム |レプリケーション命令、チャンカー ハンドル、整列、整列|
|決済パイプライン: テレメトリー + レプリケーション データ XOR 建ての支払い額 ガバナンス対応の概要 台帳の状態|財務/保管チーム |ディールエンジン / 財務省輸出額|
|ヘルス状態の計測 (取り込みバックログ、古いテレメトリ) ダッシュボード/アラートのエクスポート|可観測性 | SF-6/SF-7 拡張機能 Grafana パック拡張機能|

- Torii `/v1/sorafs/capacity/telemetry` `/v1/sorafs/capacity/state` (JSON + Norito) オペレーター エポック テレメトリ スナップショットを公開します。証拠の梱包を監査する検査官 正規台帳 حاصل کر سکیں۔【crates/iroha_torii/src/sorafs/api.rs:268】【crates/iroha_torii/src/sorafs/api.rs:816】
- `PinProviderRegistry` 統合 یقینی بناتی ہہ کہ レプリケーション命令 اسی エンドポイント کے ذریعے قابلِ رسائی ہوں؛ CLI ヘルパー (`sorafs_cli capacity telemetry --from-file telemetry.json`) 決定論的ハッシュ エイリアス解決 自動化の実行 テレメトリの検証/公開
- メータリング スナップショット `CapacityTelemetrySnapshot` エントリ پیدا کرتے ہیں جو `metering` スナップショット سے pinned ہوتے ہیں، اور Prometheus エクスポート `docs/source/grafana_sorafs_metering.json`すぐにインポートできる Grafana ボード フィードの管理 請求チームの GiB 時間の発生額 予測 nano-SORA 料金 SLA 準拠 リアルタイムの管理سکیں۔【crates/iroha_torii/src/routing.rs:5143】【docs/source/grafana_sorafs_metering.json:1】
- メータリング スムージングの有効化 スナップショット `smoothed_gib_hours` `smoothed_por_success_bps` 演算子 EMA トレンド値 生のカウンターガバナンスへの支払い額の計算結果 [crates/sorafs_node/src/metering.rs:401]

### 5. 紛争および失効の処理

|タスク |所有者 |メモ |
|------|----------|------|
| `CapacityDisputeV1` ペイロード (申立人、証拠、ターゲットプロバイダー) |ガバナンス評議会 | Norito スキーマ + バリデーター|
|紛争解決のためのサポート (証拠の添付ファイル)۔ |ツーリングWG |証拠バンドル 決定論的ハッシュ یقینی بنائیں۔ |
|繰り返される SLA 違反、自動チェック (異議申し立てへの自動エスカレーション) |可観測性 |アラートしきい値とガバナンスフック|
|失効ハンドブック (猶予期間、固定データ、避難) |ドキュメント / ストレージ チーム |ポリシー ドキュメント、オペレータ ランブック、ポリシー ドキュメント|

## テストと CI の要件

- スキーマ検証ツールと単体テスト (`sorafs_manifest`)。
- 統合テストは、宣言→レプリケーション順序→メータリング→支払いをシミュレートします。
- CI ワークフロー、サンプル容量宣言/テレメトリ再生成、署名同期 (`ci/check_sorafs_fixtures.sh` 拡張、)۔
- レジストリ API の負荷テスト (10,000 のプロバイダー、100,000 の注文をシミュレート)

## テレメトリとダッシュボード

- ダッシュボードパネル:
  - プロバイダーが宣言した利用容量
  - レプリケーション注文のバックログと平均割り当て遅延
  - SLA 準拠 (稼働時間 %、PoR 成功率)
  - エポック料金の発生とペナルティ
- アラート:
  - プロバイダーのコミット容量
  - レプリケーション順序 SLA がスタックしている
  - パイプライン障害の計測

## ドキュメントの成果物

- 容量の宣言、コミットメントの更新、使用率の確認、オペレーター ガイドの確認
- 宣言の承認 命令の承認 紛争の処理 ガバナンス ガイド
- キャパシティ エンドポイントとレプリケーション順序の形式、API リファレンス
- 開発者向けマーケットプレイスに関するよくある質問

## GA 準備チェックリスト

**SF-2c** 会計、紛争処理、オンボーディング、オンボーディング、生産ロールアウト、アーティファクトの処理 受け入れ基準の実装 同期の処理### 夜間の会計と XOR 調整
- 容量状態のスナップショット XOR 台帳のエクスポート ウィンドウの詳細:
  ```bash
  python3 scripts/telemetry/capacity_reconcile.py \
    --snapshot artifacts/sorafs/capacity/state_$(date +%F).json \
    --ledger artifacts/sorafs/capacity/ledger_$(date +%F).ndjson \
    --label nightly-capacity \
    --json-out artifacts/sorafs/capacity/reconcile_$(date +%F).json \
    --prom-out "${SORAFS_CAPACITY_RECONCILE_TEXTFILE:-artifacts/sorafs/capacity/reconcile.prom}"
  ```
  ヘルパーの行方不明/過払い解決 罰金 ゼロ以外の終了 ہے اور Prometheus テキストファイルの概要の出力 ہے۔
- `SoraFSCapacityReconciliationMismatch` アラート (`dashboards/alerts/sorafs_capacity_rules.yml` میں)
  調整指標 ギャップ ギャップ 火災 火災ダッシュボード `dashboards/grafana/sorafs_capacity_penalties.json` میں ہیں۔
- JSON 概要ハッシュ `docs/examples/sorafs_capacity_marketplace_validation/` アーカイブ アーカイブ
  ガバナンス パケット

### 論争と証拠の隠蔽
- 紛争 `sorafs_manifest_stub capacity dispute` ذریعے فائل کریں (テスト:
  `cargo test -p sorafs_car --test capacity_cli`) 正規ペイロードの数
- `cargo test -p iroha_core -- capacity_dispute_replay_is_deterministic` ペナルティスイート
  (`record_capacity_telemetry_penalises_persistent_under_delivery`) 異議を唱える 決定論的なリプレイをスラッシュする
- 証拠収集、エスカレーション `docs/source/sorafs/dispute_revocation_runbook.md` فالو کریں؛ストライキの承認と検証レポートの作成

### プロバイダーのオンボーディングと終了スモーク テスト
- 宣言/テレメトリ アーティファクト `sorafs_manifest_stub capacity ...` 再生成 申請 提出 CLI テストの再生 (`cargo test -p sorafs_car --test capacity_cli -- capacity_declaration`)۔
- Torii (`/v1/sorafs/capacity/declare`) パスワードを送信する `/v1/sorafs/capacity/state` アドレス Grafana スクリーンショットをキャプチャする`docs/source/sorafs/capacity_onboarding_runbook.md` 終了フロー فالو کریں۔
- 署名済みアーティファクトと調整出力 `docs/examples/sorafs_capacity_marketplace_validation/` アーカイブ

## 依存関係とシーケンス

1. SF-2b (アドミッション ポリシー) - マーケットプレイスで精査されたプロバイダー
2. Torii の統合 スキーマ + レジストリ層 (別名)
3. 支払いにより、メータリング パイプラインが有効になります。
4. ステージング、測定データの検証、ガバナンス管理された料金配分の有効化

進捗状況 ロードマップ دستاویز کے حوالے کے ساتھ ٹریک کریں۔ (スキーマ、コントロール プレーン、統合、メータリング、紛争処理) 機能が完成しました。 ロードマップが完成しました。