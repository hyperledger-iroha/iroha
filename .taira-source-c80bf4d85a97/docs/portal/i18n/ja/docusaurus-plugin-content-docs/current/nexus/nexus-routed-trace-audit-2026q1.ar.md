---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
タイトル: ルートトレース 2026 年第 1 四半期 (B1)
説明: نسخة مطابقة لـ `docs/source/nexus_routed_trace_audit_report_2026q1.md` تغطي نتائج تدريبات التليمتري الفصلية.
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::メモ
テストは `docs/source/nexus_routed_trace_audit_report_2026q1.md` です。あなたのことを忘れないでください。
:::

# ルートトレース 2026 年第 1 四半期 (B1)

**B1 - ルーテッド トレース監査およびテレメトリ ベースライン** ルーテッド トレース Nexus。 يوثق هذا التقرير نافذة تدقيق Q1 2026 (يناير-مارس) لكي يستطيع مجلس الحوكمة اعتماد وضع Q2.

## और देखें

|トレースID |世界時間 (UTC) |ああ |
|----------|--------------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |マルチレーンの情報を収集します。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | OTLP と diff ボットを統合し、SDK を AND4/AND7 に統合します。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | `iroha_config` は、RC1 を実行します。 |

Routed-trace (`nexus.audit.outcome` + عدادات ) のルーティング トレースPrometheus) アラートマネージャー محملة وتصدير الادلة الى `docs/examples/`。

## ああ

1. **جمع التليمتري.** الصدرت كل العقد الحدث المهيكل `nexus.audit.outcome` والمقاييس المصاحبة (`nexus_audit_outcome_total*`)。 قام المساعد `scripts/telemetry/check_nexus_audit_outcome.py` بمتابعة سجل JSON والتحقق من حالة الحدث وارشفة الحمولة تحت `docs/examples/nexus_audit_outcomes/`。 [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. ** التحقق من التنبيهات.** ضمنت `dashboards/alerts/nexus_audit_rules.yml` واداة الاختبار الخاصة بها بقاء عتبات ضوضاءありがとうございます。 CI الملف `dashboards/alerts/tests/nexus_audit_rules.test.yml` عند كل تغيير؛最高のパフォーマンスを見せてください。
3. ** 接続されたルートトレース `dashboards/grafana/soranet_sn16_handshake.json` (接続されたトレース) は、** 接続されます。重要な問題は、次のとおりです。
4. **重要な要素。** 重要な要素。** 重要な要素。 [Nexus 移行ノート](./nexus-transition-notes) وفي متتبع فروقات الاعدادات (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)。

## ああ

|トレースID |認証済み |意味 | और देखें
|----------|-----------|----------|----------|
| `TRACE-LANE-ROUTING` |パス | لقطات تنبيه fire/recover (رابط داخلي) + اعادة تشغيل `dashboards/alerts/tests/soranet_lane_rules.test.yml`; [Nexus 移行ノート](./nexus-transition-notes#quarterly-routed-trace-audit-schedule)。 | P95 は 612 ミリ秒 (750 ミリ秒以下)。ありがとうございます。 |
| `TRACE-TELEMETRY-BRIDGE` |パス | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` は OTLP を再生し、`status.md` を再生します。 |塩類と SDK の開発 Rust の開発差分ボットのアカウントです。 |
| `TRACE-CONFIG-DELTA` |パス (軽減策は終了) |メッセージ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + マニフェスト TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + マニフェスト メッセージ (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)。 | Q2 テスト TLS テスト واكد عدم وجود متاخرين؛マニフェストは 912-936 、`NEXUS-REH-2026Q2` です。 |

警告トレース 警告 `nexus.audit.outcome` 警告 بما يلبي حواجز Alertmanager (`NexusAuditOutcomeFailure` 日本語版)。

## ああ

- ルーテッド トレース TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; `NEXUS-421` 移行ノート。
- OTLP との差分 Torii を確認するAndroid AND4/AND7 に対応。
- 評価 `TRACE-MULTILANE-CANARY` 評価 評価 評価 評価 Q2特別な意味を持ちます。

## アーティファクト

|ああ |ああ |
|------|----------|
| और देखें `scripts/telemetry/check_nexus_audit_outcome.py` |
| قواعد وتنبيهات الاختبار | `dashboards/alerts/nexus_audit_rules.yml`、`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|結果 | 結果`docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
| عرض المزيد `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|ルートトレースを実行する | [Nexus 移行メモ](./nexus-transition-notes) |

يجب ارفاق هذا التقرير والقطع اعلاه وعمليات تصدير التنبيهات/التليمتري بسجل قرار الحوكمة B1 にあります。