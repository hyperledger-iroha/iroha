---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
タイトル: Relatorio de audiotoria Routed-trace 2026 Q1 (B1)
説明: `docs/source/nexus_routed_trace_audit_report_2026q1.md` のエスペリョ、テレメトリアのトリメストレイス修正結果のコブリンド。
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note フォンテ カノニカ
エスタページナリフレテ`docs/source/nexus_routed_trace_audit_report_2026q1.md`。デュアス・コピアス・アリンハダスとしてマンテンハは、トラドゥコエス・レストスタンテス・チェゲムとしてケを食べました。
:::

# Relatorio de Auditoria Routed-Trace 2026 Q1 (B1)

O 項目はロードマップ **B1 - ルーテッド トレース監査とテレメトリ ベースライン** を実行し、ルーテッド トレース プログラムを修正して Nexus を実行します。 2026 年第 1 四半期の聴衆との関係に関する文書 (ジャネイロ-マルコ) は、第 2 四半期にテレメトリアの姿勢を確認するための政策を決定します。

## エスコポとクロノグラマ

|トレースID |ジャネラ (協定世界時) |オブジェクト |
|----------|--------------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |レーンの承認ヒストグラム、ニュースのニュース、マルチレーンのアクセス状況のアラートの流動性を検証します。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 |有効なリプレイ OTLP、SDK のテレメトリ AND4/AND7 の差分ボットの取り込みを確認します。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | `iroha_config` のデルタを確認し、RC1 のロールバックを事前に確認してください。 |

トポロジのセメルハンテを作成し、ルーテッド トレース ハビリタダ (テレメトリア `nexus.audit.outcome` + コンタドレス Prometheus) を管理し、`docs/examples/` に関するアラート マネージャー カレガダと証拠のエクスポートを記録します。

## メトドロギア

1. **テレメトリアのコレタ。** メトリカス アソシエーダ (`nexus_audit_outcome_total*`) としての重要なイベントの説明 `nexus.audit.outcome`。 O ヘルパー `scripts/telemetry/check_nexus_audit_outcome.py` フェズ テール ログ JSON、有効なステータス、イベント、ペイロード、`docs/examples/nexus_audit_outcomes/`。 [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **アラートの検証** `dashboards/alerts/nexus_audit_rules.yml` は、ペイロードの永続的な一貫性を維持するために、ハーネスのテストを保証します。 O CI executa `dashboards/alerts/tests/nexus_audit_rules.test.yml` a cada mudanca;定期的にマニュアルを実行するための手順として。
3. **ダッシュボードのキャプチャ。** `dashboards/grafana/soranet_sn16_handshake.json` (ハンドシェイク機能) でルーテッド トレースをエクスポートし、テレメトリの操作を行って OS ダッシュボードを操作し、音声データを収集します。
4. **改訂の注意事項** 政府の改訂に関する事務局、[Nexus 移行メモ](./nexus-transition-notes)、デルタ構成の追跡機能はありません (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)。

## アカドス

|トレースID |結果 |証拠 |メモ |
|----------|-----------|----------|----------|
| `TRACE-LANE-ROUTING` |パス | `dashboards/alerts/tests/soranet_lane_rules.test.yml` の発射/回復 (リンク内部) + リプレイのキャプチャ。テレメトリ登録の差分 [Nexus 移行ノート](./nexus-transition-notes#quarterly-routed-trace-audit-schedule)。 | P95 は 612 ミリ秒以内に永続的に保存されます (750 ミリ秒以下)。 Semのフォローアップ。 |
| `TRACE-TELEMETRY-BRIDGE` |パス |ペイロード アーキバード `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` は、`status.md` の OTLP レジストラード ハッシュ デ リプレイを作成します。 | SDK データベースは Rust を使用してソルトを編集します。 o diff bot はゼロデルタをレポートします。 |
| `TRACE-CONFIG-DELTA` |パス (軽減策は終了) | Entrada には、統治トラッカー (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + パフォーマンス TLS マニフェスト (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + テレメトリア マニフェスト (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`) がありません。 |第 2 四半期の再実行では、TLS の承認を実行し、ストラグラーがゼロであることを確認します。 o テレメトリ レジストラのマニフェスト o スロット 912 ～ 936 の間隔 o ワークロード シード `NEXUS-REH-2026Q2`。 |

Todos os トレースは、イベント `nexus.audit.outcome` dentro de suas janelas、Satisfazendo os Guardrails do Alertmanager (`NexusAuditOutcomeFailure` permaneceu verde no triestre) を追跡します。

## フォローアップ

- ハッシュ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` の付録 Routed-trace com o ハッシュ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb`;移行メモ `NEXUS-421` を確認してください。
- 継続的なアネクサンド リプレイ OTLP ブルートと差分データ Torii は、Android AND4/AND7 での改訂の証拠と検証を行います。
- 近くのリハーサルとして `TRACE-MULTILANE-CANARY` を再利用し、テレメトリの管理ヘルパーとワークフロー検証の第 2 四半期の承認の承認を確認します。

## 技術資料のインデックス

|アティボ |ローカル |
|------|----------|
|テレメトリアの検証 | `scripts/telemetry/check_nexus_audit_outcome.py` |
|精巣警告通知 | `dashboards/alerts/nexus_audit_rules.yml`、`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|ペイロードと結果の例 | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
|デルタ設定トラッカー | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|クロノグラマはルートトレースではありません | [Nexus 移行メモ](./nexus-transition-notes) |エステ関係、OS の最新情報、アラート/テレメトリア開発サーバーのエクスポート、管理パラグラフの決定、B1 のトリメストレ。