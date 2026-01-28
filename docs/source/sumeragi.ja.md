<!-- Japanese translation of docs/source/sumeragi.md -->

---
lang: ja
direction: ltr
source: docs/source/sumeragi.md
status: needs-update
translator: manual
---

## Sumeragi: 現行実装 (v1)

残りの移行タスクを詳細に把握したい場合は、[`sumeragi_npos_task_breakdown.md`](sumeragi_npos_task_breakdown.md) を参照してください。

### 概要
- **役割とローテーション**: ソート済みトポロジはピアを `Leader`、`ValidatingPeer`、`ProxyTail`、`SetBValidator` の役割に分割します。リーダー選出前にロスターは PeerId でソートされ、各ノードで決定論的になります。Permissioned ではエポックの PRF/VRF シードを使った PRF(seed, height) により高さごとにロスターを決定論的にシャッフルし、ビュー変更でシャッフル済みトポロジを回転してリーダーを進めます。前ブロックハッシュに基づくローテーションはリーダー選出には使いません。NPoS ではアクティブロスターをソートしてから PRF でリーダーを選び、ビューに合わせたトポロジを回転して署名者インデックスを一致させます。`network_topology.rs` の `rotated_for_prev_block_hash(prev_hash)` はレガシーの決定論的補助関数として残っています。
- **K コレクターモード**: 各高さ/ビューに対し、PRF でリーダーを除外した K 個のコレクターを決定論的に選びます。PRF シードがない場合は `proxy_tail_index()`（含む）から始まる連続領域にフォールバックします（巻き戻りなし）。K=1 は単一の PRF コレクターを選び、PRF がない場合はプロキシテール単独に収束します。
- **First-commit-certificate-wins**: PRF で選ばれた指定コレクター（リーダー除外）が正当な Precommit phase commit certificate を組み立てた場合、それを公表できます。ピアは同じ `(height, hash)` に対して最初に受信した正当な commit certificate を採用し、遅れて届いた重複は破棄します。ブロックコミットはこれらの commit certificate だけで判断されます。

### ノード役割（設定）
- `validator`（既定）: 現在のトポロジ上の役割に従って合意に参加します。
- `observer`: 合意トポロジから除外され（役割は `Undefined`）、本来その位置がコレクターであっても提案・投票・収集を行いません。ブロックゴシップで完全同期し、受信した commit certificate を使ってコミットできます。

### バリデータ鍵要件
- バリデータは BLS-Normal 公開鍵と証明（PoP）を必ず提示する必要があります。NPoS ではステーキングの public lane で Active なバリデータ集合に基づいて合意ロスターをフィルタします（commit topology 更新後も適用）。Active が存在しない場合は `trusted_peers` から BLS-Normal 鍵を持たないピアを除外した集合を初期バリデータ集合として採用します（`trusted_peers_bls` は廃止済み）。`trusted_peers_pop` はロスター全体をカバーする必要があり、設定パース時に PoP の欠落・無効は拒否されます。不整合が残る場合の安全策として PoP フィルタは BLS 基準ロスターにフォールバックします。BLS でないピアは常に除外され、PoP 欠落・無効は PoP フィルタ適用時のみ除外されます。
- commit certificate には同一メッセージ署名に対する BLS 集約署名、明示的な署名集合、コンパクトな署名者ビットマップが必須で添付されます。合意検証は明示署名に基づいたままです。集約署名とビットマップは監査目的のアーティファクトであり、セマンティクスを変えてはなりません。

### メッセージフロー（定常状態）
- **リーダー**: ブロック作成が必要な状況（トランザクション待ち）で期限が来ると `BlockCreated` をブロードキャストします。キューが空のときはペースメーカーは待機しハートビート提案は行いません。
- **提案の延期**: リレーのバックプレッシャが有効、アクティブ先端に未解決の RBC セッションがある、または先端を延長する保留ブロックがクォーラム再スケジュール前の場合、提案組み立ては延期され、ペースメーカーはバックログが解消するまで待機します。
- **バリデータ**: ブロックを検証し、可用性投票を発行し、指定コレクターへ Prevote/Precommit 投票を送ります。ビュー 0 でローカルタイムアウトした場合、ノードは最大 `r` 個まで追加コレクターに投票をファンアウトできます。
- **コレクター**: 投票を集約し、Availability/Prevote/Precommit の各 commit certificate を公表します。`(height, hash)` ごとに最初の正当な Precommit phase commit certificate が勝者となり、遅い commit certificate は無視されます。
- **観測ピア**: ビュー 0 では投票しません。後続ビューでは投票する場合があります。ブロック本体と一致する Precommit phase commit certificate が揃った時点で決定論的にコミットします。
- **受信側**: commit certificate を検証し、Highest/Locked QC の追跡を更新し、対応する Precommit phase commit certificate とブロック本体が揃ったらコミットを試みます。

