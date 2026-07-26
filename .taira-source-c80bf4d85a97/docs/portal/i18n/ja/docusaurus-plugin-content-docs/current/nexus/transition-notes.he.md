---
lang: he
direction: rtl
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/transition-notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 75f6335f4ddd5d98304abe24a20013fdf328e07efd60a1fe44f0ab3fa94593e7
source_last_modified: "2026-01-06T06:54:55+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-transition-notes
title: Nexus 移行ノート
description: `docs/source/nexus_transition_notes.md` のミラーで、Phase B の移行証跡、監査スケジュール、緩和策を扱います。
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# Nexus 移行ノート

このログは **Phase B - Nexus Transition Foundations** の残作業を、multi-lane のローンチチェックリストが完了するまで追跡します。`roadmap.md` のマイルストーン項目を補完し、B1-B4 で参照される証跡を一か所に集約して、governance、SRE、SDK のリードが同じソースを共有できるようにします。

## スコープとカデンス

- routed-trace 監査とテレメトリ guardrails (B1/B2)、governance が承認した config delta セット (B3)、multi-lane ローンチリハーサルのフォローアップ (B4) を対象とします。
- 以前ここにあった一時的な cadence メモを置き換えます。2026 Q1 監査以降、詳細レポートは `docs/source/nexus_routed_trace_audit_report_2026q1.md` に置き、このページは運用スケジュールと緩和策レジスタを保持します。
- routed-trace の各ウィンドウ、governance 投票、ローンチリハーサルのたびに表を更新してください。アーティファクトの場所が変わる場合は、このページに新しい場所を反映し、下流ドキュメント (status, dashboards, SDK portals) が安定したアンカーにリンクできるようにします。

## 証跡スナップショット (2026 Q1-Q2)

| ワークストリーム | 証跡 | オーナー | ステータス | メモ |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | 完了 (Q1 2026) | 3つの監査ウィンドウを記録; `TRACE-CONFIG-DELTA` の TLS ラグは Q2 の rerun で解消。 |
| **B2 - Telemetry remediation & guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | 完了 | Alert pack、diff bot ポリシー、OTLP バッチサイズ (`nexus.scheduler.headroom` log + Grafana headroom panel) を提供; waivers なし。 |
| **B3 - Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | 完了 | GOV-2026-03-19 の投票を記録; 署名済み bundle が下記の telemetry pack を供給。 |
| **B4 - Multi-lane launch rehearsal** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | 完了 (Q2 2026) | Q2 の canary rerun で TLS ラグの緩和をクローズ; validator manifest + `.sha256` が slots 912-936、workload seed `NEXUS-REH-2026Q2`、rerun で記録された TLS プロファイル hash を捕捉。 |

## 四半期 routed-trace 監査スケジュール

| Trace ID | ウィンドウ (UTC) | 結果 | メモ |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | 合格 | Queue-admission P95 は <=750 ms の目標を大きく下回った。対応不要。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | 合格 | OTLP replay hashes を `status.md` に添付; SDK diff bot の parity は drift 0 を確認。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | 解決 | TLS ラグは Q2 rerun で解消; `NEXUS-REH-2026Q2` の telemetry pack に TLS プロファイル hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` を記録 (参照: `artifacts/nexus/tls_profile_rollout_2026q2/`)、ストラグラー 0。 |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | 合格 | Workload seed `NEXUS-REH-2026Q2`; telemetry pack + manifest/digest は `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936)、agenda は `artifacts/nexus/rehearsals/2026q2/`。 |

今後の四半期では新しい行を追加し、表が現行四半期を超える規模になったら完了項目を付録へ移動してください。routed-trace レポートや governance 議事録から参照する際は `#quarterly-routed-trace-audit-schedule` アンカーを使います。

## 緩和策とバックログ項目

