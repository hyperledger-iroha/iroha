---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-リファクタリングプラン
title: План рефакторинга Sora Nexus Ledger
説明: Зеркало `docs/source/nexus_refactor_plan.md`、описывающее поэтапную зачистку кодовой базы Iroha 3.
---

:::note Канонический источник
Эта страница отражает `docs/source/nexus_refactor_plan.md`. Держите обе копии синхронизированными, пока многоязычная версия не появится в портале.
:::

# План рефакторинга Sora Nexus Ledger

ソラ Nexus レジャー ("Iroha 3") のロードマップを確認してください。 Он отражает текущую структуру репозитория и регрессии, замеченные в учете Genesis/WSV, консенсусе Sumeragi, триггерах смарт-контрактов、スナップショット クエリ、ホスト バインディング ポインター ABI および Norito コーデック。 Цель - прийти к согласованной, тестируемой архитектуре, не пытаясь посадить все исправления одним монолитнымです。

## 0. Направляющие принципы
- Сохранять детерминированное поведение на гетерогенном железе;オプトイン機能フラグとフォールバックを確認できます。
- Norito - 最高です。 Norito は、フィクスチャの状態/スキーマをラウンドトリップでエンコード/デコードします。
- Конфигурация проходит через `iroha_config` (ユーザー -> 実際の -> デフォルト)。アドホック環境を切り替えることができます。
- ABI остается V1 を使用してください。ポインター タイプ/システムコールをホストします。
- `cargo test --workspace` およびゴールデン テスト (`ivm`、`norito`、`integration_tests`) のテストが必要です。

## 1. Снимок топологии репозитория
- `crates/iroha_core`: Sumeragi アクター、WSV、ジェネシス ローダー、パイプライン (クエリ、オーバーレイ、ZK レーン)、スマート コントラクト ホストの接着剤。
- `crates/iroha_data_model`: オンチェーンのクエリを実行します。
- `crates/iroha`: クライアント API、CLI、テスト、SDK。
- `crates/iroha_cli`: CLI、API および `iroha`。
- `crates/ivm`: Kotodama バイトコード VM、エントリ ポイントとポインターと ABI ホストの統合。
- `crates/norito`: シリアル化コーデック、JSON アダプター、AoS/NCB バックエンド。
- `integration_tests`: кросс-компонентные アサーション、ジェネシス/ブートストラップ、Sumeragi、トリガー、ページネーション、т.д。
- Sora Nexus Ledger (`nexus.md`、`new_pipeline.md`、`ivm.md`) のドキュメントを参照してください。 Єрагментирована и частично устарела относительно кода.

## 2. Опорные элементы рефакторинга и вехи

### フェーズ A - 基礎と可観測性
1. **WSV テレメトリ + スナップショット**
   - 正規スナップショット API、`state` (特性 `WorldStateSnapshot`)、クエリ、Sumeragi、CLI をサポートします。
   - `scripts/iroha_state_dump.sh` 日のスナップショット `iroha state dump --format norito`。
2. **ジェネシス/ブートストラップ決定論**
   - Genesis так、чтобы он проходил через единый Norito パイプライン (`iroha_core::genesis`) を取り込みます。
   - 統合/回帰、ジェネシスの統合/回帰、WSV ルートの arm64/x86_64 の統合(трекится в `integration_tests/tests/genesis_replay_determinism.rs`)。
3. **クレート間の固定性テスト**
   - `integration_tests/tests/genesis_json.rs`、WSV、パイプライン、ABI 不変条件、ハーネスをサポートします。
   - スキャフォールド `cargo xtask check-shape`、スキーマ ドリフト (DevEx ツール バックログ、アクション アイテム `scripts/xtask/README.md`) を参照してください。

### フェーズ B - WSV とクエリ サーフェス
1. **状態ストレージ トランザクション**
   - Свести `state/storage_transactions.rs` к транзакционному адаптеру, который обеспечивает порядок commit и детектирует конфликты.
   - 単体テスト、アセット/ワールド/トリガーのテスト。
2. **クエリモデルのリファクタリング**
   - ページネーション/カーソルを表示するには、`crates/iroha_core/src/query/` を使用します。 Norito 表現と `iroha_data_model` を組み合わせます。
   - トリガー、アセット、ロールのスナップショット クエリを実行します (отслеживается через `crates/iroha_core/tests/snapshot_iterable.rs` для текущего покрытия)。
3. **スナップショットの一貫性**
   - Убедиться、что `iroha ledger query` CLI использует тот же スナップショット パス、что и Sumeragi/fetchers。
   - CLI スナップショット回帰テスト находятся в `tests/cli/state_snapshot.rs` (機能ゲート型 для медленных прогонов)。### フェーズ C - Sumeragi パイプライン