### コミット規則（2 commit certificate パイプライン）
- 各バリデータは Highest/Locked の証明書参照（`highest_qc`/`locked_qc`）を追跡します。`highest_qc` は Prepare または Commit の証明書（通常は子高さ）で、`locked_qc` は安全性を守ります。`highest_qc` が `locked_qc` を拡張しない場合は提案を生成せず、さらに highest QC のブロック本体がローカルにない場合も提案を延期し、QC 履歴ロスターの fallback で欠落ブロック取得を試みます。
- 高さ `h` のブロックは、高さ `h + 1` の commit certificate の親ハッシュが高さ `h` でロックされた commit certificate と一致した時点で最終化されます。この「2 段」条件によりパイプラインを維持しつつ、競合チェーンがコミットを得ることを防ぎます。`h` の実行を進めながら `h + 1` の投票を集められます。
- この規則により、追加の最終化メッセージが不要になります。子 commit certificate が届けば親ブロックは安全であり、ブロックと 2 つの commit certificate を持つ全ノードが決定論的にコミットできます。最初の commit certificate を見逃したノードはゴシップ／RBC 経由でブロックと commit certificate を取得するだけで同じ決定に到達します。
- 任意の時点で開いている高さは 2 連続までのため、コレクター・RBC・テレメトリはフォークの危険なくパイプライン処理できます。`sumeragi::main_loop` 内の不変条件でこれを保証しており、`ensure_locked_qc_allows` が提案生成をガードし、保留ブロックはインメモリで追跡され、コミット経路では互換子 commit certificate を観測した後にのみ `locked_qc` を退役させます。

### ペースメーカー（ビュー変更）
- v1 ではビュー 0 における観測ピアの投票猶予が削除され、観測ピアはビュー 0 で投票しません。ビュー 0 でローカルタイムアウトした際は（ローテーション前の広げる処理なしで）ビュー変更を提案します。タイミングはオンチェーンの `SumeragiParameters`（`BlockTimeMs`/`CommitTimeMs`/`MinFinalityMs`/`PacingFactorBps`）を基準にし、NPoS の各フェーズタイムアウトは `SumeragiParameters.block_time_ms (scaled by pacing_factor_bps)` から導出し、必要なら `sumeragi.advanced.npos.timeouts` の上書きが効きます。リーダー提案はパイプライン時間の約 1/3、期待コミットは約 2/3 です。

### K / r パラメータ
- 設定キー: `sumeragi.collectors.k: usize`（高さごとのコレクター数、設定の既定は 1）、`sumeragi.collectors.redundant_send_r: u8`（冗長送信ファンアウト。ツールはバリデータ数から `2f+1` をオンチェーン既定として播種し、オンチェーン値がない場合の設定既定は 3（4 バリデータでの 2f+1））。
- オンチェーン: K と r は `SumeragiParameters` に保存され、コレクタ計画と `ConsensusParams` 広告の正となります。設定値はジェネシスの初期値を与えます。ピア間で K/r の広告が異なる場合でも、ミスマッチを記録してオンチェーン値を維持します。
- フォールバック: `k` でコレクタが選べない場合、投票はコミットトポロジ全体にフォールバックします。`redundant_send_r` は最小 1 として扱われます。

### トポロジ原則
- トポロジ生成は PoP マップがロスター全体をカバーしている場合のみ BLS PoP を持つバリデータで実施し、不完全な PoP マップは設定パースで拒否されます。不整合が残る場合の安全策として BLS 基準ロスターを使います。`leader_index()` と `proxy_tail_index()` は余り演算を使わず、監査しやすい決定論的な `TopologySegment` を返します。
- `deterministic_collectors(topology, mode, k, seed, height, view)` は `(height, view)` と PRF シードに基づく決定論的なコレクター集合を返します。PRF シードがない場合は `proxy_tail_index()` からの連続領域にフォールバックし、K=1 で従来のプロキシテール構成になります。
- `Topology::role_at(index)` で任意ピアの役割を取得し、`is_validator(idx)` でその高さにおけるバリデータかどうかを判定します。

