---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-refactor-plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-リファクタリングプラン
タイトル: Plano de refatoracao do ledger Sora Nexus
説明: `docs/source/nexus_refactor_plan.md` のエスペリョ、Iroha 3 を実行するベースのコードを詳細に説明します。
---

:::note フォンテ カノニカ
エスタページナリフレテ`docs/source/nexus_refactor_plan.md`。マンテンハは、デュアス コピアス アリンハダスとして、多言語のポータルを食べました。
:::

# Plano de refatoracao do 元帳 Sora Nexus

ドキュメント キャプチャ、ロードマップ、Sora Nexus Ledger ("Iroha 3") のリファクタリングを即座に実行します。ジェネシス/WSV ブックキーピングの観察、コンセンサス Sumeragi、スマート コントラクトのトリガー、スナップショットの参照、ホスト ポインターのバインディング - ABI コーデック Norito を参照して、実際のレイアウトをリポジトリに反映します。 O オブジェクトは、単一の Unico パッチを修正するために、正確なアーキテクチャとテストを行うためのコンバージルです。

## 0. プリンシピオス ギア
- ハードウェアの異機種間互換性を維持します。機能フラグ オプトイン com フォールバック identicos 経由で aceleracao apenas を使用します。
- Norito はシリアル化されたカメラです。往復の Norito エンコード/デコード、フィクスチャの認証テストが含まれる、テスト/スキーマの開発に関するQualquer。
- `iroha_config` の流体を設定します (ユーザー -> 実際の -> デフォルト)。 Remover は、制作環境のアドホック パスを切り替えます。
- A politica ABI permanece V1 e inegociavel。ホストは、決定的な形式のポインター タイプ/システムコールを開発します。
- `cargo test --workspace` e os ゴールデン テスト (`ivm`、`norito`、`integration_tests`) 継続的なコモ ゲート ベース パラ カダ マルコ。

## 1. トポロジーとリポジトリのスナップショット
- `crates/iroha_core`: アトーレス Sumeragi、WSV、ローダー生成、パイプライン (クエリ、オーバーレイ、ZK レーン)、グルー ド ホスト デ スマート コントラクト。
- `crates/iroha_data_model`: オンチェーンでのスキーマ自動クエリ。
- `crates/iroha`: CLI、テスト、SDK のクライアント用 API。
- `crates/iroha_cli`: `iroha` の CLI 操作、実際の数ある API。
- `crates/ivm`: VM のバイトコード Kotodama、統合されたポインター ABI のホストです。
- `crates/norito`: JSON およびバックエンド AoS/NCB に適応するシリアル化コーデック。
- `integration_tests`: アサーション クロスコンポーネント cobrindo ジェネシス/ブートストラップ、Sumeragi、トリガー、paginacao など。
- Nexus Ledger (`nexus.md`、`new_pipeline.md`、`ivm.md`) のドキュメントを詳細に記述し、断片化された部分を実装し、関連性を確認します。

## 2. ピラレス・デ・レファトラソンとマルコス

### フェーズ A - 基礎と観察
1. **テレメトリア WSV + スナップショット**
   - `state` (特性 `WorldStateSnapshot`) クエリ、Sumeragi、CLI などのスナップショットの API を確立します。
   - `scripts/iroha_state_dump.sh` を使用して、`iroha state dump --format norito` 経由でスナップショットを決定します。
2. **生成/ブートストラップの決定論**
   - ユニコ パイプライン コム Norito (`iroha_core::genesis`) の生成に関連した流体の摂取を参照。
   - WSV 同一の entre arm64/x86_64 (acompanhado em `integration_tests/tests/genesis_replay_determinism.rs`) を基に、再処理の生成を開始するか、再処理を追加します。
3. **修正クロスクレートのテスト**
   - WSV の `integration_tests/tests/genesis_json.rs` パラ検証の不変量、パイプラインおよび ABI エミュレーション Unico ハーネスを展開します。
   - スキャフォールド `cargo xtask check-shape` のパニックおよびスキーマ ドリフトの紹介 (ツール DevEx のバックログはありません。アクション アイテム `scripts/xtask/README.md` を参照)。

### フェーズ B - WSV のクエリの権限
1. **国家保管庫の取引**
   - Colapsar `state/storage_transactions.rs` エミュレーション アダプターのトランザクション処理で、コミットとコンフィトスの検出が行われました。
   - ユニットテストは、アセット/ワールド/トリガーの変更を前に確認し、ロールバックを実行します。
2. **クエリのモデルを参照する**
   - `crates/iroha_core/src/query/` でコンポーネントのページ/カーソルを再利用するためのロジックを移動します。アリンハルは Norito と `iroha_data_model` を表します。
   - トリガー、アセット、ロールに関する追加のスナップショット クエリを詳細に決定します (`crates/iroha_core/tests/snapshot_iterable.rs` を使用して、実際に実行されます)。
3. **スナップショットの一貫性**
   - CLI `iroha ledger query` のスナップショット モード Sumeragi/フェッチャーの使用を保証します。
   - `tests/cli/state_snapshot.rs` を使用した CLI を使用しないスナップショットの回帰テスト (機能ゲート型パラレルレントス実行)。### フェーズ C - パイプライン Sumeragi
1. **トポロジーとエポカスの物語**
   - Extrair `EpochRosterProvider` パラメータ特性は、WSV ステークのスナップショットのベースを実装します。
   - `WsvEpochRosterAdapter::from_peer_iter` は、コンストラクターの単純化とベンチ/テストのパラモックを提供します。
