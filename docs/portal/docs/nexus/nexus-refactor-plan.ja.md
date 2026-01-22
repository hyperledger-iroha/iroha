---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 97bfa4b4887cc3e97f4664652fd0954742774bbd76f4425f2c1ca173153b3801
source_last_modified: "2025-11-10T17:33:34.235150+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-refactor-plan
title: Sora Nexus Ledger リファクタ計画
description: `docs/source/nexus_refactor_plan.md` のミラーで、Iroha 3 コードベースの段階的なクリーンアップ作業を詳述します。
---

:::note 正本
このページは `docs/source/nexus_refactor_plan.md` を反映しています。多言語版がポータルに到達するまで両方のコピーを揃えてください。
:::

# Sora Nexus Ledger Refactor Plan

このドキュメントは Sora Nexus Ledger ("Iroha 3") のリファクタに向けた直近のロードマップをまとめています。現在のリポジトリ構成と、genesis/WSV の台帳管理、Sumeragi コンセンサス、スマートコントラクトのトリガー、スナップショットクエリ、pointer-ABI のホストバインディング、Norito コーデックで観測されたリグレッションを反映しています。目的は、全ての修正を1つの巨大なパッチで着地させるのではなく、整合的でテスト可能なアーキテクチャに収束させることです。

## 0. 指針
- 異種ハードウェア間での決定論的な挙動を保持する; 加速は opt-in の feature flags 経由でのみ使用し、同一の fallback を用意する。
- Norito はシリアライゼーション層。状態/スキーマ変更は Norito encode/decode の round-trip テストと fixture 更新を含める。
- 設定は `iroha_config` (user -> actual -> defaults) を通る。プロダクションパスから ad-hoc な環境トグルを除去する。
- ABI ポリシーは V1 のままで交渉不可。hosts は未知の pointer types/syscalls を決定論的に拒否する。
- `cargo test --workspace` と golden tests (`ivm`, `norito`, `integration_tests`) が各マイルストーンの基準ゲート。

## 1. リポジトリトポロジーのスナップショット
- `crates/iroha_core`: Sumeragi actors、WSV、genesis loader、pipelines (query, overlay, zk lanes)、スマートコントラクト host の glue。
- `crates/iroha_data_model`: on-chain データとクエリの権威的スキーマ。
- `crates/iroha`: CLI、tests、SDK が利用する client API。
- `crates/iroha_cli`: オペレーター向け CLI。現在は `iroha` の多数の APIs をミラーしている。
- `crates/ivm`: Kotodama bytecode VM と pointer-ABI host 統合のエントリポイント。
- `crates/norito`: JSON adapters と AoS/NCB backends を備えた serialization codec。
- `integration_tests`: genesis/bootstrap、Sumeragi、triggers、pagination などを跨いで検証する assertions。
- Docs は Sora Nexus Ledger の目標 (`nexus.md`, `new_pipeline.md`, `ivm.md`) を既に示しているが、実装は断片化しておりコードに対して部分的に古い。

## 2. リファクタの柱とマイルストーン

### Phase A - 基盤と可観測性
1. **WSV Telemetry + Snapshots**
   - `state` に canonical snapshot API (trait `WorldStateSnapshot`) を確立し、queries、Sumeragi、CLI から利用する。
   - `scripts/iroha_state_dump.sh` を使い `iroha state dump --format norito` で決定論的 snapshot を生成する。
2. **Genesis/Bootstrap の決定論性**
   - genesis の取り込みを単一の Norito パイプライン (`iroha_core::genesis`) に通るようリファクタする。
   - genesis と最初のブロックを再生し、arm64/x86_64 間で同一の WSV root を検証する integration/regression テストを追加する (追跡先: `integration_tests/tests/genesis_replay_determinism.rs`)。
3. **Cross-crate Fixity Tests**
   - `integration_tests/tests/genesis_json.rs` を拡張し、WSV、pipeline、ABI の invariants を1つの harness で検証する。
   - schema drift で panic する `cargo xtask check-shape` の scaffold を導入する (DevEx tooling backlog で追跡; `scripts/xtask/README.md` の action item を参照)。

### Phase B - WSV とクエリ面
1. **State Storage Transactions**
   - `state/storage_transactions.rs` を、commit 順序と conflict 検出を強制するトランザクションアダプタに統合する。
   - unit tests で asset/world/triggers の変更が失敗時に rollback されることを確認する。
2. **Query Model Refactor**
   - pagination/cursor ロジックを `crates/iroha_core/src/query/` 配下の再利用可能コンポーネントへ移す。`iroha_data_model` の Norito 表現と整合させる。
   - triggers、assets、roles の snapshot queries を決定論的順序で追加する (現状のカバレッジは `crates/iroha_core/tests/snapshot_iterable.rs` で追跡)。
3. **Snapshot Consistency**
   - `iroha ledger query` CLI が Sumeragi/fetchers と同一の snapshot パスを使うことを保証する。
   - CLI の snapshot regression tests は `tests/cli/state_snapshot.rs` にあり (遅い runs は feature-gated)。