### API サーフェス
- `iroha_cli --output-format text ops sumeragi status`（`GET /v1/sumeragi/status/summary`）がトポロジ情報・役割分布・commit certificate 状態に加え、RBC の利用状況やエポック調整値（`epoch_len`/`epoch_commit`/`epoch_reveal`）を表示します。
- `iroha_cli --output-format text ops sumeragi params`（`GET /v1/sumeragi/params`）で `collectors_k`、`collectors_redundant_send_r`、Sumeragi パラメータ（タイムアウト、モード等）を確認できます。
- `iroha_cli ops sumeragi status`（`GET /v1/sumeragi/status`）はノードの役割履歴、ビュー、ロック中 commit certificate、未コミットブロックなど完全な Norito レコードを返します。

### 実装ハイライト
- `sumeragi::main_loop` がイベントループを担当し、commit certificate 更新、提案処理、コミット処理を管理します。
- commit certificate ストレージは `HighestQc` と `LockedQc` を明示的な構造体で追跡し、ミスマッチ時に詳細ログとエビデンスを提供します。
- RBC（Reliable Broadcast）は非同期で動作し、ブロック本体が揃うと commit certificate に基づいて即座にコミットを試行します。
- Pacemaker はハートビートとビュー変更を制御し、permissioned は `SumeragiParameters` を基準に、NPoS は `SumeragiParameters.block_time_ms (scaled by pacing_factor_bps)` 由来のタイムアウト（`sumeragi.advanced.npos.timeouts` の上書きを含む）に従ってローカルタイムアウトやビュー拡張を行います。
- Telemetry は Prometheus メトリクスを公開し、`sumeragi_*` メトリクスで集約・キュー深さ・エビデンス件数などを可視化します。

### RBC／DA（データ可用性）
- RBC はトポロジから導出された Collector 集合を使用してブロック本体を配布します。ブロックヘッダーには RBC セッション ID とパケットメタデータが含まれます。
- `sumeragi.da.enabled` を有効にすると、可用性証跡（`availability evidence`）は advisory として追跡され、コミットは待機せずに進行します。ローカル payload が不足すると RBC `DELIVER` または BlockCreated/ブロック同期で取得します。可用性証跡が不足している間は `sumeragi_da_gate_block_total{reason="missing_local_data"}` が増加し、`da_reschedule_total` はレガシーのため通常 0 のままです。
- INIT 前に READY/DELIVER/チャンクが届いた場合、ノードはスタッシュし、欠落した `BlockCreated` を即時にリクエストします（欠落ブロックのバックオフを尊重）。
- `sumeragi.advanced.rbc.rebroadcast_sessions_per_tick` が tick あたりの RBC 再送セッション数を制限し、バックログ時の再送嵐を抑制します。復旧速度を上げたい場合は増やし、P2P キューが詰まる場合は下げます。
- 大規模ペイロード（≥10 MiB）を扱うシナリオでは RBC デリバリー時間、コミット時間、スループット、キュー深さをテレメトリで監視し、SLO 違反をアラートします。

### トポロジ／役割取得の CLI 例
- `iroha_cli --output-format text ops sumeragi topology` で現在の順序付きリストと役割を表示します。
- `iroha_cli ops sumeragi collectors --height <h>` で高さ `h` のコレクターを取得します。
- `iroha_cli ops sumeragi roles --peer <account>` で指定ピアの現在の役割を照会できます。

### エビデンス & スラッシング
- ダブル投票や無効な commit certificate/提案は `Evidence` レコードとして保存され、`/v1/sumeragi/evidence` エンドポイントから取得できます。
- `iroha_cli ops sumeragi evidence submit --evidence-hex <0x…>` を使って Norito エンコードされたエビデンスを提出できます。Torii は構造検証を行い、`invalid consensus evidence` の場合は保存しません。
- `iroha_cli ops sumeragi evidence count` で重複排除後の件数を確認し、監査やガバナンス判断に活用します。
- `SumeragiParameters evidence_horizon_blocks` により保持期間を制御できます。短すぎる値は CI テストで拒否されます。

