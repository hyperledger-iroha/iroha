---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-リファクタリングプラン
title: 台帳のリファクタリゼーション計画 Sora Nexus
説明: `docs/source/nexus_refactor_plan.md` の説明、Iroha 3 のコードベースに対する詳細な説明。
---

:::ノート フエンテ カノニカ
エスタパジーナリフレジャ`docs/source/nexus_refactor_plan.md`。多言語の国際ポータルを充実させましょう。
:::

# 元帳のリファクタリング計画 Sora Nexus

Nexus 台帳 (「Iroha 3」) のリファクタリングに関するロードマップのドキュメントを作成します。実際のリポジトリのレイアウトと、生成/WSV の管理に関する回帰観察、コンセンサス Sumeragi、スマート コントラクトのトリガー、スナップショットの参照、ホスト ポインターのバインディング - ABI コーデック Norito を参照します。オブジェクトは、一貫性のある建築と一致する可能性のある罪の目的を統合し、単一の単一性を実現するために収束します。

## 0. プリンシピオス ギア
- ハードウェアの異機種間互換性を維持します。 USAR Aceleracion Solo Con 機能フラグは、フォールバックのオプトインと同一性を示します。
- Norito シリアル化の上限。ラウンドトリップ Norito の実際のフィクスチャのエンコード/デコードを含む、セットアップ/エスケマ デベの確認。
- `iroha_config` の流体構成 (ユーザー -> 実際の -> デフォルト)。 Eliminar は、生産パスのアドホックなエンターノを切り替えます。
- La politica ABI sigue en V1 y 交渉の余地はありません。 Los hosts deben rechazar ポインター タイプ/システムコールは、決定形式を決定します。
- `cargo test --workspace` はゴールデン テスト (`ivm`、`norito`、`integration_tests`) を失い、コンピューターのベースを失います。

## 1. リポジトリのトポロジのスナップショット
- `crates/iroha_core`: アクター Sumeragi、WSV、ローダー生成、パイプライン (クエリ、オーバーレイ、ZK レーン)、ホストとスマート コントラクトの接着剤。
- `crates/iroha_data_model`: オンチェーン クエリに関する自動処理。
- `crates/iroha`: CLI、テスト、SDK のクライアント用 API。
- `crates/iroha_cli`: CLI のオペランド、`iroha` の API の実際の参照。
- `crates/ivm`: VM のバイトコード Kotodama、統合ポインターのエントリ - ABI のホスト。
- `crates/norito`: JSON およびバックエンド AoS/NCB に適応するシリアル化コーデック。
- `integration_tests`: クブレンジェネシス/ブートストラップ、Sumeragi、トリガー、ページナシオンなどのコンポーネント間のアサーション。
- Nexus 台帳 (`nexus.md`、`new_pipeline.md`、`ivm.md`) のドキュメントや詳細を記述し、フラグメントを実装し、古いコードを参照します。

## 2. リファクタリングと人のピラレス

### Fase A - 基礎と観察
1. **テレメトリア WSV + スナップショット**
   - `state` (特性 `WorldStateSnapshot`) のクエリ、Sumeragi および CLI のスナップショットの API を確立します。
   - `scripts/iroha_state_dump.sh` のユーザーは、`iroha state dump --format norito` 経由でプロデューサーのスナップショットを決定します。
2. **生成/ブートストラップの決定論**
   - Norito (`iroha_core::genesis`) の Unico パイプラインの生成パラメータをリファクタリングします。
   - WSV と同じ entre arm64/x86_64 (seguido en `integration_tests/tests/genesis_replay_determinism.rs`)。
3. **修正クロスクレートのテスト**
   - WSV の `integration_tests/tests/genesis_json.rs` パラ検証の不変量、パイプライン、およびソロ ハーネスの ABI を拡張します。
   - スキャフォールド `cargo xtask check-shape` のパニック前スキーマ ドリフトの紹介 (ツール DevEx のバックログの確認、アクション アイテム `scripts/xtask/README.md`)。

### フェーズ B - WSV のクエリの詳細
1. **状態ストレージのトランザクション**
   - Colapsar `state/storage_transactions.rs` は、競合の検出をコミットするためのアダプターのトランザクション処理を実行します。
   - ロスユニットテストは、アセット/ワールド/トリガーの変更前にアホラ検証を実行します。
2. **クエリモデルのリファクタリング**
   - ページ論理/カーソルのコンポーネントを再利用可能なバジョ `crates/iroha_core/src/query/` に移動します。線形は Norito と `iroha_data_model` を表します。
   - トリガー、アセット、ロールに関する Agregar スナップショット クエリが決定的になります (実際の `crates/iroha_core/tests/snapshot_iterable.rs` 経由のセグイド)。
3. **スナップショットの一貫性**
   - `iroha ledger query` CLI では、スナップショット クエリ Sumeragi/fetchers を使用します。
   - `tests/cli/state_snapshot.rs` での CLI でのスナップショットの回帰テスト (機能ゲート型パララン レントス) の損失テスト。### フェーズ C - パイプライン Sumeragi
1. **時代のトポロジーと誕生**
   - Extraer `EpochRosterProvider` は、WSV でステークされたスナップショットのレスパルダの実装に問題があります。
   - `WsvEpochRosterAdapter::from_peer_iter` は、ベンチ/テストでのコンストラクターのシンプルで親しみやすいパラ モックの参照です。
