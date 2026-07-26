---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-リファクタリングプラン
title: Sora Nexus لیجر ری فیکٹر پلان
説明: `docs/source/nexus_refactor_plan.md` کا آئینہ، جو Iroha 3 کوڈ بیس کی مرحلہ وار صفائی کے کام کی تفصیل और देखें
---

:::note ٩ینونیکل ماخذ
یہ صفحہ `docs/source/nexus_refactor_plan.md` کی عکاسی کرتا ہے۔ لوکلائزڈ ایڈیشنز پورٹل تک پہنچنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# Sora Nexus 台帳リファクタリング計画

یہ دستاویز Sora Nexus Ledger ("Iroha 3") کے ری فیکٹر کے لئے فوری ロードマップ کو محفوظ کرتی ❁❁❁❁レイアウト ジェネシス/WSV ブックキーピング Sumeragi コンセンサス スマート コントラクト トリガー スナップショット クエリ ポインター ABI ホスト バインディング Norito コーデック回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰 回帰アーキテクチャを修正する モノリシック パッチを修正するありがとうございます

## 0. いいえ
- 決定論的決定的決定的決定的決定的決定的決定的決定的決定。アクセラレーション オプトイン機能フラグ 評価 フォールバック 評価 評価 オプトイン機能フラグ 評価 フォールバック 評価 評価 オプトイン機能フラグ
- Norito シリアル化レイヤー ہے۔状態/スキーマの状態/スキーマ Norito エンコード/デコードのラウンドトリップ テスト フィクスチャの更新
- 構成 `iroha_config` (ユーザー -> 実際の -> デフォルト)パスとアドホック環境の切り替え
- ABI ポリシー V1 のバージョンホスト ポインタ タイプ/システムコール 決定論的定義
- `cargo test --workspace` ゴールデン テスト (`ivm`、`norito`、`integration_tests`) マイルストーン ٩ے لئے بنیادی Gate رہیں گے۔

## 1. ٹاپولوجی اسنیپ شاٹ
- `crates/iroha_core`: Sumeragi アクター、WSV、ジェネシス ローダー、パイプライン (クエリ、オーバーレイ、ZK レーン)、スマート コントラクト ホスト グルー
- `crates/iroha_data_model`: オンチェーン データおよびクエリと権限のあるスキーマ
- `crates/iroha`: クライアント API、CLI、テスト、SDK のテスト。
- `crates/iroha_cli`: CLI の機能 `iroha` の API のミラー機能 ہے۔
- `crates/ivm`: Kotodama バイトコード VM、ポインター - ABI ホスト統合エントリー ポイント
- `crates/norito`: シリアル化コーデック、JSON アダプター、AoS/NCB バックエンド、シリアル化コーデック
- `integration_tests`: クロスコンポーネント アサーション、ジェネシス/ブートストラップ、Sumeragi、トリガー、ページネーション、ページネーション
- ドキュメント پہلے ہی Sora Nexus Ledger اہداف (`nexus.md`, `new_pipeline.md`, `ivm.md`) の実装ٹکڑوں میں ہے اور کوڈ کے مقابلے میں جزوی طور پر پرانی ہے۔

## 2. マイルストーン

### フェーズ A - 基礎と可観測性
1. **WSV テレメトリ + スナップショット**
   - `state` 正規スナップショット API (特性 `WorldStateSnapshot`) クエリ Sumeragi CLI アプリケーション
   - `scripts/iroha_state_dump.sh` ستعمال کریں تاکہ `iroha state dump --format norito` کے ذریعے 確定的スナップショット بنیں۔
2. **ジェネシス/ブートストラップ決定論**
   - ジェネシスの摂取 Norito を利用したパイプライン (`iroha_core::genesis`) の開発
   - 統合/回帰カバレッジ ジェネシス テスト テスト リプレイ テスト arm64/x86_64 テスト テスト WSV ルート テスト テスト テスト(ٹریک: `integration_tests/tests/genesis_replay_determinism.rs`)۔
3. **クレート間の固定性テスト**
   - `integration_tests/tests/genesis_json.rs` テスト、WSV パイプライン、ABI 不変条件、ハーネス、検証、検証
   - `cargo xtask check-shape` スキャフォールド متعارف کریں جو スキーマ ドリフト پر パニック کرے (DevEx ツール バックログ میں ٹریک; `scripts/xtask/README.md` کی アクション アイテム دیکھیں)۔

### フェーズ B - WSV クエリ サーフェス
1. **状態ストレージ トランザクション**
   - `state/storage_transactions.rs` トランザクション アダプターの処理 コミットの順序付け 競合検出
   - 単体テスト、アセット/ワールド/トリガー、ロールバック、ロールバック、ワールド/トリガー
2. **クエリモデルのリファクタリング**
   - ページネーション/カーソル ロジック `crates/iroha_core/src/query/` と再利用可能なコンポーネントの統合`iroha_data_model` میں Norito 表現 کو 整列 کریں۔
   - トリガー、アセット、ロール、決定論的順序付け、スナップショット クエリ、カバレッジ `crates/iroha_core/tests/snapshot_iterable.rs` میں ٹریک ہے)۔
3. **スナップショットの一貫性**
   - `iroha ledger query` CLI のスナップショット パス Sumeragi/fetchers のパス
   - CLI スナップショット回帰テスト `tests/cli/state_snapshot.rs` میں (低速実行、機能ゲート)۔### フェーズ C - Sumeragi パイプライン
