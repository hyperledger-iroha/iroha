<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ja
direction: ltr
source: docs/source/nexus_routed_trace_audit_report_2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 9b77d8021c6e09ba132ba080f183b532b35f8f6293a13497646566d13e932306
source_last_modified: "2025-11-22T12:03:01.494516+00:00"
translation_last_reviewed: 2026-01-01
---

# 2026 Q1 Routed-Trace 監査レポート (B1)

ロードマップ項目 **B1 — Routed-Trace Audits & Telemetry Baseline** は、Nexus の routed-trace
プログラムに対する四半期レビューを要求する。本レポートは 2026 Q1 の監査ウィンドウ
（1月〜3月）を記録し、Q2 のローンチリハーサル前にガバナンス評議会が
テレメトリ体制を承認できるようにする。

## 対象範囲とタイムライン

| Trace ID | ウィンドウ (UTC) | 目的 |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00–09:45 | マルチレーン有効化前に、レーン入場ヒストグラム、キューの gossip、アラートフローを検証する。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00–10:45 | AND4/AND7 マイルストーン前に、OTLP リプレイ、diff bot のパリティ、SDK テレメトリ取り込みを検証する。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00–12:30 | RC1 カット前に、ガバナンス承認済みの `iroha_config` 差分とロールバック準備を確認する。 |

各リハーサルは production に近いトポロジで実行され、routed-trace
インストルメンテーション（`nexus.audit.outcome` テレメトリ + Prometheus カウンタ）が
有効化され、Alertmanager ルールを読み込み、証拠を `docs/examples/` にエクスポートした。

## 方法論

1. **テレメトリ収集。** 全ノードが構造化イベント `nexus.audit.outcome` と付随メトリクス
   (`nexus_audit_outcome_total*`) を発行した。ヘルパー
   `scripts/telemetry/check_nexus_audit_outcome.py` が JSON ログを追跡し、イベントステータスを
   検証して `docs/examples/nexus_audit_outcomes/` に payload をアーカイブした
   (`scripts/telemetry/check_nexus_audit_outcome.py:1`).
2. **アラート検証。** `dashboards/alerts/nexus_audit_rules.yml` とそのテストハーネスにより、
   ノイズ閾値と payload テンプレートの一貫性を確保した。CI は変更のたびに
   `dashboards/alerts/tests/nexus_audit_rules.test.yml` を実行し、各ウィンドウでも手動で
   同じルールを検証した。
3. **ダッシュボード捕捉。** 運用者は `dashboards/grafana/soranet_sn16_handshake.json`
   （ハンドシェイク健全性）とテレメトリ概要ダッシュボードをエクスポートし、
   キュー健全性と監査結果の相関を確認した。
4. **レビューノート。** ガバナンス事務局がレビュアーのイニシャル、判断、
   ミティゲーションチケットを `docs/source/nexus_transition_notes.md` と
   コンフィグ差分トラッカー (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)
   に記録した。

## 所見

| Trace ID | 結果 | 証拠 | 注記 |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | アラート発火/回復のスクリーンショット（内部リンク）+ `dashboards/alerts/tests/soranet_lane_rules.test.yml` のリプレイ; テレメトリ差分は `docs/source/nexus_transition_notes.md#quarterly-routed-trace-audit-schedule` に記録。 | キュー入場 P95 は 612 ms（目標 <=750 ms）。追跡不要。 |
| `TRACE-TELEMETRY-BRIDGE` | Pass | アーカイブ済み payload `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` と OTLP リプレイ hash を `status.md` に記録。 | SDK の redaction salt は Rust baseline と一致し、diff bot は差分ゼロを報告。 |
| `TRACE-CONFIG-DELTA` | Pass（緩和完了） | ガバナンス トラッカーの記録 (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS プロファイル manifest (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + テレメトリパック manifest (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Q2 再実行で承認済み TLS プロファイルをハッシュ化し、stragglers なしを確認; テレメトリ manifest は slot range 912–936 と workload seed `NEXUS-REH-2026Q2` を記録。 |

全ての trace がウィンドウ内に少なくとも 1 件の `nexus.audit.outcome` イベントを
生成し、Alertmanager guardrail（`NexusAuditOutcomeFailure`）は四半期を通じて
グリーンを維持した。

## フォローアップ

- routed-trace 付録を TLS hash `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`
  で更新（`nexus_transition_notes.md` 参照）、緩和 `NEXUS-421` をクローズ。
- 生の OTLP リプレイと Torii diff アーティファクトを継続的にアーカイブし、
  Android AND4/AND7 レビュー向けのパリティ証拠を強化する。
- 今後の `TRACE-MULTILANE-CANARY` リハーサルが同一のテレメトリヘルパーを再利用し、
  Q2 サインオフが検証済みワークフローの恩恵を受けることを確認する。

## アーティファクト一覧

| 資産 | 場所 |
|-------|----------|
| テレメトリ検証ツール | `scripts/telemetry/check_nexus_audit_outcome.py` |
| アラートルールとテスト | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| サンプル outcome payload | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| Config delta tracker | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| Routed-trace スケジュールとノート | `docs/source/nexus_transition_notes.md` |

このレポート、上記アーティファクト、およびアラート/テレメトリのエクスポートは、
B1 を四半期でクローズするためにガバナンスの決定ログへ添付すること。
