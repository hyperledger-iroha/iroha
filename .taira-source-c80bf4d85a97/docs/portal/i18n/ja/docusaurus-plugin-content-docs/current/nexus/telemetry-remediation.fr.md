---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-telemetry-remediation
タイトル: テレメトリーの修復計画 Nexus (B2)
説明: `docs/source/nexus_telemetry_remediation_plan.md` のミロワール、テレメトリと磁束操作のマトリクス デカルトの文書。
---

# ヴューダンサンブル

ロードマップの要素**B2 - テレメトリーの所有権** 計画と公開に依存するチャック テレメトリーの保持者 Nexus は、信号、ガードレール、アラート、所有権、日付制限および検証前に公開される 2026 年第 1 四半期の成果物を確認します。ページ参照 `docs/source/nexus_telemetry_remediation_plan.md` は、リリース エンジニアリング、テレメトリ オペレーション、所有者 SDK の優先確認者、ルーテッド トレースの繰り返しなどに関する `TRACE-TELEMETRY-BRIDGE` です。

# マトリスデカール

| ID デカルト |信号とガードレールの警告 |プロプリエテール / エスカレード |エシャンス (UTC) |プルーヴと検証 |
|----------|----------------------|----------------------|-----------|----------------------|
| `GAP-TELEM-001` |ヒストグラム `torii_lane_admission_latency_seconds{lane_id,endpoint}` の平均アラート **`SoranetLaneAdmissionLatencyDegraded`** デクレンチ クアンド `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` ペンダント 5 分 (`dashboards/alerts/soranet_lane_rules.yml`)。 | `@torii-sdk` (シグナル) + `@telemetry-ops` (アラート); l'on-call Routed-trace Nexus 経由でエスカレード。 | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` のテストと、繰り返しのキャプチャのテスト `TRACE-LANE-ROUTING` モントラントの警告/削除とスクレイピング Torii `/metrics` アーカイブと [Nexus 移行]注](./nexus-transition-notes)。 |
| `GAP-TELEM-002` | Compteur `nexus_config_diff_total{knob,profile}` アベック ガードレール `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` ブロカント レ デプロイメント (`docs/source/telemetry.md`)。 | `@nexus-core` (計測) -> `@telemetry-ops` (アラート); l'officier de garde gouvernance est page lorsque le compteur augmente de maniere inattendue。 | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` の備蓄品を備蓄する予備作戦に出撃します。要求された Prometheus のキャプチャを含むリリースのチェックリストと、証明されたログ `StateTelemetry::record_nexus_config_diff` のファイルの差分を追加します。 |
| `GAP-TELEM-003` |偶数 `TelemetryEvent::AuditOutcome` (メトリック `nexus.audit.outcome`) 警告 **`NexusAuditOutcomeFailure`** 検査結果の検査結果が 30 分を超えて持続します (`dashboards/alerts/nexus_audit_rules.yml`)。 | `@telemetry-ops` (パイプライン) avec エスカレードと `@sec-observability`。 | 2026-02-27 | La Gate CI `scripts/telemetry/check_nexus_audit_outcome.py` ペイロードのアーカイブ NDJSON とエコーアウト lorsqu'une fenetre TRACE ne contient pas d'evenement de succes;ルーテッド トレースの接続と信頼関係をキャプチャします。 |
| `GAP-TELEM-004` |ゲージ `nexus_lane_configured_total` avec ガードレール `nexus_lane_configured_total != EXPECTED_LANE_COUNT` 食事療法チェックリスト SRE オンコール。 | `@telemetry-ops` (ゲージ/エクスポート) エスカレードと `@nexus-core` のカタログの矛盾した信号。 | 2026-02-28 |スケジューラ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` のテレメトリー テストは、放射を証明します。ファイル操作者は、差分 Prometheus + ログ `StateTelemetry::set_nexus_catalogs` の反復 TRACE パッケージを追加します。 |

# フラックス操作ネル

1. **トリアージ hebdomadaire.** オーナーの対応状況 Nexus; `status.md` からの委託品の検査結果を確認してください。
2. **アラートのテスト** 主要なエントリ `dashboards/alerts/tests/*.test.yml` で CI を実行し、`promtool test rules` ガードレールの進化を確認します。
3. **Preuves d'audit.** ペンダントの繰り返し `TRACE-LANE-ROUTING` および `TRACE-TELEMETRY-BRIDGE`、オンコール キャプチャの要求結果の結果 Prometheus、アラート履歴および関連スクリプトの並べ替え (`scripts/telemetry/check_nexus_audit_outcome.py`、 `scripts/telemetry/check_redaction_status.py` は、シグノー コレレスを注ぎます) ルートトレースのアーティファクトをストックします。
4. **エスカレード** ガードレールの安全性を確認し、フェネトレの繰り返し、事故発生時のチケットの準備 Nexus の参照計画、対策のスナップショットおよび事前の監査を含めた緩和策を講じます。

公開情報マトリクスの公開 - 参照情報 `roadmap.md` および `status.md` - ロードマップ **B2** の項目は、「責任、誠実、警告、検証」の受け入れ基準を満たしています。