1. **トポロジーとエポック管理**
   - `EpochRosterProvider` 特性の実装 WSV ステーク スナップショットの実装
   - `WsvEpochRosterAdapter::from_peer_iter` ベンチ/テスト モックフレンドリーなコンストラクター فراہم کرتا ہے۔
2. **コンセンサスフローの簡素化**
   - `crates/iroha_core/src/sumeragi/*` 説明: `pacemaker`、`aggregation`、`availability`、`witness`タイプ `consensus` タイプ تحت رکھیں۔
   - アドホック メッセージの受け渡し、入力された Norito エンベロープの確認、ビュー変更プロパティのテスト、(Sumeragi メッセージング バックログの確認)
3. **レーン/プルーフの統合**
   - レーンの証明、DA のコミットメント、整列、調整、調整、RBC ゲート、RBC ゲート
   - エンドツーエンド統合テスト `integration_tests/tests/extra_functional/seven_peer_consistency.rs` RBC 対応パスを確認する

### フェーズ D - スマート コントラクトとポインター ABI ホスト
1. **ホスト境界監査**
   - ポインター型チェック (`ivm::pointer_abi`) およびホスト アダプター (`iroha_core::smartcontracts::ivm::host`) 統合
   - ポインター テーブルの期待値 ホスト マニフェスト バインディング `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` `ivm_host_mapping.rs` ゴールデン TLV マッピング 演習
2. **実行サンドボックスのトリガー**
   - トリガー、ガス、ポインタ検証、イベント ジャーナリング、`TriggerExecutor` のトリガーいいえ
   - コール/時間トリガー 回帰テスト 失敗パス ہیں (ٹریک: `crates/iroha_core/tests/trigger_failure.rs`)۔
3. **CLI とクライアントの調整**
   - CLI 操作 (`audit`、`gov`、`sumeragi`、`ivm`) のドリフト、共有`iroha` クライアント機能
   - CLI JSON スナップショット テスト `tests/cli/json_snapshot.rs`コア コマンド出力の正規 JSON リファレンスと一致するデータ

### フェーズ E - Norito コーデック強化
1. **スキーマ レジストリ**
   - `crates/norito/src/schema/` の説明 Norito スキーマ レジストリの説明 コア データ型の説明 正規エンコーディングの説明
   - サンプル ペイロード エンコーディングを検証し、ドキュメント テストを実行します (`norito::schema::SamplePayload`)。
2. **ゴールデン フィクスチャの更新**
   - `crates/norito/tests/*` ゴールデン フィクスチャ、リファクタリング、WSV スキーマ、一致、マッチ
   - `scripts/norito_regen.sh` ヘルパー `norito_regen_goldens` 処理 Norito JSON ゴールデン 決定論的処理 再生成処理
3. **IVM/Norito の統合**
   - Kotodama マニフェストのシリアル化 Norito エンドツーエンドの検証 エンドツーエンドの検証 ABI メタデータ ポインター ABI メタデータ
   - `crates/ivm/tests/manifest_roundtrip.rs` はマニフェストを表示します。 Norito はパリティをエンコード/デコードします。

## 3. 横断的な分析
- **テスト戦略**: フェーズ単体テスト -> クレート テスト -> 統合テストテストの失敗 回帰 پکڑتے ہیں؛テスト انہیں واپس آنے سے روکتے ہیں۔
- **ドキュメント**: フェーズの説明 `status.md` 説明 アイテムの説明 `roadmap.md` 説明کریں، جبکہ مکمل شدہ کام prune کریں۔
- **パフォーマンス ベンチマーク**: `iroha_core`、`ivm`、`norito` ベンチマークリファクタリング ベースライン測定のリファクタリング 回帰分析
- **機能フラグ**: クレート レベルの切り替え、バックエンド、ツールチェーン、(`cuda`、`zk-verify-batch`)۔ CPU SIMD パス ビルド バージョン ランタイム バージョンサポートされていないハードウェアです。決定論的なスカラー フォールバックが発生しています。

## 4. فوری اگلے اقدامات
- フェーズ A の足場 (スナップショット特性 + テレメトリ配線) - ロードマップの更新 実行可能なタスク
- `sumeragi`、`state`、`ivm` 欠陥監査のハイライト表示:
  - `sumeragi`: デッド コード許可ビュー変更プルーフ ブロードキャスト、VRF リプレイ状態、EMA テレメトリ エクスポート、ガード サポートフェーズ C コンセンサス フローの簡素化 レーン/プルーフ統合成果物 ゲートゲート型の統合
  - `state`: `Cell` クリーンアップ テレメトリ ルーティング フェーズ A の WSV テレメトリ トラック SoA/並列適用フェーズ C パイプライン最適化バックログمیں شامل ہوتے ہیں۔
  - `ivm`: CUDA トグル露出、エンベロープ検証、Halo2/Metal カバレッジ、フェーズ D、ホスト境界、横断的な GPU アクセラレーション テーマ、テストカーネルの評価 専用 GPU バックログの評価
- 侵襲的なコード変更 - 侵襲的なコード変更 - 攻撃的コード変更 - クロスチーム RFC でのサインオフ## 5. いいえ
- RBC、P1、オプション、Nexus 元帳レーン、RBC、P1、オプション。ステークホルダー فیصلہ درکار ہے۔
- ٩یا ہم P1 میں DS composability groups نافذ کریں یا レーン証明 کے 成熟した ہونے تک انہیں を無効にする رکھیں؟
- ML-DSA-87 パラメータの正規の場所番号: クレート `crates/fastpq_isi` (作成保留中)

---

_آخری اپ ڈیٹ: 2025-09-12_