---
lang: ja
direction: ltr
source: docs/source/nexus_cross_lane.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 6e6f144bf3aef313ba55b539c9e92c827bd626973fe38b557f0b668cc909f589
source_last_modified: "2025-12-13T05:07:11.929584+00:00"
translation_last_reviewed: 2026-01-01
---

# Nexus クロスレーン・コミットメントと証明パイプライン

> **ステータス:** NX-4 デリバラブル - クロスレーン・コミットメントのパイプラインと証明（目標 2025 Q4）。  
> **オーナー:** Nexus Core WG / Cryptography WG / Networking TL。  
> **関連ロードマップ項目:** NX-1（lane geometry）、NX-3（settlement router）、NX-4（本ドキュメント）、NX-8（global scheduler）、NX-11（SDK conformance）。

本ノートは、レーンごとの実行データが検証可能なグローバルコミットメントになる流れを説明する。既存の settlement router（`crates/settlement_router`）、lane block builder（`crates/iroha_core/src/block.rs`）、テレメトリ/ステータス面、そしてロードマップ **NX-4** に必要だが未実装の LaneRelay/DA フックを結び付ける。

## 目標

- レーンブロックごとに決定的な `LaneBlockCommitment` を生成し、プライベートな状態を漏らさずに settlement、流動性、分散データを収める。
- これらのコミットメント（および DA アテステーション）をグローバル NPoS リングに中継し、マージ台帳がクロスレーン更新を順序付け、検証し、永続化できるようにする。
- Torii とテレメトリで同一のペイロードを公開し、運用者、SDK、監査者が専用ツールなしでパイプラインを再現できるようにする。
- NX-4 を完了とみなすための不変条件と証拠バンドル（レーン証明、DA アテステーション、マージ台帳統合、回帰テスト）を定義する。

## コンポーネントとサーフェス

| コンポーネント | 責務 | 実装参照 |
|-----------|----------------|---------------------------|
| レーン実行器と settlement router | XOR 変換を見積もり、トランザクションごとの receipt を蓄積し、バッファ方針を適用 | `crates/iroha_core/src/settlement/mod.rs`, `crates/settlement_router` |
| lane block builder | `SettlementAccumulator` を排出し、レーンブロックとともに `LaneBlockCommitment` を発行 | `crates/iroha_core/src/block.rs:3340-3415` |
| LaneRelay broadcaster | レーン QC と DA 証明を束ね、`iroha_p2p` に流し、merge ring を供給 | `crates/iroha_core/src/nexus/lane_relay.rs`, `crates/iroha_core/src/sumeragi/main_loop.rs` |
| グローバル merge ledger | レーン QC を検証し、merge hints を削減し、world-state のコミットメントを永続化 | `docs/source/merge_ledger.md`, `crates/iroha_core/src/sumeragi/status.rs`, `crates/iroha_core/src/state.rs` |
| Torii ステータスとダッシュボード | `lane_commitments`, `lane_settlement_commitments`, `lane_relay_envelopes`、scheduler gauge、Grafana ボードを公開 | `crates/iroha_torii/src/routing.rs:16660-16880`, `dashboards/grafana/nexus_lanes.json` |
| 証拠ストレージ | `LaneBlockCommitment`、RBC アーティファクト、Alertmanager スナップショットを監査向けに保管 | `docs/settlement-router.md`, `artifacts/nexus/*`（将来の bundle） |

## データ構造とペイロードレイアウト

正準のペイロードは `crates/iroha_data_model/src/block/consensus.rs` にある。

### `LaneSettlementReceipt`

- `source_id` - トランザクションハッシュ、または呼び出し側が指定する id。
- `local_amount_micro` - dataspace ガストークンのデビット。
- `xor_due_micro` / `xor_after_haircut_micro` / `xor_variance_micro` - XOR 帳簿の決定的な項目と、レシート単位の安全マージン（`due - after haircut`）。
- `timestamp_ms` - settlement 中に取得した UTC ミリ秒タイムスタンプ。

