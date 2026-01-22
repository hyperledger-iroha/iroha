<!-- Japanese translation of docs/source/nexus_refactor_plan.md -->

---
lang: ja
direction: ltr
source: docs/source/nexus_refactor_plan.md
status: complete
translator: manual
---

# Sora Nexus Ledger リファクタ計画

本ドキュメントは Sora Nexus Ledger（Iroha 3）の即時ロードマップをまとめたものです。ジェネシス／WSV 管理、Sumeragi コンセンサス、スマートコントラクトトリガー、スナップショットクエリ、ポインタ ABI ホストバインディング、Norito コーデックで観測されている回帰を反映しています。巨大な一括パッチではなく、テスト可能な一貫したアーキテクチャへ収束することが目的です。

## 0. 指針
- 異種ハードウェア間の決定性を保持。アクセラレーションは機能フラグで opt-in とし、同一フォールバックを提供する。
- Norito がシリアライゼーション層。状態／スキーマ変更は Norito の往復テストとフィクスチャ更新を必須とする。
- 設定は `iroha_config`（user→actual→defaults）経由。プロダクション経路からアドホックな環境トグルを排除。
- ABI ポリシーは V1 固定。未知のポインタ型／システムコールは決定的に拒否。
- `cargo test --workspace`、`ivm`・`norito`・`integration_tests` のゴールデンテストが各マイルストンの最低ライン。

## 1. リポジトリ構成スナップショット
- `crates/iroha_core`: Sumeragi アクター、WSV、ジェネシスローダー、パイプライン（クエリ、オーバーレイ、ZKレーン）、スマートコントラクトホスト。
- `crates/iroha_data_model`: オンチェーンデータ／クエリの正規スキーマ。
- `crates/iroha`: CLI・テスト・SDK が利用するクライアント API。
- `crates/iroha_cli`: オペレーター CLI（`iroha` の API を多数ミラー）。
- `crates/ivm`: Kotodama VM、ポインタ ABI ホスト統合。
- `crates/norito`: シリアライゼーションコーデック（JSON アダプタ、AoS/NCB バックエンド）。
- `integration_tests`: ジェネシス／ブートストラップ、Sumeragi、トリガー、ページングなど横断テスト。
- ドキュメント（`nexus.md`, `new_pipeline.md`, `ivm.md`）は Nexus の目標を示すが、実装は断片的でコードと乖離。

## 2. リファクタの柱とマイルストン

### Phase A — 基盤と可観測性
1. **WSV テレメトリとスナップショット**
   - `state` に正規スナップショット API（`WorldStateSnapshot` trait）を定義し、クエリ／Sumeragi／CLI で共用。
   - `scripts/iroha_state_dump.sh` を使って `iroha state dump --format norito` による決定的スナップショットを取得する。
2. **ジェネシス／ブートストラップの決定性**
   - ジェネシス処理を Norito ベースの単一路（`iroha_core::genesis`）へ集約。
   - `integration_tests/tests/genesis_replay_determinism.rs` でジェネシスと最初のブロックを arm64 / x86_64 の双方でリプレイし、WSV のルート（`state_root` / `world_state_hash`）が一致することを検証する。
3. **クロスクレート固定テスト**
   - `integration_tests/tests/genesis_json.rs` を拡張し、WSV／パイプライン／ABI の不変条件を一括検証。
   - スキーマ変化を検知して panic する `cargo xtask check-shape` のスケルトンを追加（DevEx ツールバックログで追跡中。詳細は `scripts/xtask/README.md` のアクション項目を参照）。

### Phase B — WSV とクエリ面
1. **ストレージトランザクション**
   - `state/storage_transactions.rs` をトランザクションアダプタへ集約し、コミット順序と競合検出を強制。
   - ユニットテストで資産／世界／トリガー変更が失敗時にロールバックすることを確認。
2. **クエリモデル再設計**
   - ページング／カーソルロジックを `crates/iroha_core/src/query/` の再利用可能コンポーネントへ移動。Norito 表現を `iroha_data_model` と整合。
   - トリガー・資産・ロールのスナップショットクエリに決定的な順序付けを追加する（現状は `crates/iroha_core/tests/snapshot_iterable.rs` がカバレッジを追跡）。