2. **コンセンサスを実現するシンプルな方法**
   - `crates/iroha_core/src/sumeragi/*` のモジュールを再編成します: `pacemaker`、`aggregation`、`availability`、`witness` は、`consensus` と比較します。
   - エンベロープ Norito のアドホックなメッセージ受け渡しの代替メッセージと、ビュー変更のプロパティ テストの紹介 (メンサゲリア Sumeragi のバックログはありません)。
3. **インテグラカオ レーン/プルーフ**
   - アリンハル・レーンは、DA と RBC の制服を保証するという約束を証明します。
   - エンドツーエンドの `integration_tests/tests/extra_functional/seven_peer_consistency.rs` を検証し、RBC の機能を確認してください。

### Fase D - スマート コントラクトとホスト ポインター - ABI
1. **オーディトリア ダ フロンテイラ ド ホスト**
   - ポインター タイプの検証 (`ivm::pointer_abi`) とホストのアダプター (`iroha_core::smartcontracts::ivm::host`) を統合します。
   - ポインタ テーブルとホスト マニフェストの OS バインディングが `crates/iroha_core/tests/ivm_pointer_abi_tlv_types.rs` および `ivm_host_mapping.rs` のコベルトスで期待されているため、TLV ゴールデンの OS マッピングを実行します。
2. **トリガーの実行サンドボックス**
   - Refatorar は、`TriggerExecutor` コミュニケート ガス、ポインタの検証、イベントのジャーナリングを通じて、パラロッドをトリガーします。
   - ファルハの呼び出し/時間コブリンド パスのトリガに関する回帰テストを追加しました (`crates/iroha_core/tests/trigger_failure.rs` 経由のコンパンハド)。
3. **CLI クライアントのアリンハメント**
   - CLI (`audit`、`gov`、`sumeragi`、`ivm`) を使用して、クライアント `iroha` パラメタ ドリフトを実行する機能をサポートします。
   - スナップショット JSON のテストは CLI で実行され、`tests/cli/json_snapshot.rs` で実行されます。 mantenha-os atualizados para que a Saida dos comandos は、参照 JSON canonica を継続します。

### Fase E - コーデック Norito の強化
1. **スキーマの登録**
   - スキーマ Norito および `crates/norito/src/schema/` の標準エンコーディングの標準コアの基準。
   - アディシオナドのドキュメント テストは、ペイロードの例を示すコードを検証します (`norito::schema::SamplePayload`)。
2. **ゴールデンフィクスチャーをリフレッシュ**
   - Atualizar os のゴールデン フィクスチャ em `crates/norito/tests/*` は、新しいスキーマ WSV およびリファクタリング コンポーネントと一致します。
   - `scripts/norito_regen.sh` は、ヘルパー `norito_regen_goldens` を介して、ゴールデン JSON Norito を再生成します。
3. **インテグラカオ IVM/Norito**
   - Kotodama のマニフェストのシリアル化が Norito 経由でエンドツーエンドで有効になり、メタデータ ポインター ABI の一貫性が保証されます。
   - `crates/ivm/tests/manifest_roundtrip.rs` マンテム パリダード Norito パラマニフェストのエンコード/デコード。

## 3. 横型プレオキュパコス
- **Estrategia de testes**: 単体テスト -> クレート テスト -> 統合テストを促進します。ファルハムのキャプチャをテストして、問題を解決します。 novos テストは que retornem を妨げます。
- **Documentaco**: Apos cada fase, atualizar `status.md` e Levar itens em aberto para `roadmap.md` enquanto Remove tarefas conluidas.
- **パフォーマンスのベンチマーク**: Manter ベンチは `iroha_core`、`ivm`、`norito` に存在します。 Adicionar medicoes ベースの pos-refactor は、検証済みの問題を解決します。
- **機能フラグ**: Manter は、バックエンドと外部ツールチェーンの完全な機能を切り替えます (`cuda`、`zk-verify-batch`)。 CPU の SIMD パスは、ランタイムの構成要素と選択要素を構成します。フォルネサー フォールバックは、ハードウェアのサポートを決定するエスカラレスです。## 4. Acoes immediatas
- Fase A の足場 (スナップショット特性 + テレメトリの配線) - ロードマップを作成し、実際に実行します。
- `sumeragi`、`state` および `ivm` に関する最近の不正行為の報告:
  - `sumeragi`: デッドコード プロテジェム、ビュー変更のブロードキャスト、リプレイ VRF の確立、およびテレメトリア EMA のエクスポートの許可。永続的なゲートは、レーン/プルーフ セジャム エンターゲスを統合するために、簡単にコンセンサスを実行してゲートを作成します。
  - `state`: `Cell` は、Fase A のテレメトリア WSV のローテーション、パイプラインのバックログなしで SoA/並列適用のエントラムとして有効です。
  - `ivm`: CUDA の切り替え、エンベロープの検証、および Halo2/Metal のホスト境界パラオ トラバルホの GPU のトランスバーサル テーマを説明します。カーネルは永続的にバックログを持たず、GPU がすぐに使用できるようになります。
- RFC クロスチームが、侵襲的ムダンカスの承認を得るために準備を整えます。

## 5. クエストエス・エム・アベルト
- RBC は、P1 の永久管理オプションを開発します。元帳 Nexus のレーンを義務付けますか?関係者に決定を求めてください。
- DS em P1 ou os mantemos desativados は、レーンの証明としてアマデュレカムを食べましたか?
- ローカル カノニコ パラ パラメトロス ML-DSA-87 に適していますか?候補: novo crate `crates/fastpq_isi` (criacao pendente)。

---

_Ultima atualizacao: 2025-09-12_