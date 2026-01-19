# テレメトリとメトリクス概要（日本語版）

本書は Iroha が提供する Prometheus 互換メトリクスと JSON ベースのステータス API を日本語でまとめたものです。英語の詳細は [`telemetry.md`](./telemetry.md) を参照してください。

## 提供エンドポイント

- `/metrics`: Prometheus 書式のテキスト。テレメトリ無効化時やプロファイルで制限されている場合は非公開。
- `/status`: JSON ステータス。`sumeragi`（リーダー、Highest/Locked QC（`highest_qc`/`locked_qc`）、ビュー変更証明カウンタ（`view_change_proof_{accepted,stale,rejected}_total`）、ゴシップフォールバック、トランザクションキュー深さ、エポック情報 `epoch { length_blocks, commit_deadline_offset, reveal_deadline_offset }`、RBC ストア統計〔`sessions`, `bytes`, `pressure_level`, `backpressure_deferrals_total`, `persist_drops_total`, `evictions_total`, `recent_evictions`〕、PRF シード／高さ／ビュー など）や `governance` のスナップショットを含む。
- `/v1/sumeragi/new_view` / `/v1/sumeragi/new_view/sse`: NEW_VIEW 受信数を JSON / SSE で取得（固定サイズのメモリ内ウィンドウで保持し、古い `(height, view)` は破棄）。
- `/v1/sumeragi/status` / `/v1/sumeragi/status/sse`: コンセンサス状態を JSON または Norito で取得。JSON 版には `epoch { length_blocks, commit_deadline_offset, reveal_deadline_offset }` に加え、`da_reschedule_total`、`lane_governance_sealed_total` / `lane_governance_sealed_aliases`、`worker_loop { stage, stage_started_ms, last_iteration_ms, queue_depths { vote_rx, block_payload_rx, rbc_chunk_rx, block_rx, consensus_rx, lane_relay_rx, background_rx } }` などの集約メトリクスが含まれる。
- `/v1/sumeragi/rbc` / `/v1/sumeragi/rbc/sessions`: RBC セッションの統計と詳細（`lane_backlog` / `dataspace_backlog` を含む）。
- `/v1/sumeragi/pacemaker`: ビュータイマー設定と現在値。
- `/v1/sumeragi/collectors`: コレクタ計画（`mode`, `(height, view)`, `collectors_k`, `redundant_send_r`, `proxy_tail_index`, `min_votes_for_commit`, Collector 一覧, NPoS 時は `epoch_seed`）。
- `/v1/sumeragi/params`: コミット済み Sumeragi パラメータのスナップショット。
- `/v1/sumeragi/phases`: コンセンサス各フェーズの最新レイテンシ。

集約フィールド `lane_governance_sealed_total` と `lane_governance_sealed_aliases` は、ガバナンス・マニフェストが未適用のレーンを一目で把握するためのカウンタです。`/v1/sumeragi/status` に加えて `iroha_cli sumeragi status --summary` でも同じ値が表示され、CLI はエイリアス一覧をそのまま印字します。ローンチ手順や CI/CD では `iroha_cli nexus lane-report --only-missing --fail-on-sealed` を組み合わせ、シール解除が完了していない場合は早期に検知できるようにしてください。

## 設定項目

- `telemetry_enabled`（既定値: true）: テレメトリ全体を有効／無効化。
- `telemetry_profile`（既定値: `operator`）: 収集対象やルート公開範囲を決定するプリセット。`metrics` / `expensive_metrics` / `developer_outputs` の 3 フラグを束ねる。

| プロファイル       | `/status` | `/metrics` | `/v1/sumeragi/*` / SSE | 想定利用シナリオ       |
|--------------------|-----------|------------|------------------------|-------------------------|
| `disabled`         | ×         | ×          | ×                      | 完全に収集しない        |
| `operator`         | ○         | ×          | ×                      | 本番ノード／JSON モニタ |
| `extended`         | ○         | ○          | ×                      | Prometheus 収集あり     |
| `developer`        | ○         | ×          | ○                      | ローカル検証／開発用    |
| `full`             | ○         | ○          | ○                      | 開発＋運用両対応        |