### Phase C - Sumeragi Pipeline
1. **Topology と Epoch 管理**
   - `EpochRosterProvider` を trait として切り出し、WSV の stake snapshots をバックエンドにする実装を用意する。
   - `WsvEpochRosterAdapter::from_peer_iter` は benches/tests に便利な mock 対応 constructor を提供する。
2. **Consensus Flow の簡素化**
   - `crates/iroha_core/src/sumeragi/*` を `pacemaker`, `aggregation`, `availability`, `witness` のモジュールに再構成し、共有型は `consensus` に置く。
   - ad-hoc な message passing を型付き Norito envelopes に置き換え、view-change の property tests を導入する (Sumeragi messaging backlog で追跡)。
3. **Lane/Proof Integration**
   - lane proofs を DA commitments と整合させ、RBC gating を一様にする。
   - end-to-end 統合テスト `integration_tests/tests/extra_functional/seven_peer_consistency.rs` が RBC 有効ルートを検証する。

### Phase D - Smart Contracts と Pointer-ABI Hosts
1. **Host Boundary Audit**
   - pointer-type チェック (`ivm::pointer_abi`) と host adapters (`iroha_core::smartcontracts::ivm::host`) を統合する。
   - pointer table の期待値と host manifest の bindings は `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` と `ivm_host_mapping.rs` でカバーし、golden TLV mappings を検証する。
2. **Trigger Execution Sandbox**
   - triggers を共通の `TriggerExecutor` 経由で実行し、gas、pointer 検証、event journaling を強制するようにリファクタする。
   - call/time triggers の失敗パスを含む regression tests を追加する (追跡先: `crates/iroha_core/tests/trigger_failure.rs`)。
3. **CLI と Client の整合**
   - CLI 操作 (`audit`, `gov`, `sumeragi`, `ivm`) が共有 `iroha` client 関数に依存し、drift を避けるようにする。
   - CLI JSON snapshot tests は `tests/cli/json_snapshot.rs` にあり、コアコマンド出力が canonical JSON 参照に一致し続けるよう更新する。

### Phase E - Norito Codec のハードニング
1. **Schema Registry**
   - `crates/norito/src/schema/` に Norito schema registry を作成し、コアデータ型の canonical encodings を提供する。
   - サンプル payload のエンコーディングを検証する doc tests を追加する (`norito::schema::SamplePayload`)。
2. **Golden Fixtures Refresh**
   - `crates/norito/tests/*` の golden fixtures を更新し、リファクタ後の新しい WSV schema に合わせる。
   - `scripts/norito_regen.sh` は `norito_regen_goldens` helper を使って Norito JSON goldens を決定論的に再生成する。
3. **IVM/Norito Integration**
   - Kotodama manifest のシリアライズを Norito で end-to-end に検証し、pointer ABI metadata の一貫性を保証する。
   - `crates/ivm/tests/manifest_roundtrip.rs` が manifests の Norito encode/decode パリティを維持する。

## 3. 横断的な論点
- **Testing Strategy**: 各フェーズで unit tests -> crate tests -> integration tests を推進する。失敗テストが現在のリグレッションを捕捉し、新規テストが再発を防ぐ。
- **Documentation**: 各フェーズの着地後に `status.md` を更新し、未完了項目を `roadmap.md` に移して完了タスクを整理する。
- **Performance Benchmarks**: `iroha_core`, `ivm`, `norito` の既存 benches を維持し、リファクタ後にベースライン計測を追加してリグレッションがないことを確認する。
- **Feature Flags**: 外部 toolchain を必要とする backends (`cuda`, `zk-verify-batch`) のみ crate レベルの toggles を維持する。CPU SIMD パスは常にビルドされ実行時に選択されるため、未対応ハードウェア向けの決定論的 scalar fallback を提供する。

## 4. 直近のアクション
- Phase A の scaffolding (snapshot trait + telemetry wiring) - roadmap 更新の actionable tasks を参照。
- `sumeragi`, `state`, `ivm` の最近の defect audit で次のポイントが判明した:
  - `sumeragi`: dead-code allowances が view-change proof の broadcast、VRF replay state、EMA telemetry export をガードしている。Phase C の consensus flow 簡素化と lane/proof integration の deliverables が着地するまで gated のまま。
  - `state`: `Cell` cleanup と telemetry routing は Phase A の WSV telemetry トラックへ移行し、SoA/parallel-apply のメモは Phase C の pipeline 最適化 backlog に統合する。
  - `ivm`: CUDA toggle の露出、envelope validation、Halo2/Metal coverage は Phase D の host-boundary 作業と GPU 加速の横断テーマにマッピングされる; kernels は準備完了まで専用 GPU backlog に残る。
- 侵襲的なコード変更を着地させる前に、この計画を要約した cross-team RFC を用意して sign-off を得る。

## 5. 未解決の質問
- RBC は P1 を過ぎても任意のままか、それとも Nexus ledger lanes では必須か。ステークホルダーの判断が必要。
- P1 で DS composability groups を強制するか、lane proofs が成熟するまで無効のままにするか。
- ML-DSA-87 パラメータの canonical location はどこか。候補: 新規 crate `crates/fastpq_isi` (作成待ち)。

---

_最終更新: 2025-09-12_
