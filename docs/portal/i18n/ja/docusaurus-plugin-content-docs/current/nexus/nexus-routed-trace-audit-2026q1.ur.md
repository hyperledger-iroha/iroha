---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
タイトル: 2026 年第 1 四半期のルートトレース آڈٹ رپورٹ (B1)
説明: `docs/source/nexus_routed_trace_audit_report_2026q1.md` کا آئینہ، جو سہ ماہی ٹیلیمیٹری ریہرسل کے نتائج کا احاطہ کرتا ہے۔
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note ٩ینونیکل ماخذ
یہ صفحہ `docs/source/nexus_routed_trace_audit_report_2026q1.md` کی عکاسی کرتا ہے۔ باقی تراجم آنے تک دونوں نقول ہم آہنگ رکھیں۔
:::

# 2026 Q1 Routed-Trace (B1)

**B1 - ルーテッド トレース監査およびテレメトリ ベースライン** Nexus ルーテッド トレース レベルのテスト。 ❁❁❁❁ یہ رپورٹ Q1 2026 (جنوری-مارچ) کی آڈٹ ونڈو دستاویز کرتی ہے تاکہ گورننس کونسل Q2 لانچ پہرسل سے پہلے ٹیلیمیٹری پوزیشن کی منظوری دے سکے۔

## دائرہ کار اور ٹائم لائن

|トレースID | (UTC) |すごい |
|----------|--------------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |マルチレーン レーン入場ヒストグラム キュー ゴシップ アラート フロー 評価|
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | AND4/AND7 マイルストーン OTLP リプレイ 差分ボット パリティ SDK テレメトリの取り込み|
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 カット ガバナンス承認 `iroha_config` デルタ ロールバック準備完了|

本番環境に似たトポロジ ルーテッド トレース計測機能 (`nexus.audit.outcome` テレメトリ + Prometheus カウンタ) アラート マネージャ ルール証拠 `docs/examples/` میں ایکسپورٹ ہوا۔

## और देखें

1. **ٹیلیمیٹری کلیکشن۔** 構造化 `nexus.audit.outcome` ایونٹ اور متعلقہ メトリクス (`nexus_audit_outcome_total*`) は کیں۔ を発行しますヘルパー `scripts/telemetry/check_nexus_audit_outcome.py` JSON ログ テール کیا، ایونٹ اسٹیٹس ویلیڈیٹ کیا، اور ペイロード کو `docs/examples/nexus_audit_outcomes/` میں آرکائیوありがとう[scripts/telemetry/check_nexus_audit_outcome.py:1]
2. ** `dashboards/alerts/nexus_audit_rules.yml` テスト ハーネス テスト ハーネス アラート ノイズしきい値 ペイロード テンプレート メッセージうわーCI ہر تبدیلی پر `dashboards/alerts/tests/nexus_audit_rules.test.yml` چلاتا ہے؛ یہی رولز ہر ونڈو میں دستی طور پر بھی چلائے گئے۔
3. **`dashboards/grafana/soranet_sn16_handshake.json` (ハンドシェイク ヘルス) ルーテッド トレース パネル テレメトリ概要ダッシュボード テレメトリ概要ダッシュボードキューの健全性と監査結果の相関関係
4. **レビュー担当者のイニシャル、緩和チケット、[Nexus 移行ノート](./nexus-transition-notes) 構成デルタトラッカー (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) すごい

## ナオト

|トレースID |ナタリー | और देखेंにゅう |
|----------|-----------|----------|----------|
| `TRACE-LANE-ROUTING` |パス |警戒射撃/回復 اسکرین شاٹس (اندرونی لنک) + `dashboards/alerts/tests/soranet_lane_rules.test.yml` 再生テレメトリ差分 [Nexus 移行メモ](./nexus-transition-notes#quarterly-routed-trace-audit-schedule) میں ریکارڈ۔ |キュー受付 P95 612 ms پر رہا (ہدف <=750 ms)۔ فالو اپ درکار نہیں۔ |
| `TRACE-TELEMETRY-BRIDGE` |パス |アーカイブされた結果ペイロード `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` اور OTLP リプレイ ハッシュ `status.md` میں ریکارڈ۔ | SDK 編集ソルト Rust ベースラインとの一致差分ボット ゼロデルタ پورٹ کیے۔ |
| `TRACE-CONFIG-DELTA` |パス (軽減策は終了) |ガバナンス トラッカー エントリ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + TLS プロファイル マニフェスト (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + テレメトリ パック マニフェスト (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)。 |第 2 四半期の再実行、TLS プロファイル ハッシュ、ゼロの落伍者、およびその結果テレメトリ マニフェスト スロット 912 ～ 936 ワークロード シード `NEXUS-REH-2026Q2` ステータス|

追跡は、アラートマネージャーのガードレールを追跡します。 پورے ہوئے (`NexusAuditOutcomeFailure` پورے کوارٹر میں گرین رہا)۔

## フォローアップ

- ルーテッド トレースの付録 TLS ハッシュ `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` の詳細緩和 `NEXUS-421` 移行ノート میں بند کیا گیا۔
- Android AND4/AND7 のレビュー、パリティの証拠、生の OTLP リプレイ、Torii の差分アーティファクトの確認੭ुुल्लु
- `TRACE-MULTILANE-CANARY` リハーサル、テレメトリ ヘルパー、Q2 サインオフ検証ワークフロー、Q2 承認فائدہ اٹھائے۔

## アーティファクト

|資産 |大事 |
|------|----------|
|テレメトリバリデータ | `scripts/telemetry/check_nexus_audit_outcome.py` |
|アラート ルールとテスト | `dashboards/alerts/nexus_audit_rules.yml`、`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|サンプル結果ペイロード | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
|構成デルタトラッカー | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|ルートトレースのスケジュールと注意事項 | [Nexus 移行メモ](./nexus-transition-notes) |

アーティファクト アラート/テレメトリのエクスポート ガバナンス意思決定ログ 管理上の決定 管理上の注意事項B1 の بند ہو جائے۔