---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
タイトル: ルーテッドトレースの監査に関する情報 2026 年第 1 四半期 (B1)
説明: `docs/source/nexus_routed_trace_audit_report_2026q1.md` の説明、テレメトリアのリビジョン トリメストラルの結果の確認。
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::ノート フエンテ カノニカ
エスタページナリフレジャ`docs/source/nexus_routed_trace_audit_report_2026q1.md`。 Manten ambas copias alineadas hasta que lleguen las traducciones restantes。
:::

# Routed-Trace 2026 Q1 に関する情報 (B1)

ロードマップ **B1 - ルーテッド トレース監査およびテレメトリ ベースライン** の項目には、Nexus のルーテッド トレース プログラムのリビジョン 3 が必要です。 2026 年第 1 四半期の公聴会に関する情報 (エネロマルゾ) は、第 2 四半期におけるテレメトリアの状況に関する情報を提供します。

## アルカンセ・イ・リネア・デ・ティエンポ

|トレースID |ベンタナ (UTC) |オブジェクト |
|----------|--------------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |レーンの入院状況、コーラのゴシップ、およびマルチレーンの危険性に関する警告のヒストグラムを検証します。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 |有効なリプレイ OTLP、SDK のテレメトリの取り込みおよび AND4/AND7 の差分ボットの検証。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | RC1 のロールバック前の準備として、`iroha_config` に関するデルタを確認してください。 |

ルートトレース ハビリタダ (テレメトリア `nexus.audit.outcome` + コンタドール Prometheus)、`docs/examples/` でのアラート マネージャー カルガダと証拠のエクスポートに関する規則を確認してください。

## メトドロギア

1. **テレメトリの収集。** 構造化されたイベント `nexus.audit.outcome` とコンパナンテス メトリクス (`nexus_audit_outcome_total*`)。ヘルパー `scripts/telemetry/check_nexus_audit_outcome.py` ログ JSON の末尾、`docs/examples/nexus_audit_outcomes/` でのイベントの有効なアーカイブ ペイロード。 [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **アラートの検証** `dashboards/alerts/nexus_audit_rules.yml` は、プルエバスのハーネスを安全に管理し、ペイロードのテンプレートと一貫性を保ちます。 CI 出力 `dashboards/alerts/tests/nexus_audit_rules.test.yml` en cada cambio;ラスミスマスレグラスSEエジェルシタロンマニュアルメンテデュランテカダベンタナ。
3. **ダッシュボードのキャプチャ。** `dashboards/grafana/soranet_sn16_handshake.json` (ハンドシェイクの食事) のルーテッド トレース パネルをエクスポートし、テレメトリの一般的な相関関係を確認するためにダッシュボードを表示します。
4. **改訂に関する注意事項** 改訂に関する政府事務局、[Nexus 移行ノート](./nexus-transition-notes) およびデルタ設定の追跡に関する決定事項 (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)。

## ハラズゴス

|トレースID |結果 |証拠 |メモ |
|----------|-----------|----------|----------|
| `TRACE-LANE-ROUTING` |パス | `dashboards/alerts/tests/soranet_lane_rules.test.yml` の発射/回復 (エンレースインターノ) + リプレイのキャプチャ。テレメトリ登録の差分 [Nexus 移行ノート](./nexus-transition-notes#quarterly-routed-trace-audit-schedule)。 | P95 は 612 ミリ秒での管理を許可します (目的 <= 750 ミリ秒)。セギミエントを要求する必要はありません。 |
| `TRACE-TELEMETRY-BRIDGE` |パス |ペイロード アーカイブ `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` は、`status.md` のハッシュ デ リプレイ OTLP レジストラードです。 | Rust と SDK の一致を修正する必要があります。 el diff bot レポート cero デルタ。 |
| `TRACE-CONFIG-DELTA` |パス (軽減策は終了) |ゴベルナンザ追跡エントリ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + パフォーマンス TLS マニフェスト (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + テレメトリア パケット マニフェスト (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)。 | La reejecucion Q2 のハシオ エル パーフィル TLS のアプロバドと確認の確認。テレメトリ レジストラのマニフェスト、スロット 912 ～ 936、ワークロード シード `NEXUS-REH-2026Q2`。 |

イベント `nexus.audit.outcome` デントロ デ サス ベンタナス、アラート マネージャーでのガードレールの満足度 (`NexusAuditOutcomeFailure` 安全な期間の管理)。

## フォローアップ

- 実際の付録のルーテッド トレース コントロール ハッシュ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`; `NEXUS-421` は移行に関する注意事項を保証します。
- Torii の Android AND4/AND7 の改訂版の証拠となるアーカイブの OTLP の処理と差分を継続的に再生します。
- 直近のリハーサル `TRACE-MULTILANE-CANARY` を確認し、テレメトリアのミスモ ヘルパーと第 2 四半期のベネフィシー デル フルホの有効性を確認します。

## アーティファクトのインデックス|アクティボ |ユビカシオン |
|------|----------|
|テレメトリアの検証 | `scripts/telemetry/check_nexus_audit_outcome.py` |
|警告テストの規則 | `dashboards/alerts/nexus_audit_rules.yml`、`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|実行結果のペイロード | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
|デルタ構成トラッカー | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|ルートトレースのスケジュールを設定する | [Nexus 移行メモ](./nexus-transition-notes) |

エステ情報は、B1 デル トリメストレでの決定に関する登録および決定に関する事前通知と、アラート/テレメトリアの輸出に関する情報です。