### ガバナンスとモード切り替え
- `SumeragiMode` は `Hot`, `Cold`, `Maintenance` などの運転モードを定義し、`SetSumeragiMode` 命令でオンチェーン切り替えが可能です。
- `SumeragiParameters` の更新は `SetSumeragiParameters` で行い、`collectors_k`, `collectors_redundant_send_r`, タイムアウトなどを調整します。更新は `next_mode` と `mode_activation_height` を通じて段階的に適用されます。
- 既定ではモード切り替えは 1 ブロック後に有効化され、CLI の `iroha_cli --output-format text ops sumeragi params` で確認できます。

### モード運用パターン
- **Hot モード**: 通常運転。すべてのバリデータが BFT パイプラインに参加します。
- **Maintenance モード**: 新しい提案を拒否し、既存ブロックのコミットのみ許可。アップグレード時や長時間メンテナンスで使用します。
- **Cold モード**: 合意を停止し、ブロック処理を凍結します。緊急停止シナリオで使用します。

### CLI / API チートシート
- `iroha_cli --output-format text ops sumeragi status` / `GET /v1/sumeragi/status/summary`
- `iroha_cli ops sumeragi status` / `GET /v1/sumeragi/status`
- `iroha_cli --output-format text ops sumeragi params` / `GET /v1/sumeragi/params`
- `iroha_cli --output-format text ops sumeragi topology`
- `iroha_cli ops sumeragi collectors --height <h>`
- `iroha_cli --output-format text ops sumeragi evidence list`
- `iroha_cli ops sumeragi evidence count`
- `iroha_cli ops sumeragi evidence submit --evidence-hex <0x…>`

### テレメトリメトリクス
- `sumeragi_leader_proposals_total`
- `sumeragi_collector_qc_total{kind="availability|prevote|precommit"}`
- `sumeragi_pending_blocks`
- `sumeragi_pacemaker_backpressure_deferrals_total`
- `sumeragi_evidence_records_total`
- `sumeragi_collectors_k`, `sumeragi_collectors_r`

### テストカバレッジ
- `crates/iroha_core/tests/sumeragi_negative_paths.rs`: 無効な提案・commit certificate・重複投票を投稿し、`invalid proposal` / `invalid consensus evidence` を検証します。
- `integration_tests/tests/sumeragi_da.rs`: RBC/DA の大規模ペイロードを検証し、SLO 違反で失敗させます。
- `docs/source/sumeragi_da.md` に RBC の運用手順と測定基準がまとまっています。

---

## 実装詳細

### トポロジ
- `Topology` 構造体は ordered ピアリストと役割算出ロジックを保持します。
- `Topology::rotated_for_prev_block_hash` は前ブロックハッシュに基づく決定論的ローテーションを提供するレガシー補助で、現行の permissioned リーダー選出では使用しません。
- `TopologySegment` は特定レンジのピアをイテレータ形式で扱える軽量ビューです。

### コレクター選定
- `deterministic_collectors(topology, mode, k, seed, height, view)` は `(height, view)` と PRF シードに基づくコレクター順序（`Vec<PeerId>`）を返します。K の設定に応じて集約戦略が変わります。
- 冗長送信 `r` が設定されている場合、バリデータは `collectors.extend_for_redundancy(r)` で追加コレクターへ投票を送れます。

### View / Epoch の管理
- ビュー変更は `Pacemaker` がトリガーし、`ViewChange` メッセージが全ピアに伝播します。
- `Epoch` はガバナンスイベントやトポロジ変更を区切る単位であり、`FetchedTopology` が参照するスナップショットを更新します。

### Highest / Locked QC
- `HighestQc` と `LockedQc` はそれぞれ最新 commit certificate と安全性を担保する commit certificate を保持します。
- `update_highest_qc` は高さ・ビューを比較し、より新しい QC（Prepare/Commit）のみ受け入れます。
- `ensure_locked_qc_allows` が提案時に安全条件を確認し、違反した場合は提案を破棄しエビデンスを生成します。