レシートは `SettlementEngine` の決定的な見積もりルールを継承し、`LaneBlockCommitment` 内で集約される。

### `LaneSwapMetadata`

見積もりに用いたパラメータを記録する任意メタデータ:

- `epsilon_bps`, `twap_window_seconds`, `volatility_class`.
- `liquidity_profile` の bucket（Tier1-Tier3）。
- `twap_local_per_xor` の文字列で、監査者が変換を厳密に再計算できるようにする。

### `LaneBlockCommitment`

レーンごとの要約を各ブロックに保存:

- ヘッダ: `block_height`, `lane_id`, `dataspace_id`, `tx_count`.
- 合計: `total_local_micro`, `total_xor_due_micro`, `total_xor_after_haircut_micro`, `total_xor_variance_micro`.
- 任意の `swap_metadata`.
- 順序付き `receipts` ベクタ。

これらの struct はすでに `NoritoSerialize`/`NoritoDeserialize` を導出しているため、オンチェーン、Torii、または fixtures 経由でスキーマ差分なしにストリームできる。

### `LaneRelayEnvelope`

`LaneRelayEnvelope`（`crates/iroha_data_model/src/nexus/relay.rs` 参照）は、レーンの
`BlockHeader`、任意の `ExecutionQcRecord`、任意の `DaCommitmentBundle` ハッシュ、完全な
`LaneBlockCommitment`、およびレーン単位の RBC バイト数をパッケージする。エンベロープは
Norito 由来の `settlement_hash`（`compute_settlement_hash` 経由）を保持し、受信者が merge
ledger へ転送する前に settlement ペイロードを検証できるようにする。`verify` が失敗する
場合（QC subject の不一致、DA ハッシュの不一致、settlement ハッシュの不一致）、
`verify_with_quorum` が失敗する場合（署名者ビットマップ長/クオラムエラー）、あるいは
集約 QC 署名が dataspace 委員会の roster に対して検証できない場合は拒否する。QC の
プリイメージはレーンブロックハッシュに `parent_state_root` と `post_state_root` を加えた
もので、メンバーシップと state-root の正しさを同時に検証する。

### レーン委員会の選定

レーン relay QC は dataspace 単位の委員会で検証する。委員会サイズは `3f+1` で、`f` は
dataspace カタログの `fault_tolerance` で設定される。バリデータプールは dataspace の
バリデータで、admin-managed レーンのガバナンス manifest と、stake-elected レーンの
public lane staking レコードから構成される。委員会メンバーはエポックごとに、
`dataspace_id` と `lane_id` に束縛した VRF エポックシードで決定的に抽選する（エポック
期間は安定）。プールが `3f+1` を下回る場合、レーン relay の finality はクオラムが回復
するまで停止する。運用者は admin multisig 命令 `SetLaneRelayEmergencyValidators` を使って
プールを拡張できる（`CanManagePeers` と `nexus.lane_relay_emergency.enabled = true` が必要、
デフォルトは無効）。有効時は、設定済みの最小条件
（`nexus.lane_relay_emergency.multisig_threshold`/`multisig_members`、デフォルト 3-of-5）を
満たす multisig アカウントでなければならない。オーバーライドは dataspace ごとに保存し、
プールがクオラム未満のときだけ適用し、空の validator リストを送ると解除される。
`expires_at_height` が設定されている場合、lane relay envelope の `block_height` が期限
を超えると検証はオーバーライドを無視する。テレメトリカウンタ
`lane_relay_emergency_override_total{lane,dataspace,outcome}` は、オーバーライドが適用された
（`applied`）のか、欠落/期限切れ/不足/無効だったのかを記録する。

## コミットメントのライフサイクル

1. **見積もりと receipt の準備。**  
   settlement ファサード（`SettlementEngine`, `SettlementAccumulator`）はトランザクションごとに
   `PendingSettlement` を記録する。各レコードは TWAP 入力、流動性プロファイル、タイムスタンプ、
   XOR 金額を保存し、後で `LaneSettlementReceipt` になる。

