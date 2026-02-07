---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-リファクタリングプラン
タイトル: 台帳のリファクタリゼーション計画 Sora Nexus
説明: `docs/source/nexus_refactor_plan.md` のミロワール、コード Iroha 3 の基本的なフェーズのネット対戦の詳細。
---

:::note ソースカノニク
Cette ページは `docs/source/nexus_refactor_plan.md` を反映します。 Gardez les deux は、ポータルに到着したときに多言語で編集できる複数の言語をコピーします。
:::

# 台帳のリファクタリゼーション計画 Sora Nexus

Nexus Ledger (「Iroha 3」) のリファクタリングによるロードマップの即時ドキュメント キャプチャ。互換性ジェネシス/WSV のトポロジとデポの実際の回帰観察、コンセンサス Sumeragi、スマート コントラクトのトリガー、スナップショットのリクエスト、ホスト ポインター ABI のバインディングとコーデック Norito を参照します。オブジェクトは、単一のパッチを適用せずに、統合されたアーキテクチャと一貫したテストが可能であり、テスト可能です。

## 0. プリンシペスのディレクター
- 異質な素材を決定するための準備者。機能フラグのオプトインによるフォールバックの同一性を利用して、アクセラレーションを一意にします。
- Norito シリアル化のテスト。ラウンドトリップ Norito のエンコード/デコードと、一週間のフィクスチャのテストが含まれる変更日/スキーマを宣伝します。
- LA 設定トランジット パー `iroha_config` (ユーザー -> 実際の -> デフォルト)。サプライヤーは、本番環境のパスをアドホックに切り替えます。
- La politique ABI Reste V1 および交渉不可。ホストがリジェッターの決定を制御し、ポインター タイプ/システムコールが中断されないようにします。
- `cargo test --workspace` およびゴールデン テスト (`ivm`、`norito`、`integration_tests`) は、ゲート ド ベースを注ぐチャク ジャロンを保持します。

## 1. デポのトポロジーのスナップショット
- `crates/iroha_core`: アクチュア Sumeragi、WSV、ローダージェネシス、パイプライン (クエリ、オーバーレイ、ZK レーン)、スマート コントラクトのグルー ホスト。
- `crates/iroha_data_model`: スキーマはオンチェーンでドニーとリクエストを自動で実行します。
- `crates/iroha`: API クライアントは CLI、テスト、SDK を使用します。
- `crates/iroha_cli`: CLI オペレーター、`iroha` による API の使用制限の削除。
- `crates/ivm`: VM のバイトコード Kotodama、ホストのエントリと統合ポインタの ABI のポイント。
- `crates/norito`: JSON およびバックエンド AoS/NCB に適応するシリアル化コーデック。
- `integration_tests`: アサーション クロスコンポーネント Couvrant Genesis/ブートストラップ、Sumeragi、トリガー、ページネーションなど。
- Nexus 台帳 (`nexus.md`、`new_pipeline.md`、`ivm.md`) のドキュメントの説明は、実装は最も断片的であり、廃止された古いコードです。

## 2. リファクタリングと作業の練習

### フェーズ A - 財団と観察可能性
1. **テレメトリ WSV + スナップショット**
   - `state` (特性 `WorldStateSnapshot`) のスナップショット API は、クエリ、Sumeragi および CLI を使用します。
   - スナップショットの作成を行うユーザー `scripts/iroha_state_dump.sh` は、`iroha state dump --format norito` 経由で決定されます。
2. **決定論の生成/ブートストラップ**
   - リファクタライザーは、Norito (`iroha_core::genesis`) に関するパイプライン固有のベースの取り込み生成を実行します。
   - 統合/回帰は、生成の初期段階に加えて、WSV が arm64/x86_64 と一致するかどうかを確認します (`integration_tests/tests/genesis_replay_determinism.rs`)。
3. **修正クロスクレートのテスト**
   - Etendre `integration_tests/tests/genesis_json.rs` は、不変条件 WSV、パイプライン、ABI ダン スル ハーネスを検証します。
   - スキーマ ドリフトによるスキャフォールド `cargo xtask check-shape` の導入 (バックログ DevEx ツールの調査、`scripts/xtask/README.md` のアクション アイテムの確認)。

### フェーズ B - WSV および表面要求
1. **状態ストレージのトランザクション**
   - Collapser `state/storage_transactions.rs` は、アップリケのトランザクション ネルを適応させ、コミットと衝突の検出を制御します。
   - 単体テストでは、資産/ワールド/トリガーのフォント ロールバックを一時的に変更するかどうかを検証します。
2. **要求されたモデルのリファクタリング**
   - `crates/iroha_core/src/query/` のように、ページネーション/カーソルの再利用可能なコンポーネントをデプレーサします。アライナーの表現 Norito と `iroha_data_model`。
   - スナップショット クエリは、トリガー、アセット、およびロールを順番に決定します (`crates/iroha_core/tests/snapshot_iterable.rs` によるクーベルチュール実行)。
3. **スナップショットの一貫性**
   - 確実なファイル CLI `iroha ledger query` は、スナップショット クエリ Sumeragi/fetchers のファイル ミーム ケミンを利用します。
   - `tests/cli/state_snapshot.rs` を使用したスナップショット CLI 設定の回帰テスト (機能ゲート型プール実行レント)。### フェーズ C - パイプライン Sumeragi
1. **時代と時代のトポロジー**
   - Extraire `EpochRosterProvider` は、WSV ステークのスナップショットに関する実装ベースの平均値です。
   - `WsvEpochRosterAdapter::from_peer_iter` fournit un constructioneur シンプルでフレンドリーなモックとベンチ/テスト。