3. **スナップショット整合性**
   - `iroha ledger query` CLI が Sumeragi／フェッチャーと同じスナップショット経路を使用するよう調整。
   - CLI スナップショットの回帰テストは `tests/cli/state_snapshot.rs` にあり、遅延経路向けに feature gate されています。

### Phase C — Sumeragi パイプライン
1. **トポロジ／エポック管理**
   - `EpochRosterProvider` を WSV ステークスナップショットを利用する trait に抽出。
  - `WsvEpochRosterAdapter::from_peer_iter` を使えばベンチ／テスト向けに簡単にモックロスターを構築できる。
2. **コンセンサスフロー単純化**
   - `crates/iroha_core/src/sumeragi/*` を `pacemaker`、`aggregation`、`availability`、`witness` モジュールに整理し共有型は `consensus` へ。
   - アドホックなメッセージパスを型付き Norito エンベロープに置き換え、ビュー変更順序のプロパティテストを追加する（Sumeragi メッセージングバックログで追跡中）。
3. **レーン／証明統合**
   - レーン証明を DA コミットメントと整合させ、RBC ゲートを統一。
   - 統合テスト `integration_tests/tests/extra_functional/seven_peer_consistency.rs` が RBC 有効経路を検証。

### Phase D — スマートコントラクト／ポインタ ABI ホスト
1. **ホスト境界監査**
   - ポインタ型チェック（`ivm::pointer_abi`）とホストアダプタ（`iroha_core::smartcontracts::ivm::host`）を統合。
   - ポインタテーブルの期待値とホストマニフェストのバインディングは `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` と `ivm_host_mapping.rs` のゴールデンテストでカバーされる。
2. **トリガー実行サンドボックス**
   - ガス／ポインタ検証／イベント記録を共通化する `TriggerExecutor` を導入。
   - コール／タイムトリガーの失敗ケースは `crates/iroha_core/tests/trigger_failure.rs` などの回帰テストで検証する。
3. **CLI／クライアント整合**
   - CLI (`audit`, `gov`, `sumeragi`, `ivm`) が `iroha` クライアント関数を共有するよう統一。
   - CLI の JSON スナップショットテストは `tests/cli/json_snapshot.rs` で維持され、主要コマンドの出力がモデル JSON と整合することを継続検証する。

### Phase E — Norito コーデック強化
1. **スキーマレジストリ**
   - `crates/norito/src/schema/` に正規エンコードを提供するレジストリを設置。
   - `norito::schema::SamplePayload` の doctest でサンプルペイロードのエンコードを検証済み。
2. **ゴールデン更新**
   - リファクタ後の WSV スキーマに合わせ `crates/norito/tests/*` ゴールデンを更新。
   - `scripts/norito_regen.sh` が `norito_regen_goldens` ヘルパーを呼び出し、Norito の JSON ゴールデンを決定的に再生成します。
3. **IVM/Norito 統合**
   - Kotodama マニフェストの Norito シリアライゼーションをエンドツーエンド検証し、ポインタ ABI メタデータを一致させる。
   - `crates/ivm/tests/manifest_roundtrip.rs` でマニフェストの Norito エンコード／デコード整合性を検証。

## 3. 横断的課題
- **テスト戦略:** 各フェーズでユニット→クレート→統合テストを進める。既存回帰を捉え新規テストで再発防止。
- **ドキュメント:** 各フェーズ完了後に `status.md` を更新し、未完了の項目は関連チケットへのリンク付きで `roadmap.md` に移管する。
- **性能ベンチ:** `iroha_core`、`ivm`、`norito` の既存ベンチを維持し、リファクタ後の基準値を記録。
- **機能フラグ:** 外部ツールチェーンが必要なバックエンド（`cuda`, `zk-verify-batch`）のみ crate レベルで管理。CPU SIMD は常にビルドし、未対応ハードでは決定的スカラフォールバックを使用。

## 4. 即時アクション
- Phase A のスケルトン（スナップショット trait とテレメトリ配線）の着手。
- 本計画の要約 RFC をチーム横断でレビューし、大規模変更前に承認を得る。

## 5. 未決事項
- RBC を P1 以降も任意とするか、Nexus レーンでは必須とするか（要ステークホルダー決定）。
- DS composability group を P1 で有効化するか、レーン証明が成熟するまで無効のままにするか。
- ML-DSA-87 パラメータの正規配置先（候補: 新規 `crates/fastpq_isi`）。

---

_最終更新: 2025-09-12_