### RBC
- RBC セッションは `RbcSessionId` で識別され、メタデータにはブロック高さ・ハッシュ・収集に必要な閾値が含まれます。
- RBC は Gossip でブロック断片を流通させ、全ピアが復元できるようにすることで DA を実現します。
- `sumeragi.da.enabled` を有効にすると、`availability evidence` は advisory として追跡され、コミットは待機せずに進行します。RBC は同じ設定で有効になり、ペイロード配布と欠落回復に使われます（ローカル payload は `RbcDeliver` またはブロック同期で満たされます）。
- INIT に含まれるロースターは未検証スナップショットとして保持し、コミットトポロジから導出したロースターを READY/DELIVER 検証とローカル署名の権威ソースとします。導出ロースターと競合する INIT ロースターは拒否されます。
- READY/DELIVER は権威ロースターが確定するまで一時保留し、欠落している `BlockCreated` はコミットトポロジにフォールバックしてリクエストします。権威化後に `roster_hash` が不一致な READY/DELIVER は破棄されます。
- READY 再送信は決定論的な f+1 サブセット（リーダーを常に含む）に限定し、メッセージ嵐を抑制します。

### `proof_policy` の設定
- `proof_policy = "off"`（既定）: commit QC のみ要求。
- `proof_policy = "zk_parent"`: 親の ZK 証明を要求（`zk_finality_k` 深さ）。

### Telemetry & Backpressure
- `pacemaker_backpressure_deferrals_total` が増加した場合、`scripts/sumeragi_backpressure_log_scraper.py` を使ってログと照合できます。
- `sumeragi_da_summary::*` メトリクスが RBC 実行状況を示し、CI で SLO を検証します。

### Evidence パイプライン
1. CLI や Torii から Norito エンコードした Evidence を受信。
2. `validate_evidence` が署名集合、ビュー、ハッシュ、一貫性を確認。
3. 正当であれば WSV に保存し、Prometheus カウンタを更新。
4. Horizon 過ぎた古いエビデンスはガーベジコレクションで削除。

### CLI 運用メモ
- `iroha_cli --output-format text ops sumeragi status` は `{role, view, locked_qc_height, highest_qc_height, pending_blocks}` に加えて RBC 利用状況やエポック調整値（`epoch_len`/`epoch_commit`/`epoch_reveal`）を即座に把握するためのショートカットです。
- `iroha_cli ops sumeragi params` は `collectors_k`, `collectors_r`, タイムアウト、モードを含むフル Norito JSON を返します。
- `--output-format text` を付けると CLI が整形した短い出力のみが得られます。

### コレクターテレメトリ実務ガイド

停滞したコレクターやコミット遅延は `availability evidence` が生成されない、DA 集約が長引く、`commit_ms`/`pipeline_total_ms` がスパイクする、といった形で表面化します。次の信号を監視してください。

**主要ダッシュボード**
- `sum(rate(sumeragi_da_votes_ingested_total[1m])) by (collector_idx)` — 投票を取り込めていないコレクターを特定。
- `sumeragi_qc_last_latency_ms{kind="availability"}` とヒストグラム `sumeragi_qc_assembly_latency_ms{kind="availability"}` — Availability evidence 組み立ての最新／直近レイテンシ。
- `sumeragi_phase_latency_ms{phase="collect_da"}` と `{phase="collect_precommit"}` の P95（5 分窓） — 可用性票と precommit QC 収集に費やした時間。
- `sumeragi_phase_latency_ms{phase="collect_aggregator"}` — 冗長送信の遅延。`sumeragi_gossip_fallback_total`、`block_created_dropped_by_lock_total`、`block_created_hint_mismatch_total`、`block_created_proposal_mismatch_total`、`pacemaker_backpressure_deferrals_total` と合わせてファンアウト不足やバックプレッシャを判別。
- `sumeragi_phase_latency_ema_ms{phase="propose|collect_da|collect_prevote|collect_precommit|commit"}`
  — 各フェーズの平滑化レイテンシ（EMA）。生値との乖離で異常を検出。
- `/v1/sumeragi/telemetry`（または `iroha_cli --output-format text ops sumeragi status`） — コレクター別の投票数、commit certificate レイテンシ、RBC backlog、最新 Highest/Locked QC ハッシュを含む軽量スナップショット。
- `/v1/sumeragi/status/sse` — 1 秒周期程度の SSE ストリームでライブ観測。
- `/v1/sumeragi/phases` / `iroha_cli --output-format text ops sumeragi phases` — `{propose_ms, collect_da_ms, collect_prevote_ms, collect_precommit_ms, collect_aggregator_ms, commit_ms, pipeline_total_ms}` と `ema_ms` を併記したフェーズ別タイムライン。
- `docs/source/grafana_sumeragi_overview.json` — commit certificate 高さの乖離、BlockCreated ドロップ、VRF 参加状況を可視化する Grafana ダッシュボード。