1. **トポロジーとエポック管理**
   - WSV ステーク スナップショットの特性 `EpochRosterProvider` です。
   - `WsvEpochRosterAdapter::from_peer_iter` は、コンストラクター、モック、ベンチ/テストを備えています。
2. **コンセンサスフローの簡素化**
   - `crates/iroha_core/src/sumeragi/*` のメッセージ: `pacemaker`、`aggregation`、`availability`、`witness` が必要です。 `consensus`。
   - アドホック メッセージ受け渡し、Norito エンベロープ、ビュー変更プロパティ テスト (трекится в Sumeragi メッセージング バックログ)。
3. **レーン/プルーフの統合**
   - レーンプルーフ、DA コミットメント、および RBC ゲーティングをサポートします。
   - エンドツーエンドで `integration_tests/tests/extra_functional/seven_peer_consistency.rs` を確認し、RBC を確認します。

### フェーズ D - スマート コントラクトと Pointer-ABI ホスト
1. **ホスト境界監査**
   - ポインター タイプのオプション (`ivm::pointer_abi`) およびホスト アダプター (`iroha_core::smartcontracts::ivm::host`)。
   - ポインター テーブルとホスト マニフェスト バインディング、`crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` および `ivm_host_mapping.rs`、ゴールデン TLV マッピングを参照します。
2. **実行サンドボックスのトリガー**
   - トリガー、トリガー、`TriggerExecutor`、ガス、ポインター検証、イベント ジャーナリング。
   - 回帰テストでは、コール/タイム トリガーと障害パスをテストします (`crates/iroha_core/tests/trigger_failure.rs`)。
3. **CLI とクライアントの調整**
   - CLI の機能 (`audit`、`gov`、`sumeragi`、`ivm`) のクライアントфункции `iroha`、ドリフトです。
   - CLI JSON スナップショット テスト находятся в `tests/cli/json_snapshot.rs`;これは、標準的な JSON リファレンスのコア クラスです。

### フェーズ E - Norito コーデック強化
1. **スキーマ レジストリ**
   - Norito スキーマ レジストリと `crates/norito/src/schema/`、コア データ型の正規エンコーディングを確認します。
   - ドキュメント テスト、エンコード サンプル ペイロード (`norito::schema::SamplePayload`)。
2. **ゴールデン フィクスチャの更新**
   - `crates/norito/tests/*` のゴールデン フィクスチャを確認し、WSV スキーマを確認してください。
   - `scripts/norito_regen.sh` デート Norito JSON ゴールデンズ ヘルパー `norito_regen_goldens`。
3. **IVM/Norito の統合**
   - Kotodama マニフェストのエンドツーエンドの Norito、ポインター ABI メタデータの説明。
   - `crates/ivm/tests/manifest_roundtrip.rs` удерживает Norito マニフェストをエンコード/デコードします。

## 3. Сквозные вопросы
- **テスト戦略**: 単体テスト -> クレート テスト -> 統合テスト。 Падающие тесты фиксируют текущие регрессии;ログインしてください。
- **ドキュメント**: После посадки каждой фазы обновлять `status.md` и переносить незакрытые пункты в `roadmap.md`, удаляя заверленные задачи。
- **パフォーマンス ベンチマーク**: `iroha_core`、`ivm`、`norito` のベンチマークを取得します。問題が解決されれば、それが可能になります。
- **機能フラグ**: バックエンド、ツールチェーン (`cuda`、`zk-verify-batch`) をクレートレベルで切り替えます。 CPU SIMD が必要な場合は、CPU SIMD を使用してください。スカラー フォールバックが必要になります。

## 4. Ближайствие действия
- フェーズ A の足場 (スナップショット特性 + テレメトリ配線) - см.実行可能なタスクとロードマップ。
- `sumeragi`、`state` および `ivm` のメッセージ:
  - `sumeragi`: ブロードキャスト ビュー変更プルーフ、VRF リプレイ状態、および EMA テレメトリ エクスポートに対するデッドコード許容量。ゲートされたゲート、成果物フェーズ C 、コンセンサス フロー、レーン/プルーフを確認します。
  - `state`: `Cell` クリーンアップ、テレメトリ ルーティング、フェーズ A WSV テレメトリ トラック、SoA/並列適用、フェーズ C パイプライン最適化バックログ。
  - `ivm`: CUDA トグル、エンベロープ検証、Halo2/Metal カバレッジ、フェーズ D のホスト境界、GPU アクセラレーション。カーネルと GPU バックログが表示されます。
- クロスチーム RFC は、承認を得るために承認されます。## 5. Открытые вопросы
- RBC оставаться опциональным после P1、или он обязателен для Nexus レジャーレーン? Требуется резение стейкхолдеров
- DS コンポーザビリティ グループと P1 を組み合わせてレーン証明を作成しますか?
- ML-DSA-87 を使用しますか? Кандидат: новый crate `crates/fastpq_isi` (создание ожидается)。

---

_Последнее обновление: 2025-09-12_