2. **合意形成による簡素化**
   - リオーガナイザー `crates/iroha_core/src/sumeragi/*` モジュール: `pacemaker`、`aggregation`、`availability`、`witness` の平均的なタイプは `consensus` です。
   - 置換ファイルのメッセージ受け渡しアドホックなエンベロープ Norito タイプおよびビュー変更プロパティ テストの導入 (バックログ メッセージング Sumeragi)。
3. **統合レーン/プルーフ**
   - アライナーは、レーンの証明と DA のエンゲージメントを保証し、RBC の制服をゲートすることを保証します。
   - エンドツーエンドの統合テスト `integration_tests/tests/extra_functional/seven_peer_consistency.rs` は、メンテナンス ル ケミン アベック RBC がアクティブであることを確認します。

### フェーズ D - スマート コントラクトとホスト ポインター - ABI
1. **辺境監査の主催者**
   - ポインター タイプのチェック (`ivm::pointer_abi`) とホストの適応 (`iroha_core::smartcontracts::ivm::host`) を統合します。
   - `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` および `ivm_host_mapping.rs` のポインター テーブルとホスト マニフェストのバインディングについて説明し、TLV ゴールデン マッピングを実行します。
2. **トリガーの実行のサンドボックス**
   - リファクタライザーは、`TriggerExecutor` コミュニケーション アップリケ ガス、ポインタの検証、およびイベントのジャーナリング パラメータを実行するトリガーを作成します。
   - Ajouter des testing de regression pour les は、コール/時間 couvrant les chemins d'echec をトリガーします (`crates/iroha_core/tests/trigger_failure.rs` 経由のスイビ)。
3. **CLI とクライアントの調整**
   - 保証者は、クライアント側の機能 `iroha` がドリフトを回避できるように、操作 CLI (`audit`、`gov`、`sumeragi`、`ivm`) を報告します。
   - `tests/cli/json_snapshot.rs` による CLI のスナップショット JSON のテスト。 Gardez-les a jour pour que la syortie core は、参照 JSON 正規版との対応を継続します。

### フェーズ E - コーデック Norito の要求
1. **スキーマのレジストリ**
   - スキーマ Norito のレジストリを作成し、`crates/norito/src/schema/` ソース コードをコード化してタイプ コアを作成します。
   - ペイロードのエンコードと例を検証するためのドキュメント テスト (`norito::schema::SamplePayload`)。
2. **黄金の備品をリフレッシュ**
   - 黄金の備品 `crates/norito/tests/*` は、WSV の新しいリファクタリング スキーマに対応します。
   - `scripts/norito_regen.sh` は、ヘルパー `norito_regen_goldens` 経由でゴールデン JSON Norito を再生成します。
3. **統合 IVM/Norito**
   - マニフェスト Kotodama のシリアル化を Norito 経由でエンドツーエンドで検証し、メタデータ ポインター ABI の一貫性を保証します。
   - `crates/ivm/tests/manifest_roundtrip.rs` パリテのメンテナンス Norito ファイルマニフェストをエンコード/デコードします。

## 3. 横断的な関心事
- **テスト戦略**: Chaque フェーズの単体テスト -> クレート テスト -> 統合テスト。テストと実際の回帰を捕捉します。レ・ヌーボー・テストは明らかなルール・ルトゥールをテストします。
- **ドキュメント**: 到着後の段階、1 週間の `status.md` および報告者からのアイテムの報告と `roadmap.md` の終了報告書。
- **パフォーマンスのベンチマーク**: `iroha_core`、`ivm`、および `norito` の既存のベンチを維持します。リグレッションの不在を検証するための基本的なポストリファクタリングの測定。
- **機能フラグ**: 管理者は、ツールチェーンの外部に必要なバックエンドのユニークな機能を切り替えます (`cuda`、`zk-verify-batch`)。 Les chemins SIMD CPU は、実行の構築と選択を行います。フォールバックのスケールは、サポートされていない材料を注ぐことを決定します。## 4. 即時アクション
- フェーズ A の足場 (スナップショット特性 + 配線テレメトリー) - スケジュール上のロードマップで実行可能なアクションを確認します。
- 最近のデフォルトの監査 `sumeragi`、`state` および `ivm` の争点の開示:
  - `sumeragi`: デッドコード保護の許可、ビュー変更のブロードキャスト、VRF の再生、およびテレメトリ EMA のエクスポートが可能です。 Ils は、フェーズ C および統合レーン/プルーフ ソエント ライブラリのコンセンサスの流れを単純化するためのゲートを保持します。
  - `state`: `Cell` のネットワークと、フェーズ A のピステ テレメトリ WSV のルーティング テレメトリ、フェーズ C のパイプラインのバックログ最適化の SoA/並列適用バスキュレントのメモを確認します。
  - `ivm`: CUDA の切り替え、Halo2/Metal の環境の検証と動作の説明、フェーズ D のホスト境界とテーマのトランスバーサル 加速 GPU の移動。カーネルはバックログを保持し、GPU を成熟させます。
- 作成者は、コードの侵入に伴う変更を事前に確認して、RFC チーム間での再開計画を立てます。

## 5. 質問は覆す
- RBC doit-il Rester optionnel apres P1, ou est-il obligatoire pour les LANes du Ledger Nexus?事前に当事者が必要とする決定。
- 合成グループのグループを作成する DS と P1 を使用して、レーンの証明を成熟させる必要がありますか?
- ML-DSA-87 パラメータのローカリゼーションを確立しますか?候補者: nouveau crate `crates/fastpq_isi` (注意して作成)。

---

_デルニエール 1 時間: 2025-09-12_