**アラート閾値**
- Availability evidence レイテンシ: `sumeragi_qc_last_latency_ms{kind="availability"}` が `0.6 * commit_time_ms` を 2 連続で超過、またはヒストグラム P95 が `0.7 * commit_time_ms` を超えたら要警告（permissioned は `CommitTimeMs`、NPoS は `effective_npos_timeouts.commit_ms`）。
- 投票取り込みの停滞: `sum(rate(sumeragi_da_votes_ingested_total[2m])) == 0` かつ `sumeragi_rbc_backlog_sessions_pending > 0`（RBC は動いているのに票が集まらない）。
- コミット遅延: `commit_ms` または `sumeragi_phase_latency_ms{phase="commit"}` の P95 が `0.75 * commit_time_ms` を超過、あるいは新しいブロックが届いているのに 3 ラウンド以上 0 のまま（permissioned は `CommitTimeMs`、NPoS は `effective_npos_timeouts.commit_ms`）。
- コレクターファンアウト: `collect_aggregator_ms` が `0.5 * sumeragi.advanced.npos.timeouts.aggregator_ms` を 3 ラウンド連続で上回る、`sumeragi_redundant_sends_total` が一つの View で `sumeragi.collectors.redundant_send_r` を超える、`rate(sumeragi_gossip_fallback_total[5m]) > 0`、`increase(block_created_dropped_by_lock_total[5m]) > 0`、`increase(block_created_hint_mismatch_total[5m]) > 0`、`increase(block_created_proposal_mismatch_total[5m]) > 0`、または `increase(pacemaker_backpressure_deferrals_total[5m]) > 0` のいずれかが持続。

冗長送信カウンタは DA リトライが追加コレクターへキャッシュ済み RBC ペイロードを再送したときに増えます。`npos_redundant_send_retries_update_metrics` テストでこの経路をカバーし、ダッシュボードと契約の乖離を検出します。

**トリアージ手順**
1. `iroha_cli --output-format text ops sumeragi collectors` で担当コレクターが現行の担当に残っているか確認。
2. `/v1/sumeragi/telemetry` を見て `votes_ingested` が伸びていないコレクター index を特定。単一コレクターだけ停滞しているなら一時的に `sumeragi.collectors.redundant_send_r` を増やし、票を次順位へ回す。
3. `sumeragi_bg_post_queue_depth` と `p2p_*_throttled_total` でキューやトランスポートの逼迫を診断。
4. FASTPQ プローバ遅延が疑われる場合は `/v1/torii/zk/prover/reports` と Torii ログでプローバの失敗を調べ、必要に応じて再起動。
5. 両コレクターが停止しているなら RBC backlog を確認。`sumeragi_rbc_store_evictions_total` と `sumeragi_rbc_persist_drops_total` のスパイク、または `/v1/sumeragi/status` の `rbc_store.recent_evictions` を手掛かりにボトルネックとなるペイロードやエポックを特定。
   さらに `iroha_cli --output-format text ops sumeragi status` は `lane_governance_sealed_total` / `lane_governance_sealed_aliases` を併記するため、未だ封止されたレーンを GUI を介さずに確認できます。ローンチ／ロールバック手順や CI では `iroha_cli app nexus lane-report --only-missing --fail-on-sealed` を合わせて実行し、マニフェストが未展開のまま進行しないようガードしてください。
6. 復旧後はインシデントを記録し、冗長送信やコレクターパラメータを通常値へ戻す。

---

## RBC / DA ロードマップ

1. **DA/RBC 有効化**: `sumeragi.da.enabled = true` を既定とし、`availability evidence` は advisory として追跡（コミットは待機せずに進行）。RBC は同じ設定で有効になり、ペイロード配布と欠落回復に使われます（ローカル payload は `RbcDeliver` またはブロック同期で満たされます）。
2. **前処理**: リーダーはブロック提案と同時に RBC セッションを起動し、Collectors が投票を集約するまでに全ピアへブロック断片を配布します。
3. **Fallback**: RBC 完了前に View 変更が発生した場合、次リーダーが同一ブロックを提案する必要性を評価し、未完セッションを踏まえて処理します。
4. **監視**: `sumeragi_da_summary::*` と `/v1/sumeragi/rbc/sessions` を定期取得し、遅延や失敗を早期検知します。
5. **CI**: `integration_tests/tests/sumeragi_da.rs` が 4 ピア／6 ピアのシナリオで 10 MiB 以上のペイロードを送信し、RBC デリバリー ≤3.6 s、コミット ≤4.0 s、スループット ≥2.7 MiB/s を検証します。

