---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-routed-trace-audit-2026q1
タイトル: Rapport d'audit ルートトレース 2026 Q1 (B1)
説明: `docs/source/nexus_routed_trace_audit_report_2026q1.md` のミロワール、テレメトリの繰り返しの結果を確認します。
---
<!--
  SPDX-License-Identifier: Apache-2.0
-->

:::note ソースカノニク
Cette ページは `docs/source/nexus_routed_trace_audit_report_2026q1.md` を反映します。 Gardez les deux は、到着するまでに alignees jusqu'a ce que les traductions をコピーします。
:::

# Rapport d'audit Routed-Trace 2026 Q1 (B1)

ロードマップ **B1 - ルートトレース監査とテレメトリ ベースライン** プログラムのルートトレース Nexus のレビュー項目。 Ce rapport documente la fenetre d'audit Q1 2026 (janvier-mars) afin que le conseil de gouvernance puisse valider la posture de telemetry avant les repetitions de lancement Q2.

## ポルティとカランドリエ

|トレースID |フェネトレ (UTC) |目的 |
|----------|--------------|----------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 |レーンの入場のヒストグラム、ファイルのゴシップ、およびマルチレーンのアクティブ化の前の流動性を検証します。 |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | OTLP のリプレイ、差分ボットのパリティ、およびテレメトリ SDK の取り込み AND4/AND7 を検証します。 |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 |デルタ `iroha_config` は、RC1 の事前準備とロールバックの準備を承認します。 |

定期的なトポロジーの生産管理の繰り返し (テレメトリー `nexus.audit.outcome` + コンピューター Prometheus)、ルール アラート マネージャーの料金と輸出規制 `docs/examples/`。

## 方法論

1. **テレメトリの収集** Tous les noeuds ont emis l'evenement Structure `nexus.audit.outcome` et les metriques associees (`nexus_audit_outcome_total*`)。ヘルパー `scripts/telemetry/check_nexus_audit_outcome.py` は、JSON 形式のログ、`docs/examples/nexus_audit_outcomes/` のペイロードの有効なステータスおよびアーカイブ ファイルです。 [scripts/telemetry/check_nexus_audit_outcome.py:1]
2. **アラートの検証** `dashboards/alerts/nexus_audit_rules.yml` およびハーネス テストは、ブルーイットのセキュリティおよびペイロードのテンプレートの一貫性を保証するものではありません。 CI は、`dashboards/alerts/tests/nexus_audit_rules.test.yml` チャック変更を実行します。 les memes regles ont ete exercees manuellement ペンダント チャク フェネトル。
3. **ダッシュボードをキャプチャします。** `dashboards/grafana/soranet_sn16_handshake.json` (サンテ ハンドシェイク) をエクスポートする操作と、グローバルなテレメトリー ダッシュボードと、ファイルの検証結果を表示します。
4. **再選の注意事項** 初期の委託品の政府事務局、緩和策の決定およびチケットの管理 [Nexus 移行メモ](./nexus-transition-notes) およびデルタ構成のトラッカー (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`)。

## コンステート

|トレースID |結果 |プルーヴ |メモ |
|----------|-----------|----------|----------|
| `TRACE-LANE-ROUTING` |パス |アラート火災/回復 (先取特権) + リプレイ `dashboards/alerts/tests/soranet_lane_rules.test.yml` をキャプチャします。登録されているテレメトリーの差分 [Nexus 移行ノート](./nexus-transition-notes#quarterly-routed-trace-audit-schedule)。 |ファイル保存の許可は 612 ミリ秒 (cible <=750 ミリ秒) です。 Aucun suvi が必要です。 |
| `TRACE-TELEMETRY-BRIDGE` |パス |ペイロード アーカイブ `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` とハッシュ デ リプレイ OTLP は `status.md` に登録されます。 | Rust ベースの SDK 対応版の編集の説明。差分ボットはゼロデルタ信号を送信します。 |
| `TRACE-CONFIG-DELTA` |パス (軽減策は終了) |管理トラッカーのエントリ (`docs/source/project_tracker/nexus_config_deltas/2026Q1.md`) + マニフェスト プロファイル TLS (`artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`) + マニフェスト デュ パック テレメトリ (`artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`)。 | Q2 を再実行し、ハッシュ プロファイル TLS を承認し、遅延ゼロを確認します。マニフェスト テレメトリはスロット 912 ～ 936 のプラグインを登録し、ワークロード シード `NEXUS-REH-2026Q2` を登録します。 |

`nexus.audit.outcome` dans leurs fenetres、satisfaisant les Guardrails Alertmanager (`NexusAuditOutcomeFailure` estreste vert sur le trimestre)。

## スイビス

- 付録のルーティングトレースは、1 週間の平均ハッシュ TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` を追跡します。緩和策 `NEXUS-421` 移行ノートを参照してください。
- Android AND4/AND7 では、引き続き、OTLP ブルートと差分 Torii のアーカイブを再生し、優先順位の高いパリテを再生します。
- プロチェーンのリハーサル `TRACE-MULTILANE-CANARY` を再利用して、テレメトリーのミーム ヘルパーを再利用して、検証 Q2 のワークフローの有効性を確認します。

## アーティファクトのインデックス|資産 |定置 |
|------|----------|
|テレメトリの検証 | `scripts/telemetry/check_nexus_audit_outcome.py` |
|規則とテストに関する規則 | `dashboards/alerts/nexus_audit_rules.yml`、`dashboards/alerts/tests/nexus_audit_rules.test.yml` |
|ペイロード結果の例 | `docs/examples/nexus_audit_outcomes/TRACE-TELEMETRY-BRIDGE-20260224T101732Z-pass.json` |
|デルタ設定のトラッカー | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` |
|計画とメモ Routed-Trace | [Nexus 移行メモ](./nexus-transition-notes) |

Ce rapport、les artefacts ci-dessus et les Exports d'alertes/telemetrie doivent etre添付のジャーナル・デ・ディシジョン・デ・ガバナンス・プール・クロチュラーB1 du trimestre。