2. **合意内容の簡略化**
   - `crates/iroha_core/src/sumeragi/*` をモジュールとして再構成します: `pacemaker`、`aggregation`、`availability`、`witness` を、`consensus` と比較します。
   - エンベロープ Norito のアドホックなメッセージ受け渡しに関するヒントとビュー変更の導入プロパティ テスト (メンサジェリア Sumeragi のバックログのセギド)。
3. **統合レーン/プルーフ**
   - 直線レーンの証明は、DA と同様の RBC ゲートの海の制服に準拠しています。
   - エンドツーエンドの `integration_tests/tests/extra_functional/seven_peer_consistency.rs` 統合テストで、RBC の有効性を検証します。

### Fase D - スマート コントラクトとホスト ポインター - ABI
1. **ホストの制限された聴衆**
   - Consolidar はポインター タイプのチェック (`ivm::pointer_abi`) とホストのアダプター (`iroha_core::smartcontracts::ivm::host`) を失います。
   - `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` および `ivm_host_mapping.rs` のホスト マニフェストのバインディングが失われ、ポインタ テーブルが期待され、TLV ゴールデンのマッピングが失われます。
2. **トリガーのサンドボックスの排出**
   - Refactorizar は、国連 `TriggerExecutor` コミュニケート ガス、ポインタの検証、イベントのジャーナリングを介して、パラエジェクターをトリガーします。
   - Agregar は、呼び出し/時間パスのトリガに関する回帰テストを実行します (`crates/iroha_core/tests/trigger_failure.rs` 経由のセグイド)。
3. **CLI クライアントとの連携**
   - クライアントのコンパートメント機能に依存する CLI (`audit`、`gov`、`gov`、`ivm`) の操作 `iroha` パラメータ ドリフト。
   - `tests/cli/json_snapshot.rs` でのスナップショットの JSON del CLI の損失テスト。 JSON 正規版のリファレンスとして、シンガポールのコマンドが同時に実行されます。

### Fase E - エンデュレシミエント デル コーデック Norito
1. **スキーマの登録**
   - スキーマ Norito バジョ `crates/norito/src/schema/` パラバステサー エンコーディングの標準コアを登録します。
   - ペイロードの文書テストを検証するためのドキュメント テストを作成します (`norito::schema::SamplePayload`)。
2. **ゴールデン器具の実現**
   - 新しいスキーマ WSV のリファクタリングで、`crates/norito/tests/*` と一致するゴールデン フィクスチャが実際に失われます。
   - `scripts/norito_regen.sh` は、EL ヘルパー `norito_regen_goldens` を介して、Norito のゴールデン JSON を再生成します。
3. **統合 IVM/Norito**
   - Kotodama のマニフェストのシリアル化は、Norito 経由でエンドツーエンドで検証され、メタデータ ポインター ABI の一貫性が保たれます。
   - `crates/ivm/tests/manifest_roundtrip.rs` mantiene la paridad Norito パラマニフェストをエンコード/デコードします。

## 3. テマスの横断販売
- **テスト戦略**: 単体テスト -> クレート テスト -> 統合テストを計画的に実行します。ロステストは、実際の回帰を捕らえます。ロス・ヌエボスはエビタン・ケ・レアパレスカンをテストする。
- **ドキュメント**: 正確な内容、実際の内容 `status.md` および `roadmap.md` に関する詳細情報を確認してください。
- **ベンチマーク**: `iroha_core`、`ivm` および `norito` の Mantener ベンチが存在します。 agregar mediciones ベースのリファクタリング後、干し草のリグレッションがないことを確認します。
- **機能フラグ**: Mantener は、外部ツールチェーンに必要なバックエンドの nivel クレート ソロパラを切り替えます (`cuda`、`zk-verify-batch`)。 CPU の SIMD パスは実行時に選択を構成します。証明者はフォールバックをエスカレートし、ハードウェアの決定的な問題を解決できません。## 4. アクシオネス・インメディアタス
- フェーズ A の足場 (スナップショット特性 + テレメトリの配線) - ロードマップの実際の作業内容を確認します。
- `sumeragi`、`state`、`ivm` に関する欠陥の聴取法:
  - `sumeragi`: デッドコード プロテジェン ブロードキャスト、ビュー変更のプルエバス、VRF の再生、テレメトリア EMA のエクスポートの許可。永続的なゲートは、レーン/プルーフ アテリセンの統合に必要な、フルホ デ コンセンサスでの簡素化を実現します。
  - `state`: `Cell` のリンピエザと、フェーズ A のテレメトリ WSV の追跡、フェーズ C のパイプラインの最適化における SoA/並列適用の統合バックログの管理。
  - `ivm`: CUDA の切り替えの説明、エンベロープの検証、および Halo2/Metal のホスト境界の管理、Fase D マス エル テマのトランスバーサル GPU の制御。カーネルの永久的なバックログと GPU の初期リスト。
- 侵襲的状況に応じて承認を得るために、RFC クロスチームの再開計画を準備します。

## 5. 自由な活動
- デベ RBC は P1 を管理し、元帳 Nexus に義務を負っていますか?利害関係者に決定を求める。
- Impulsamos grupos de composabilidad DS en P1 o los mantenemos deshabilitados hasta que maduren las laneproof?
- ML-DSA-87 はパラメトロス パラメトロスをサポートしていますか?候補: nuevo crate `crates/fastpq_isi` (creacion pendiente)。

---

_究極の現実化: 2025-09-12_