## テレメトリ匿名化と整合性

- `telemetry_redaction.mode` は `strict|allowlist|disabled`。`operator`/`extended`/`full` では `strict` が必須です。
- センシティブ判定は接頭辞とキーワードの一覧に基づきます。詳細な一覧は `docs/source/telemetry.md` を参照してください。
- `telemetry_integrity.enabled` を有効にすると `chain { seq, prev_hash, hash, signature?, key_id? }` が追加されます。`telemetry_integrity.signing_key_hex` を設定すると keyed Blake3 署名が付与されます。
- 継続性が必要な場合は `telemetry_integrity.state_dir` を設定し、再起動後もシーケンスを継続します。

## プラットフォーム別テレメトリプロファイル

### Android SDK (AND7)

ロードマップ項目 **NRPC/AND7** では、Android SDK が Rust ノードと同一のテレメトリ／プライバシー準拠状態を示すことが求められています。SRE ガバナンス審査や運用向けブリーフィングでは、下記アセット群を必ず添付してください。

| アセット | 役割 |
|----------|------|
| [`sdk/android/telemetry_redaction.md`](./sdk/android/telemetry_redaction.md) | ハッシュ済み authority、デバイス区分、保持期間、オーバーライド手順など AND7 の公式ポリシー。 |
| [`android_runbook.md`](./android_runbook.md#2-telemetry--redaction) | `ClientConfig` のスレッディング、エクスポータ確認、オーバーライド対応など運用手順。 |
| [`sdk/android/readiness/signal_inventory_worksheet.md`](./sdk/android/readiness/signal_inventory_worksheet.md) | 各シグナルの担当者、検証フック、証跡リンクをまとめたオーナー表。 |
| [`sdk/android/telemetry_chaos_checklist.md`](./sdk/android/telemetry_chaos_checklist.md) | 四半期ごとのカオステスト台本。SRE ガバナンスで参照されます。 |
| [`sdk/android/readiness/and7_operator_enablement.md`](./sdk/android/readiness/and7_operator_enablement.md) | サポート／当番向けトレーニングカリキュラムとナレッジチェック計画。 |

**主なガードレール**

- `android.torii.http.request` / `android.torii.http.retry` は `iroha_config.telemetry.redaction_salt` で公開される Blake2b-256 ソルトを使って authority をハッシュ化します。ソルトローテーション時は `android.telemetry.redaction.salt_version` でドリフトを検知してください。
- デバイス情報は `android.telemetry.device_profile` のバケット（SDK メジャー＋`emulator|consumer|enterprise`）に限定されます。Rust ノードの `node.hardware_profile` と比較し、10% を超える乖離が続く場合はスキーマ差分調査が必要です。
- ネットワーク情報には `network_type` と `roaming` しか含まれません。加入者情報を求めるリクエストはオーバーライド手順（Android サポートプレイブック参照）でのみ扱います。
- オーバーライドは `android.telemetry.redaction.override` シグナルと `docs/source/sdk/android/telemetry_override_log.md` で記録し、適用／解除の都度ログを更新します。

**運用検証のヒント**

- `scripts/telemetry/check_redaction_status.py` をステージング環境に対して実行し、カオス演習やインシデントの証跡に添付するステータスバンドルを生成します。
- カオス演習の報告には Grafana スクリーンショットに加え、`android.telemetry.redaction.failure` と `android.telemetry.redaction.override` の最新カウンタを含め、`docs/source/sdk/android/readiness/labs/` 配下に保管します。
- 代表的な PromQL:
  - `increase(android.telemetry.redaction.failure_total[5m]) > 0` はカオス期間外でページングすべき条件。
  - `sum by (device_profile)(android.telemetry.device_profile)` と Rust 側の `node.hardware_profile` ヒストグラムを突き合わせてバケットの整合を確認。
  - `clamp_min(rate(android.telemetry.redaction.override_total[1h]), 0)` を月次オーバーライド監査の入力に利用。

サポート体制やエスカレーション手順は [`android_support_playbook.md`](./android_support_playbook.md#8-telemetry-redaction--observability-and7) を参照してください。

## ガバナンス関連メトリクス

- `governance_proposals_status{status}`: 提案の状態別カウント（`proposed`, `approved`, `rejected`, `enacted`）。
- `governance_protected_namespace_total{outcome}`: 保護ネームスペースの登録結果。
- `governance_manifest_admission_total{result}`: マニフェストによるキュー判定の結果。`result` ラベルで `allowed` / `missing_manifest` / `non_validator_authority` / `quorum_rejected` / `protected_namespace_rejected` / `runtime_hook_rejected` を区別する。
- `governance_manifest_quorum_total{outcome}`: マニフェストに定義されたバリデータ定足数のチェック結果 (`outcome="satisfied"` / `"rejected"`)。
- `governance_manifest_hook_total{hook, outcome}`: ガバナンスフックの適用状況。現在は `hook="runtime_upgrade"` の `allowed` / `rejected` を記録し、アップグレード申請が manifest ポリシーを満たしたかを可視化する。
- `/status` JSON の `governance` セクションと整合性が取られており、再起動後も WSV から復元される。

## パイプライン／IVM 指標

- `pipeline_stage_ms_bucket`: 各ステージのレイテンシーヒストグラム。`histogram_quantile` で P50/P90/P95 を算出。
- `pipeline_dag_vertices` / `pipeline_dag_edges` / `pipeline_conflict_rate_bps`: 最新ブロックの DAG 形状と衝突率（bp）。
- `pipeline_access_set_source_total{source=manifest_hints|entrypoint_hints|prepass_merge|conservative_fallback}`: アクセスセット導出元別の累積カウント。
- `commit_time_ms_bucket`: コミット時間ヒストグラム。
- `ivm_cache_hits` / `ivm_cache_misses` / `ivm_cache_evictions`: IVM プリデコードキャッシュのヒット率に利用。
- `ivm_register_max_index_bucket` / `ivm_register_unique_count_bucket`: VM 実行中に利用された GPR の最大値／ユニーク数を観測し、ホットレジスタ帯の逸脱を検知。

## Sumeragi（コンセンサス）指標

- 進行状況: `block_height`, `block_height_non_empty`, `commit_time_ms_bucket`, `sumeragi_cert_size_bucket`。
- Tail 投票やビュー変更: `sumeragi_tail_votes_total`, `sumeragi_widen_before_rotate_total`, `sumeragi_view_change_suggest_total`, `sumeragi_view_change_install_total`。
- コレクタ関連:
  - `sumeragi_collectors_targeted_current`: 現在のブロックでターゲット済みのコレクタ数（ゲージ）。
  - `sumeragi_collectors_targeted_per_block_bucket`: ブロック単位のコレクタ数を記録するヒストグラム。
  - `sumeragi_redundant_sends_total`, `sumeragi_redundant_sends_by_peer`, `sumeragi_redundant_sends_by_collector`: 冗長送信の総量／peer 別／collector インデックス別カウンタ。
- DA 可用性警告: `sumeragi_da_gate_block_total{reason="missing_local_data"}` で可用性証跡の不足を追跡します。`sumeragi_rbc_da_reschedule_total` と `/v1/sumeragi/status` の `da_reschedule_total` はレガシーで、通常 0 のままです。
- チャネル圧力: `sumeragi_dropped_block_messages_total`, `sumeragi_dropped_control_messages_total`, `dropped_messages`。
- VRF 活動: `sumeragi_vrf_commits_emitted_total`, `sumeragi_vrf_reveals_emitted_total`, `sumeragi_vrf_non_reveal_*`。
- NEW_VIEW 可視化: `sumeragi_new_view_receipts_by_hv{height,view}`, `sumeragi_new_view_dropped_by_lock_total`。
- Pacemaker: `sumeragi_pacemaker_backoff_ms`, `sumeragi_pacemaker_rtt_floor_ms`, `sumeragi_pacemaker_jitter_ms` など。
- メンバーシップ整合性: `sumeragi_membership_view_hash`, `sumeragi_membership_height`, `sumeragi_membership_view`, `sumeragi_membership_epoch` は `(height, view, epoch)` に結びついた決定論的ロスターハッシュを公開します。ピア間で値を比較することで、警告を待たずにロスターの不一致を特定できます。

### 推奨 PromQL 例

- コレクタ分布 (p95):  
  `histogram_quantile(0.95, sum by (le) (rate(sumeragi_collectors_targeted_per_block_bucket[5m])))`
- 冗長送信スパイク検知:  
  `rate(sumeragi_redundant_sends_total[5m])`
- トップ N peer の冗長送信:  
  `topk(5, sum by (peer) (rate(sumeragi_redundant_sends_by_peer[5m])))`
- チャネルドロップ監視:  
  `rate(sumeragi_dropped_block_messages_total[5m])`, `rate(sumeragi_dropped_control_messages_total[5m])`
- コミット時間 P95:  
  `histogram_quantile(0.95, sum(rate(commit_time_ms_bucket[5m])) by (le))`
- IVM キャッシュヒット率 (%):  
  `100 * (ivm_cache_hits - ivm_cache_hits offset 5m) / clamp_min((ivm_cache_hits - ivm_cache_hits offset 5m) + (ivm_cache_misses - ivm_cache_misses offset 5m), 1)`

## `/v1/sumeragi/phases` の利用

- 返却値: `{ propose_ms, collect_da_ms, collect_prevote_ms, collect_precommit_ms, collect_aggregator_ms, commit_ms, pipeline_total_ms, ema_ms{…} }`
- `collect_aggregator_ms` は冗長コレクタへのファンアウト遅延を示す。`sumeragi_redundant_sends_*` と組み合わせてアラート設定を調整。
- `pipeline_total_ms` は Pacemaker が制御するフェーズ（propose/collect_da/collect_prevote/collect_precommit/commit）の合計遅延を表す。監視やしきい値調整に便利な単一指標として利用できる（`collect_aggregator_ms` はファンアウト指標として別扱い）。
- 付随情報として `block_created_dropped_by_lock_total`, `block_created_hint_mismatch_total`, `block_created_proposal_mismatch_total` も含まれる。

## Nexus メトリクス

### 設定ドリフト監視

- `nexus_config_diff_total{knob,profile}`（カウンタ）: 単一レーンの既定設定からアクティブ構成が逸脱するたびに増分されます。`knob` ラベルは変更された領域（例: `nexus.lane_catalog.count`, `nexus.routing.rules`, `nexus.da`）を示し、`profile` は現在のプロファイル (`active`) です。

**アラート例**

```
increase(nexus_config_diff_total{profile="active"}[5m]) > 0
```

計画されたメンテナンスウィンドウ外で増加した場合はページングしてください。増加時には `telemetry` ログに `nexus.config.diff` イベント（Norito JSON で `baseline` と `current` の値を格納）が記録されるため、設定レビューや `TRACE-TELEMETRY-BRIDGE` リハーサル時に参照できます。

- `nexus_lane_configured_total`（ゲージ）: ノードが適用した Nexus レーンカタログのエントリ数を報告します。単一レーンバンドルでは `1`、Nexus マルチレーン構成では `3` といった期待値と比較し、誤設定されたピアを早期に検出します。

### Torii Norito-RPC の可観測性

Norito-RPC トランスポート向けのメトリクス要件は `docs/source/torii/norito_rpc_telemetry.md` に記載されています。主な項目は以下のとおりです。

- `torii_request_duration_seconds_bucket{scheme}` — 方式別レイテンシーヒストグラム。`scheme="norito_rpc"` を抽出すると burn-in パネルを直接作成できます。
- `torii_request_failures_total{scheme,code}` — スキームと HTTP ステータスで分類されたエラー回数。
- `torii_http_requests_total{content_type,status,method}` — `application/x-norito` と JSON を区別するリクエストカウンタ。
- `torii_http_request_duration_seconds_bucket{content_type,method}` — コンテントタイプ別のレイテンシー比較に使用。
- `torii_http_response_bytes_total{content_type,method,status}` — ペイロードサイズのドリフト検知。
- `torii_norito_decode_failures_total{payload_kind,reason}` — マジック不一致や CRC エラーなどデコード失敗の理由別カウンタ。
- `torii_address_invalid_total{surface,reason}` / `torii_address_local8_total{surface}` — Norito 経由でも IH58/圧縮エラーを表す。
- 既存の `torii_active_connections_total{scheme}` と `torii_pre_auth_reject_total{reason}` に `scheme="norito_rpc"` ラベルを必ず加えてください。

アラートルールは `dashboards/alerts/torii_norito_rpc_rules.yml`（テスト: `dashboards/alerts/tests/torii_norito_rpc_rules.test.yml`）に定義されています。

- `ToriiNoritoRpcErrorSpike`: 5xx が 5 分間に 5 件を超えた場合。
- `ToriiNoritoRpcLatencyDegraded`: `histogram_quantile(0.95, …)` が 750 ms を超えた場合。
- `ToriiNoritoRpcSilentTraffic`: 30 分間に Norito リクエストが 1 件も観測されない場合。

ルール更新時は `scripts/telemetry/test_torii_norito_rpc_alerts.sh` で `promtool test rules` を実行し、CI とローカルの両方で整合性を確保してください。

#### Norito RPC 障害時ランブック

Norito トラフィックの SLO を下回ったり、上記アラートが発火した場合は次の手順で調査します。

- **主要シグナル:** `ToriiNoritoRpcErrorSpike`, `ToriiNoritoRpcLatencyDegraded`, `ToriiNoritoRpcSilentTraffic`, `torii_norito_decode_failures_total`, `torii_request_duration_seconds_bucket{scheme="norito_rpc"}`。これらは `dashboards/grafana/torii_norito_rpc_observability.json` にまとめられています。
- **クライアント側の検証:** SDK CI とモックハーネスが `torii_mock_harness_retry_total`, `torii_mock_harness_duration_ms`, `torii_mock_harness_fixture_version` を公開します。まずクライアントで退行が起きていないか確認してください。
- **サポートツール:** `python/iroha_python/scripts/run_norito_rpc_smoke.sh`（E2E 確認）と `scripts/telemetry/test_torii_norito_rpc_alerts.sh`（ルール検証）。

**一次対応**

1. 発生タイプを切り分ける。エラー増加（`torii_request_failures_total`）、レイテンシー増加（P95 > 750 ms）、トラフィック消失（JSON は生きているが Norito がゼロ）など。
2. `ConnScheme::NoritoRpc` でフィルタした Torii ログを参照し、`schema_mismatch` / TLS / CRC エラーを特定。
3. `curl -H 'Content-Type: application/x-norito' https://.../rpc/ping` あるいはモックハーネスで、リバースプロキシがヘッダーをドロップしていないかを確認。
4. サーバー側が健全だが SDK メトリクスだけが異常な場合は、リリースを一時停止してクライアントオーナーにエスカレーションする。

**緩和策**

- 影響を与えるクライアントだけを `torii.preauth_scheme_limits.norito_rpc` でレート制限。
- スキーマや fixture mismatch の疑いがある場合は `fixtures/norito_rpc/schema_hashes.json`（DTO→hash テーブル）を比較し、`cargo xtask norito-rpc-fixtures --all` で再生成。
- Ingress/MTU 問題は修正後に smoke テストを再実行。
- 大規模障害で JSON へ戻す場合は `torii.transport.norito_rpc.stage` を `canary` または `disabled` に戻し、`/rpc/capabilities` が新しいステージを返すことを確認後、`docs/source/runbooks/torii_norito_rpc_canary.md` の手順に従って復旧記録を残す。

**復旧確認**

1. `torii_request_failures_total` と `torii_norito_decode_failures_total` がベースラインに戻ったことを監視。
2. `torii_active_connections_total{scheme="norito_rpc"}` とサイレントトラフィックアラートが安定化したことを確認。
3. `python/iroha_python/scripts/run_norito_rpc_smoke.sh` とルールテストを再度実行。
4. ダッシュボードのスクリーンショットや CLI 出力を保存し、NRPC-2 関連のチケット／`status.md` に証跡を残す。

**アラート例**

```
nexus_lane_configured_total != EXPECTED_LANE_COUNT
```

### ルーテッドトレース監査結果

- `Telemetry::record_audit_outcome` は監査ウィンドウが完了するたびに `nexus.audit.outcome` テレメトリーログを出力します。Norito ペイロードには `trace_id`, `slot_height`, `reviewer`, `status`（例: `pass` / `fail` / `mitigated`）、および任意の `mitigation_url` が含まれます。
- Prometheus では `nexus_audit_outcome_total{trace_id,status}` と `nexus_audit_outcome_last_timestamp_seconds{trace_id}` が公開され、ダッシュボード／アラートから監視できます。

**運用フロー**
1. TRACE リハーサル中は `journalctl -u irohad -o json` あるいは OTLP ブリッジを監視し、各スケジュール済みウィンドウに対してイベントが記録されることを確認します。
2. 取得した JSON ペイロードを監査成果物と一緒に保存し、レビュー担当者が結論と緩和手順を追跡できるようにします。
3. `scripts/telemetry/check_nexus_audit_outcome.py`（例: `--trace-id TRACE-TELEMETRY-BRIDGE --window-start 2026-02-24T10:00Z --window-minutes 30`）を実行し、イベントの存在とステータスを検証のうえ `docs/examples/nexus_audit_outcomes/` にスナップショットを残してください。`fail`（初期値）などの禁止ステータスは即座に失敗として扱われます。
4. `status="fail"` が現れた場合、もしくは想定されたスロットから 30 分以内にイベントが現れない場合にはページングするアラートルールを整備します。Prometheus 側では `dashboards/alerts/nexus_audit_rules.yml` が失敗ステータスを監視し、欠落イベントは上記スクリプトを CI/TRACE ジョブに組み込むことで自動検知します。

## アラート構築のヒント

- `increase(block_created_hint_mismatch_total[5m]) > 0`: 提案ヘッダの不整合
- `increase(block_created_proposal_mismatch_total[5m]) > 0`: 提案ペイロードの不一致
- `increase(block_created_dropped_by_lock_total[5m]) > 0`: Locked commit certificate によるドロップ
- `increase(pacemaker_backpressure_deferrals_total[5m]) > 0`: キュー飽和・リレー/RBC バックログ・保留ブロック滞留による Pacemaker 停止
- `increase(sumeragi_redundant_sends_total[5m])` と `rate(sumeragi_dropped_*[5m])` の同時増加は collector/チャネル設定の見直しを示唆
- ログ相関の自動化: `python3 scripts/sumeragi_backpressure_log_scraper.py <logfile>` を実行すると、`pacemaker_backpressure_deferrals_total` のスパイクと RBC の `retry` / `abort` ログをまとめて確認できます。`journalctl -f ... | python3 scripts/sumeragi_backpressure_log_scraper.py -` のように標準入力からも扱え、`--status path/to/status.json` で `/v1/sumeragi/status` スナップショットを併せて把握できます。詳細はスクリプトの `--help` と英語版ランブックを参照してください。

## ガバナンス運用チェックリスト（抜粋）

1. `iroha_cli gov audit-deploy` でマニフェストと `code_hash` / `abi_hash` の整合性確認。
2. `/status` の `governance.recent_manifest_activations` を確認し、対象のアップグレードが反映されているか確認。
3. `increase(governance_protected_namespace_total{outcome="rejected"}[5m])` や `increase(governance_manifest_quorum_total{outcome="rejected"}[5m])` が増加した場合は Torii ログで拒否理由を調査。
4. `governance_proposals_status{status="approved"}` と `status="enacted"` のカウント推移を監視し、停滞時は `iroha_cli gov enact` の再実行を検討。
5. `increase(governance_manifest_hook_total{hook="runtime_upgrade", outcome="rejected"}[5m])` をチェックし、マニフェストポリシーにより拒否されたランタイムアップグレード申請を検出する。増加が見られた場合は Torii のアドミッションログと突き合わせ、allowlist やメタデータ要件が提案内容と一致しているか再確認する。

---

より詳しい手順や追加のメトリクスは英語版の [`telemetry.md`](./telemetry.md) を参照してください。翻訳内容に誤りを見つけた場合は Issue もしくは Pull Request でお知らせください。