---

### VRF ランダムネスパイプライン

NPoS のペースメーカーは VRF からリーダー／コレクタのローテーションを導出します。各エポック（`epoch_length_blocks`）は次の 2 ウィンドウで構成されます。

1. **コミットウィンドウ**（`vrf_commit_deadline_offset` ブロック）— バリデータがリビールのハッシュを持つ `VrfCommit` を提出。
2. **リビールウィンドウ**（`vrf_reveal_deadline_offset` ブロック）— `VrfReveal` でリビールを開示。コントローラはコミットと照合して 32 バイトのリビールを記録します。

コミット／リビールの取り込みは同期的かつ決定的です。

- オペレーター（または自動化）が Torii に Norito JSON を POST:
  - `POST /v1/sumeragi/vrf/commit` `{ "epoch": <u64>, "signer": <u32>, "commitment_hex": "0x…" }`
  - `POST /v1/sumeragi/vrf/reveal` `{ "epoch": <u64>, "signer": <u32>, "reveal_hex": "0x…" }`
- Torii がレート制御と 32 バイト入力の検証を行い、`SumeragiHandle` へ転送。
- `handle_vrf_commit` / `handle_vrf_reveal` がエポックとウィンドウ、コミットの整合性を確認し、進行中のエポック状態を `world.vrf_epochs` に永続化します。
- エポック中の PRF シードは固定され、スナップショットは参加情報のみ更新し、リーダー／コレクタ選定は次エポックまで変わりません。

エポック境界（高さが `epoch_length_blocks` の倍数）でアクターは:
1. ペナルティ（`committed_no_reveal`, `no_participation`）を計算。
2. 有効なリビールを混ぜ合わせて次エポックシード `S_e` を生成。
3. `VrfEpochRecord` を最終化して保存。
4. `epoch_report::VrfPenaltiesReport`、ステータスカウンタ、テレメトリを更新。

更新されたシードは `deterministic_collectors` でコレクター選定に再利用され、`/v1/sumeragi/collectors` で `(height, view)` とともに公開されます。

#### CLI とオペレーター手順
- `iroha_cli ops sumeragi vrf-epoch --epoch <n>` で保存済みシード、参加テーブル、ペナルティ、`commit_deadline_offset`／`reveal_deadline_offset` を確認。`--output-format text` で 1 行表示。
- `iroha_cli --output-format text ops sumeragi telemetry` は最新の投票合計、RBC backlog、VRF 参加サマリ（`reveals_total`, `late_reveals_total`, `committed_no_reveal` など）を返します。`--output-format text` を外すと JSON 全体が得られます。
- `iroha_cli --output-format text ops sumeragi params` で `evidence_horizon_blocks` や `activation_lag_blocks`, `slashing_delay_blocks` を含むコンセンサスパラメータを照合。
- 手動投稿が必要な場合は Torii の POST エンドポイントを利用（自動化が通常は処理）。例:

  ```bash
  curl -X POST "$TORII/v1/sumeragi/vrf/commit" \
    -H "Content-Type: application/json" \
    -d '{"epoch":42,"signer":1,"commitment_hex":"0x..."}'
  curl -X POST "$TORII/v1/sumeragi/vrf/reveal" \
    -H "Content-Type: application/json" \
    -d '{"epoch":42,"signer":1,"reveal_hex":"0x..."}'
  ```

  `/v1/sumeragi/collectors` でコレクター集合 (`collectors[*].peer_id`) を、`/v1/sumeragi/status` で `prf_epoch_seed`, `prf_height`, `vrf_late_reveals_total` 等を確認できます。

