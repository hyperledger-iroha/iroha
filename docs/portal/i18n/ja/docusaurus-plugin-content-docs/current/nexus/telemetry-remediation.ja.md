---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/telemetry-remediation.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: df6d0c02a15a525d87a4f3bba53f3a97b18d76270332680625c8d203399176ff
source_last_modified: "2025-11-14T04:43:20.587026+00:00"
translation_last_reviewed: 2026-01-30
---

# 概要

ロードマップ項目 **B2 - テレメトリギャップのオーナーシップ** は、Q1 2026 の監査ウィンドウ開始前に、残るNexusテレメトリギャップを信号、アラートガードレール、オーナー、期限、検証成果物に結び付けた公開計画を要求します。このページは `docs/source/nexus_telemetry_remediation_plan.md` を反映しており、release engineering、telemetry ops、SDKオーナーが routed-trace と `TRACE-TELEMETRY-BRIDGE` のリハーサルに先立ってカバレッジを確認できます。

# ギャップ行列

| Gap ID | 信号とアラートガードレール | オーナー / エスカレーション | 期限 (UTC) | 証跡と検証 |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | ヒストグラム `torii_lane_admission_latency_seconds{lane_id,endpoint}` とアラート **`SoranetLaneAdmissionLatencyDegraded`**。`histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` が5分続くと発報 (`dashboards/alerts/soranet_lane_rules.yml`)。 | `@torii-sdk` (信号) + `@telemetry-ops` (アラート); Nexus routed-trace on-call でエスカレーション。 | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` のアラートテストに加え、`TRACE-LANE-ROUTING` リハーサルの発報/回復キャプチャと Torii `/metrics` のスクレイプを [Nexus transition notes](./nexus-transition-notes) に保存。 |
| `GAP-TELEM-002` | カウンタ `nexus_config_diff_total{knob,profile}` と guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` によりデプロイをゲート (`docs/source/telemetry.md`)。 | `@nexus-core` (計測) -> `@telemetry-ops` (アラート); カウンタが想定外に増加した場合はガバナンス当番が呼び出し。 | 2026-02-26 | ガバナンス dry-run の出力を `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` の隣に保存。リリースチェックリストに Prometheus クエリのスクリーンショットと `StateTelemetry::record_nexus_config_diff` が差分を出したことを示すログ抜粋を含める。 |
| `GAP-TELEM-003` | イベント `TelemetryEvent::AuditOutcome` (メトリクス `nexus.audit.outcome`) とアラート **`NexusAuditOutcomeFailure`**。失敗または欠落した結果が30分超続く場合に発報 (`dashboards/alerts/nexus_audit_rules.yml`)。 | `@telemetry-ops` (pipeline) から `@sec-observability` へエスカレーション。 | 2026-02-27 | CIゲート `scripts/telemetry/check_nexus_audit_outcome.py` が NDJSON payload を保存し、TRACEウィンドウに成功イベントがない場合に失敗。アラートのスクリーンショットを routed-trace レポートに添付。 |
| `GAP-TELEM-004` | ゲージ `nexus_lane_configured_total` と guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` が SRE on-call チェックリストに反映。 | `@telemetry-ops` (gauge/export) から `@nexus-core` へ。ノードが不整合なカタログサイズを報告した場合にエスカレーション。 | 2026-02-28 | スケジューラのテレメトリテスト `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` が発行を証明。オペレーターは Prometheus diff + `StateTelemetry::set_nexus_catalogs` のログ抜粋を TRACE リハーサルパッケージに添付。 |

# 運用フロー

1. **週次トリアージ。** オーナーは Nexus readiness コールで進捗を報告し、ブロッカーとアラートテスト成果物を `status.md` に記録する。
2. **アラートのドライラン。** 各アラートルールは `dashboards/alerts/tests/*.test.yml` のエントリと一緒に提供し、guardrail が変わるたびに CI が `promtool test rules` を実行する。
3. **監査証跡。** `TRACE-LANE-ROUTING` と `TRACE-TELEMETRY-BRIDGE` のリハーサル中に on-call が Prometheus クエリ結果、アラート履歴、関連スクリプト出力 (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` の相関シグナル) を収集し、routed-trace 成果物と共に保存する。
4. **エスカレーション。** guardrail がリハーサル外で発報した場合、担当チームが本計画を参照した Nexus インシデントチケットを作成し、メトリクスのスナップショットと緩和策を添付して監査を再開する。

この行列を公開し `roadmap.md` と `status.md` から参照することで、ロードマップ項目 **B2** は「責任、期限、アラート、検証」の受け入れ基準を満たします。