| 項目 | 説明 | オーナー | 目標 | ステータス / メモ |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` で遅延した TLS プロファイルの伝播を完了し、rerun の証跡を取得して緩和ログをクローズする。 | @release-eng, @sre-core | Q2 2026 routed-trace ウィンドウ | クローズ - TLS プロファイル hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` を `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` に記録; rerun でストラグラーなしを確認。 |
| `TRACE-MULTILANE-CANARY` prep | Q2 リハーサルのスケジュール、telemetry pack への fixtures 付与、SDK harness が検証済み helper を再利用することの確認。 | @telemetry-ops, SDK Program | 2026-04-30 計画ミーティング | 完了 - agenda を `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` に保存; slot/workload メタデータと harness 再利用を tracker に記載。 |
| Telemetry pack digest rotation | `scripts/telemetry/validate_nexus_telemetry_pack.py` を各リハーサル/リリース前に実行し、config delta tracker の横に digests を記録する。 | @telemetry-ops | リリース候補ごと | 完了 - `telemetry_manifest.json` + `.sha256` を `artifacts/nexus/rehearsals/2026q1/` に出力 (slot range `912-936`, seed `NEXUS-REH-2026Q2`); digests を tracker と証跡インデックスへ反映。 |

## config delta bundle 統合

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` は canonical diff サマリです。新しい `defaults/nexus/*.toml` や genesis 変更が入ったら、まず tracker を更新し、ここに要点を反映します。
- 署名済み config bundles はリハーサル telemetry pack を供給します。`scripts/telemetry/validate_nexus_telemetry_pack.py` で検証された pack を config delta の証跡と一緒に公開し、オペレーターが B4 で使用した正確なアーティファクトを再現できるようにします。
- Iroha 2 bundles は lane なしを維持: `nexus.enabled = false` の configs は lane/dataspace/routing の overrides を拒否するため、Nexus プロファイル (`--sora`) が有効でない場合は single-lane テンプレートから `nexus.*` セクションを削除します。
- governance 投票ログ (GOV-2026-03-19) は tracker とこのメモの両方からリンクし、将来の投票が承認手順を再発見せずに形式を踏襲できるようにします。

## ローンチリハーサルのフォローアップ

- `docs/source/runbooks/nexus_multilane_rehearsal.md` は canary 計画、参加者名簿、rollback 手順をまとめています。lane トポロジーやテレメトリ exporter が変わるたびに runbook を更新します。
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` は 4月9日のリハーサルで検証した全アーティファクトを列挙し、Q2 の準備メモ/agenda を追加しています。証跡を単調に保つため、将来のリハーサルも同じ tracker に追記し、単発 tracker を作らないようにします。
- OTLP collector の snippets と Grafana exports を `docs/source/telemetry.md` に沿って公開し、exporter の batching 指針が変わったら更新します。Q1 更新では headroom アラートを防ぐため batch size を 256 サンプルに引き上げました。
- multi-lane の CI/test 証跡は `integration_tests/tests/nexus/multilane_pipeline.rs` に移行し、`Nexus Multilane Pipeline` workflow (`.github/workflows/integration_tests_multilane.yml`) で実行します。旧 `pytests/nexus/test_multilane_pipeline.py` 参照は廃止済み。`defaults/nexus/config.toml` の hash (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) を tracker と同期させながらリハーサル bundles を更新します。

## ランタイム lane ライフサイクル

- ランタイムの lane ライフサイクル計画は dataspace bindings を検証し、Kura/階層ストレージの reconciliation が失敗すると中断してカタログを変更しません。helpers は廃止 lane の cached relay を削除し、merge-ledger 合成が古い proofs を再利用しないようにします。
- Nexus の config/lifecycle helpers (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) を使って再起動なしで lanes を追加/廃止できます。routing、TEU snapshots、manifest registries は計画成功後に自動リロードされます。
- オペレーター向け: 計画が失敗したら dataspaces の欠落や作成できない storage roots (tiered cold root/Kura lane directories) を確認し、基盤パスを修正して再試行してください。成功した計画は lane/dataspace テレメトリ diff を再送し、dashboards が新しいトポロジーを反映します。

## NPoS テレメトリと backpressure 証跡

Phase B のリハーサル後レビューでは、NPoS pacemaker と gossip 層が backpressure の制限内に収まることを示す決定論的テレメトリの取得が求められました。`integration_tests/tests/sumeragi_npos_performance.rs` の統合ハーネスはこれらのシナリオを実行し、新しいメトリクスが追加されるたびに JSON サマリ (`sumeragi_baseline_summary::<scenario>::...`) を出力します。ローカル実行は以下:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K`, `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` を設定すると高負荷トポロジーを検証できます。デフォルトは B4 で使った 1 s/`k=3` collector プロファイルです。

| シナリオ / test | カバレッジ | 主要テレメトリ |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | リハーサルの block time で 12 ラウンドを実行し、EMA latency の envelope、queue 深度、redundant-send の gauges を記録して証跡 bundle をシリアライズする。 | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | トランザクションキューを飽和させ、admission deferrals が決定論的に発火し、キューが capacity/saturation カウンタを出力することを確認する。 | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | pacemaker jitter と view timeouts をサンプルし、+/-125 permille の帯域が適用されていることを示す。 | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | 大きな RBC payload を store の soft/hard 制限まで投入し、sessions と bytes カウンタが増減して安定し、store を超過しないことを示す。 | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | 再送を強制して redundant-send 比率 gauges と collectors-on-target カウンタを進め、レビューが求めたテレメトリが end-to-end で接続されていることを示す。 | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | 決定論的間隔で chunks を落とし、backlog モニタが静かなドレインではなく fault を上げることを確認する。 | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

ガバナンスが backpressure アラームがリハーサルのトポロジーと一致する証跡を求める場合、harness が出力する JSON 行と実行時の Prometheus scrape を添付してください。

## 更新チェックリスト

1. 新しい routed-trace ウィンドウを追加し、四半期が進んだら古い行を移動する。
2. Alertmanager のフォローアップごとに緩和表を更新し、チケットを閉じる場合でも記録する。
3. config deltas が変わったら tracker、本ノート、telemetry pack の digest 一覧を同じ pull request で更新する。
4. 新しいリハーサル/テレメトリのアーティファクトはここにリンクし、将来の roadmap 更新が散在するメモではなく単一ドキュメントを参照できるようにする。

## 証跡インデックス

| 資産 | 場所 | メモ |
|-------|----------|-------|
| routed-trace 監査レポート (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Phase B1 証跡の正本; ポータル向けに `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md` にミラー。 |
| config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA の diff サマリ、レビュアーのイニシャル、GOV-2026-03-19 投票ログを含む。 |
| telemetry remediation plan | `docs/source/nexus_telemetry_remediation_plan.md` | alert pack、OTLP バッチサイズ、B2 に紐づく export 予算 guardrails を記録。 |
| multi-lane rehearsal tracker | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 4月9日のリハーサルアーティファクト、validator の manifest/digest、Q2 のメモ/agenda、rollback 証跡を列挙。 |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | slot range 912-936、seed `NEXUS-REH-2026Q2`、governance bundles のアーティファクト hash を記録。 |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | Q2 rerun で取得した承認済み TLS プロファイル hash; routed-trace 付録で引用。 |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Q2 リハーサルの計画メモ (ウィンドウ、slot range、workload seed、action owners)。 |
| Launch rehearsal runbook | `docs/source/runbooks/nexus_multilane_rehearsal.md` | staging -> execution -> rollback の運用 checklist; lane トポロジーや exporter ガイダンス変更時に更新。 |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 レトロで参照された CLI; pack 変更時に digests を tracker と一緒に保管。 |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | `nexus.enabled = true` を multi-lane configs で検証し、Sora catalog hashes を保持し、`ConfigLaneRouter` により lane ごとの Kura/merge-log パス (`blocks/lane_{id:03}_{slug}`) を準備してからアーティファクト digests を公開する。 |