2. **receipt をブロックに封印。**  
   `BlockBuilder::finalize` 中に、各 `(lane_id, dataspace_id)` ペアがアキュムレータを排出する。
   builder は `LaneBlockCommitment` を生成し、receipt リストをコピーして合計を計算し、
   任意の swap メタデータ（`SwapEvidence` 経由）を保存する。結果のベクタは Sumeragi ステータス
   スロット（`crates/iroha_core/src/sumeragi/status.rs`）に押し込み、Torii とテレメトリが即時公開する。

3. **relay パッケージングと DA アテステーション。**  
   `LaneRelayBroadcaster` はブロック封印中に生成された `LaneRelayEnvelope` を消費し、
   高優先度の `NetworkMessage::LaneRelay` フレームとして配布する。エンベロープは検証され、
   `(lane_id,dataspace_id,height,settlement_hash)` で重複排除され、Sumeragi ステータススナップショット
   （`/v1/sumeragi/status`）に保存される。broadcaster は今後も、DA アーティファクト
   （RBC チャンク証明、Norito ヘッダ、SoraFS/Object の manifest）を付与し、head-of-line の
   ブロックを避けつつ merge ring に供給できるように進化する。

4. **グローバル順序付けと merge ledger。**  
   NPoS リングは各 relay envelope を検証する。dataspace 委員会に対する `lane_qc` の検証、
   settlement 合計の再計算、DA 証明の検証を行い、`docs/source/merge_ledger.md` で説明される
   merge ledger の削減にレーン tip を投入する。merge エントリが封印されると、
   world-state ハッシュ（`global_state_root`）は各 `LaneBlockCommitment` をコミットする。

5. **永続化と公開。**  
   Kura はレーンブロック、merge エントリ、`LaneBlockCommitment` をアトミックに書き込み、
   リプレイで同じ削減を再構築できるようにする。`/v1/sumeragi/status` は以下を公開する:
   - `lane_commitments`（実行メタデータ）。
   - `lane_settlement_commitments`（ここで説明するペイロード）。
   - `lane_relay_envelopes`（relay ヘッダ、QC、DA ダイジェスト、settlement ハッシュ、RBC バイト数）。
  ダッシュボード（`dashboards/grafana/nexus_lanes.json`）は同じテレメトリ/ステータス面を読み、
  レーン throughput、DA 可用性アラート、RBC 量、settlement 差分、relay 証拠を表示する。

## 検証と証明ルール

merge ring はレーンコミットメントを受け入れる前に、以下を必ず満たす必要がある:

1. **レーン QC の妥当性。** 実行投票プリイメージ（ブロックハッシュ、`parent_state_root`,
   `post_state_root`, 高さ/ビュー/エポック, `chain_id`, モードタグ）に対する BLS 集約署名を
   dataspace 委員会 roster で検証し、署名者ビットマップ長が委員会に一致し、署名者が
   正しいインデックスに対応し、ヘッダ高さが `LaneBlockCommitment.block_height` と一致することを
   確認する。
2. **receipt の整合性。** receipt ベクタから `total_*` 集計を再計算し、合計が不一致、または
   `source_id` が重複している場合は拒否する。
3. **swap メタデータの健全性。** `swap_metadata`（存在する場合）がレーンの settlement 設定と
   バッファ方針に一致することを確認する。
4. **DA アテステーション。** relay が提供する RBC/SoraFS 証明が埋め込まれたダイジェストに
   ハッシュ一致し、chunk セットがブロック全体を覆うことを検証する（`rbc_bytes_total` メトリクスが
   これを反映する必要がある）。
5. **merge の削減。** レーン証明が通過したらレーン tip を merge ledger エントリに含め、
   Poseidon2 削減（`reduce_merge_hint_roots`）を再計算する。不一致があれば merge エントリを中止する。