**ランダムネス運用チェックリスト**
- 各ブロック後に `iroha_cli --output-format text ops sumeragi status` を確認し、`vrf_penalty_epoch` が跳ねたり `committed_no_reveal` が増えた場合は `iroha_cli --output-format text ops sumeragi vrf-epoch --epoch <n>` で不足しているバリデータを特定。
- コミットウィンドウでは全バリデータがコミットを出したか `iroha_cli --output-format text ops sumeragi telemetry` (`commitments_total`) と `/v1/sumeragi/telemetry` の `vrf.commitments` で確認。
- リビール締め切り前に `vrf.reveals_total` と `vrf_late_reveals_total` を監視。リビール漏れがあれば速やかにバリデータへ通知し、`POST /v1/sumeragi/vrf/reveal` で代理投稿できるよう準備。
- 記録された `commit_deadline_offset`／`reveal_deadline_offset` が想定スケジュールと一致するか照合。ズレは構成ドリフトの兆候。
- 遅延リビールが届いたら seed を控え、`vrf.late_reveals` へ登録されたことと `prf.epoch_seed` が変わらないことを確認。
- エポック rollover 後にペナルティが解消 (`vrf_committed_no_reveal_total` が対象サイナーで 0 へ) されていることを確かめ、`/v1/sumeragi/vrf/epoch/{n}` が `finalized: true` を示すか検証。
- 健全な NPoS では RBC データプレーンが CI の予算を維持します（`integration_tests/tests/sumeragi_npos_happy_path.rs` を参照）。`sumeragi_rbc_deliver_broadcasts_total` が各ブロックで進み、`sumeragi_bg_post_queue_depth` と `sumeragi_bg_post_queue_depth_by_peer` が ≤16 に収まっているか確認。

`integration_tests/tests/sumeragi_npos_performance.rs` の `npos_rbc_store_backpressure_records_metrics` と `npos_rbc_chunk_loss_fault_reports_backlog` が RBC ストア圧力とチャンク損失のテレメトリを検証します。

#### 指標とアラート
- `sumeragi_vrf_commit_emitted_total` / `sumeragi_vrf_reveal_emitted_total` — 受理されたコミット／リビール数。
- `sumeragi_vrf_non_reveal_total` / `sumeragi_vrf_no_participation_total` — 各エポックのペナルティカウンタ。
- `sumeragi_prf_epoch_seed`（`/v1/sumeragi/status`）— 現行シード（16 進文字列から復元して可視化可能）。
- `sumeragi_prf_context_height` / `_view` — コレクター／リーダー抽出に使用された `(height, view)`。
- `/v1/sumeragi/vrf/epoch/{epoch}` — `seed_hex`、参加状況、ペナルティ集計を含むスナップショット。CLI 版は `iroha_cli ops sumeragi vrf-epoch`。
- `/v1/sumeragi/telemetry` の `vrf` セクション — `{found, epoch, finalized, seed_hex, roster_len, participants_total, commitments_total, reveals_total, late_reveals_total, committed_no_reveal[], no_participation[], late_reveals[]}` を返し、リアルタイム参加率を可視化。
- `/v1/sumeragi/status` は `vrf_penalty_epoch`, `vrf_committed_no_reveal_total`, `vrf_no_participation_total`, `vrf_late_reveals_total` を公開。遅延リビールは `sumeragi_vrf_reveals_late_total` と `world.vrf_epochs[*].late_reveals` に記録され、シードに影響しないことが保証されます。

#### VRF アラート閾値
- `increase(sumeragi_vrf_no_participation_total[1h]) > 0` または `increase(sumeragi_vrf_non_reveal_total[1h]) > 0` が継続する場合はページ送信。
- `vrf_late_reveals_total` が急増したら 遅延提出の原因（遅延したバリデータやネットワーク）を調査。
- `sumeragi_prf_epoch_seed` がエポック途中で変化した場合は重大インシデントとして扱い、`vrf-epoch` スナップショットと Torii ログで差分を解析。

### マルチピアソークマトリクス

Milestone A6 の sign-off では `sumeragi_npos_performance.rs` のストレスシナリオを
複数のピア構成（4/6/8 ノード）で実行し、ログとサマリを束ねた sign-off パックを
残す必要があります。以下のスクリプトはすべてのシナリオを実行し、
各シナリオの `summary.json` / README を生成したうえで SRE と共有できる ZIP アーカイブを出力します。

```bash
python3 scripts/run_sumeragi_soak_matrix.py \
  --artifacts-root artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M) \
  --pack artifacts/sumeragi-soak-$(date +%Y%m%d-%H%M)/signoff.zip
```

マトリクスの詳細とチェックリストは
[`sumeragi_soak_matrix.md`](sumeragi_soak_matrix.md) を参照してください。
