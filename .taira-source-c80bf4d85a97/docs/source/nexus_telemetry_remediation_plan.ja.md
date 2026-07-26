---
lang: ja
direction: ltr
source: docs/source/nexus_telemetry_remediation_plan.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 19d46f99e2ba79c56cbc3af65b47f5fb6997fa66f8ee951806b21696418a1d7b
source_last_modified: "2025-11-27T14:13:33.645951+00:00"
translation_last_reviewed: 2026-01-01
---

% Nexus テレメトリ是正計画（Phase B2）

# 概要

ロードマップ項目 **B2 - telemetry gap ownership** は、未解消の Nexus テレメトリギャップを
信号、アラートの guardrail、担当、期限、検証アーティファクトにひも付けた公開計画を
Q1 2026 の監査ウィンドウ開始前に整備することを要求する。本書はそのマトリクスを集中管理し、
release engineering、telemetry ops、SDK オーナーが routed-trace と
`TRACE-TELEMETRY-BRIDGE` のリハーサル前にカバレッジを確認できるようにする。

# ギャップマトリクス

| Gap ID | シグナルとアラート guardrail | Owner / Escalation | 期限 (UTC) | 証拠と検証 |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | `torii_lane_admission_latency_seconds{lane_id,endpoint}` ヒストグラムとアラート **`SoranetLaneAdmissionLatencyDegraded`**。`histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` が 5 分継続した場合に発火（`dashboards/alerts/soranet_lane_rules.yml`）。 | `@torii-sdk`（シグナル）+ `@telemetry-ops`（アラート）- Nexus routed-trace オンコールにエスカレーション。 | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` のアラートテストと、発火/復旧を示す `TRACE-LANE-ROUTING` リハーサルのキャプチャ、Torii `/metrics` のスクレイプを `docs/source/nexus_transition_notes.md` にアーカイブ。 |
| `GAP-TELEM-002` | カウンタ `nexus_config_diff_total{knob,profile}` と guardrail `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` によりデプロイをゲート（`docs/source/telemetry.md`）。 | `@nexus-core`（計測）-> `@telemetry-ops`（アラート）- 予期せぬ増加時は governance duty officer に通知。 | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` に governance dry-run の出力を保存。リリースチェックリストに Prometheus クエリのスクリーンショットと、`StateTelemetry::record_nexus_config_diff` の発火を示すログ抜粋を添付。 |
| `GAP-TELEM-003` | イベント `TelemetryEvent::AuditOutcome`（メトリクス `nexus.audit.outcome`）とアラート **`NexusAuditOutcomeFailure`**。失敗または欠落が 30 分超続くと発火（`dashboards/alerts/nexus_audit_rules.yml`）。 | `@telemetry-ops`（パイプライン）から `@sec-observability` へエスカレーション。 | 2026-02-27 | CI ゲート `scripts/telemetry/check_nexus_audit_outcome.py` が NDJSON をアーカイブし、TRACE ウィンドウで成功イベントが欠けると失敗。アラートのスクリーンショットを routed-trace レポートに添付。 |
| `GAP-TELEM-004` | ゲージ `nexus_lane_configured_total` を guardrail `nexus_lane_configured_total != EXPECTED_LANE_COUNT` で監視（`docs/source/telemetry.md` に記載）。SRE オンコールのチェックリストに供給。 | `@telemetry-ops`（gauge/export）から `@nexus-core` へエスカレーション。ノードが不一致のカタログサイズを報告した場合に対応。 | 2026-02-28 | scheduler テレメトリテスト `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` が発行を証明。運用者は Prometheus diff と `StateTelemetry::set_nexus_catalogs` のログ抜粋を TRACE リハーサルパッケージに添付。 |

# Export 予算と OTLP 制限

- **決定 (2026-02-11):** OTLP エクスポータを **1 ノードあたり 5 MiB/min** または
  **25,000 spans/min** の低い方に制限。256 spans のバッチサイズと 10 秒のエクスポート
  タイムアウトを設定。上限の 80% を超えると `dashboards/alerts/nexus_telemetry_rules.yml`
  の `NexusOtelExporterSaturated` アラートが発火し、監査ログ向けに
  `telemetry_export_budget_saturation` イベントを出力。
- **強制:** Prometheus ルールが `iroha.telemetry.export.bytes_total` と
  `iroha.telemetry.export.spans_total` カウンタを読み取る。OTLP collector プロファイルは同一の
  既定値を出荷し、ノード設定でガバナンス免除なしに引き上げてはならない。
- **証拠:** アラートテストベクタと承認済み制限は `docs/source/nexus_transition_notes.md` に
  routed-trace 監査アーティファクトと併せて保管。B2 受け入れは export 予算をクローズ扱い。

# 運用ワークフロー

1. **週次トリアージ。** オーナーは Nexus readiness コールで進捗を報告し、ブロッカーと
   アラートテストの成果物を `status.md` に記録する。
2. **アラート dry-run。** 各アラートルールは `dashboards/alerts/tests/*.test.yml` のエントリと
   あわせて提供され、guardrail 変更時に CI が `promtool test rules` を実行する。
3. **監査証拠。** `TRACE-LANE-ROUTING` と `TRACE-TELEMETRY-BRIDGE` のリハーサル中にオンコールが
   Prometheus クエリ結果、アラート履歴、関連スクリプト出力
   (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py`
   など) を取得し、routed-trace のアーティファクトと一緒に保存する。
4. **エスカレーション。** リハーサル外で guardrail が発火した場合、担当チームは本計画を
   参照した Nexus インシデントチケットを作成し、メトリクスのスナップショットと
   緩和手順を添えて監査を再開する。

このマトリクスを公開し `roadmap.md` と `status.md` から参照したことで、ロードマップ項目
**B2** は「責任、期限、アラート、検証」の受け入れ基準を満たす。
