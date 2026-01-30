---
lang: ja
direction: ltr
source: docs/portal/i18n/ja/docusaurus-plugin-content-docs/current/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 63079a25584c7c2f19753685f59e3c504545756ad78965a4ba4d3760e2ec2d3d
source_last_modified: "2026-01-03T18:08:02+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-routed-trace-audit-2026q1
title: 2026 Q1 routed-trace 監査レポート (B1)
description: `docs/source/nexus_routed_trace_audit_report_2026q1.md` のミラーで、四半期テレメトリリハーサルの結果をまとめます。
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note 正本
このページは `docs/source/nexus_routed_trace_audit_report_2026q1.md` を反映しています。残りの翻訳が完了するまで両方のコピーを揃えてください。
:::

# 2026 Q1 Routed-Trace 監査レポート (B1)

ロードマップ項目 **B1 - Routed-Trace Audits & Telemetry Baseline** は、Nexus routed-trace プログラムの四半期レビューを要求します。本レポートは Q1 2026 (1月-3月) の監査ウィンドウを記録し、ガバナンス評議会が Q2 ローンチリハーサルの前にテレメトリ態勢を承認できるようにします。

## 範囲とタイムライン

| Trace ID | Window (UTC) | 目的 |
|----------|--------------|-----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | multi-lane 有効化前に lane 入場ヒストグラム、キュー gossip、アラートフローを検証する。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | AND4/AND7 マイルストーン前に OTLP リプレイ、diff bot のパリティ、SDK テレメトリ取り込みを検証する。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 カット前にガバナンス承認済み `iroha_config` デルタとロールバック準備を確認する。 |

各リハーサルは、本番に近いトポロジで routed-trace 計測を有効化した状態 ( `nexus.audit.outcome` テレメトリ + Prometheus カウンタ) で実施され、Alertmanager ルールをロードし、証跡を `docs/examples/` にエクスポートしました。

## 方法論

1. **テレメトリ収集。** 全ノードが構造化イベント `nexus.audit.outcome` と付随メトリクス (`nexus_audit_outcome_total*`) を送出。`scripts/telemetry/check_nexus_audit_outcome.py` が JSON ログを追跡し、イベント状態を検証して payload を `docs/examples/nexus_audit_outcomes/` に保存しました。 [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **アラート検証。** `dashboards/alerts/nexus_audit_rules.yml` とそのテストハーネスにより、アラートノイズ閾値と payload テンプレートの整合性を維持。CI は変更ごとに `dashboards/alerts/tests/nexus_audit_rules.test.yml` を実行し、各ウィンドウでも手動で同じルールを適用しました。
3. **ダッシュボード取得。** オペレーターは `dashboards/grafana/soranet_sn16_handshake.json` の routed-trace パネル (handshake 健全性) とテレメトリ概要ダッシュボードをエクスポートし、キュー健全性と監査結果を相関させました。
4. **レビューノート。** ガバナンス書記がレビュアーのイニシャル、判断、緩和チケットを [Nexus transition notes](./nexus-transition-notes) と設定差分トラッカー (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) に記録しました。

## 所見

| Trace ID | 結果 | 証跡 | 注記 |
|----------|---------|----------|-------|
| `TRACE-LANE-ROUTING` | Pass | アラート fire/recover スクリーンショット (内部リンク) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` リプレイ; テレメトリ差分は [Nexus transition notes](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) に記録。 | キュー入場 P95 は 612 ms (目標 <=750 ms) を維持。フォローアップ不要。 |
| `TRACE-TELEMETRY-BRIDGE` | Pass | 監査結果 payload `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` と OTLP リプレイハッシュが `status.md` に記録。 | SDK の redaction salt は Rust ベースラインと一致; diff bot は差分ゼロを報告。 |
| `TRACE-CONFIG-DELTA` | Pass (mitigation closed) | ガバナンストラッカー (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS プロファイル manifest (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + テレメトリパック manifest (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`). | Q2 rerun が承認済み TLS プロファイルをハッシュ化し、取り残しゼロを確認。テレメトリ manifest は slot 範囲 912-936 と workload seed `NEXUS-REH-2026Q2` を記録。 |

全ての trace はウィンドウ内に少なくとも1件の `nexus.audit.outcome` イベントを生成し、Alertmanager の guardrails を満たしました (四半期を通じて `NexusAuditOutcomeFailure` はグリーン)。

## フォローアップ

- routed-trace 付録は TLS ハッシュ `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` で更新済み。緩和 `NEXUS-421` は transition notes でクローズ。
- Android AND4/AND7 レビューのパリティ証拠を強化するため、OTLP 生リプレイと Torii diff アーティファクトをアーカイブに添付し続ける。
- 今後の `TRACE-MULTILANE-CANARY` リハーサルでも同じテレメトリ helper を再利用し、Q2 サインオフが検証済みワークフローの恩恵を受けることを確認する。

## アーティファクト索引

| 資産 | 場所 |
|-------|----------|
| テレメトリバリデータ | `scripts/telemetry/check_nexus_audit_outcome.py` |
| アラートルールとテスト | `dashboards/alerts/nexus_audit_rules.yml`, `dashboards/alerts/tests/nexus_audit_rules.test.yml` |
| サンプル outcome payload | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| 設定差分トラッカー | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
| routed-trace スケジュールとノート | [Nexus transition notes](./nexus-transition-notes) |

このレポート、上記アーティファクト、およびアラート/テレメトリのエクスポートは、四半期の B1 をクローズするためにガバナンス決定ログへ添付する必要があります。