6. **テレメトリと監査トレイル。** レーンごとの監査カウンタ
   （`nexus_audit_outcome_total{lane_id,...}`）を増やし、エンベロープを永続化して証拠バンドルが
   証明と観測トレイルの両方を含むようにする。

## データ可用性と観測性

- **メトリクス:**  
  `nexus_scheduler_lane_teu_*`, `nexus_scheduler_dataspace_*`, `sumeragi_rbc_da_reschedule_total`,
  `da_reschedule_total`, `sumeragi_da_gate_block_total{reason="missing_local_data"}`,
  `lane_relay_invalid_total{error}`, `lane_relay_emergency_override_total{outcome}`,
  `nexus_audit_outcome_total` は `crates/iroha_telemetry/src/metrics.rs` に存在する。運用者は
  missing-availability のスパイクに警告を出し（reschedule カウンタはレガシーで常にゼロであるべき）、
  `lane_relay_invalid_total` は敵対的ドリル以外ではゼロを保つ。
- **Torii サーフェス:**  
  `/v1/sumeragi/status` は `lane_commitments`, `lane_settlement_commitments`, dataspace スナップショットを含む。
  `/v1/nexus/lane-config`（計画中）は `LaneConfig` のジオメトリを公開し、クライアントが
  `lane_id` と dataspace ラベルを対応付けられるようにする。
- **ダッシュボード:**  
  `dashboards/grafana/nexus_lanes.json` はレーン backlog、DA 可用性シグナル、上記の settlement 合計を表示する。
  アラート定義は以下でページングする:
  - `nexus_scheduler_dataspace_age_slots` がポリシー違反。
  - `sumeragi_da_gate_block_total{reason="missing_local_data"}` が継続的に増加。
  - `total_xor_variance_micro` が過去の基準から逸脱。
- **証拠バンドル:**  
  各リリースは `LaneBlockCommitment` のエクスポート、Grafana/Alertmanager スナップショット、
  relay DA manifest を `artifacts/nexus/cross-lane/<date>/` に付与する。バンドルは NX-4 readiness
  レポート提出時の正準証拠セットとなる。

## 実装チェックリスト（NX-4）

1. **LaneRelay サービス**
   - `LaneRelayEnvelope` にスキーマを定義し、`crates/iroha_core/src/nexus/lane_relay.rs` の broadcaster を
     `crates/iroha_core/src/sumeragi/main_loop.rs` に接続してブロック封印に組み込み、ノード単位の重複排除と
     ステータス永続化付きで `NetworkMessage::LaneRelay` を送出する。
   - 監査向けに relay アーティファクトを保存（`artifacts/nexus/relay/...`）。
2. **DA アテステーションフック**
   - RBC / SoraFS チャンク証明を relay envelope に統合し、`SumeragiStatus` にサマリーメトリクスを保存。
   - Torii と Grafana で DA ステータスを公開。
3. **merge ledger 検証**
   - merge エントリ検証で raw lane header ではなく relay envelope を必須にする。
   - 合成コミットメントを merge ledger に流し、決定的削減を確認する replay テスト
     （`integration_tests/tests/nexus/*.rs`）を追加。
4. **SDK とツール更新**
   - SDK 向けに `LaneBlockCommitment` の Norito レイアウトを文書化
     （`docs/portal/docs/nexus/lane-model.md` からリンク済み; API スニペットを追加）。
   - 決定的 fixtures は `fixtures/nexus/lane_commitments/*.{json,to}` に置き、スキーマ変更時は
     `cargo xtask nexus-fixtures` で再生成（または `--verify` で検証）して
     `default_public_lane_commitment` と `cbdc_private_lane_commitment` を更新する。
5. **観測性と runbook**
   - 新規メトリクス向け Alertmanager パックを接続し、`docs/source/runbooks/nexus_cross_lane_incident.md` に
     証拠ワークフローを記載する（追補）。

上記チェックリストと本仕様を完了することで、**NX-4** のドキュメント要件を満たし、残る実装作業を解放する。
