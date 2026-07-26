---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-telemetry-remediation
タイトル: Plano de remediacao de telemetria do Nexus (B2)
説明: `docs/source/nexus_telemetry_remediation_plan.md` のエスペリョ、遠隔操作のマトリズ デ ラクナスのドキュメント。
---

#ヴィサオ・ジェラル

O 項目のロードマップ **B2 - テレメトリの所有権** 公開計画の公開情報、テレメトリの詳細、Nexus の情報、アラートのガードレール、応答の確認、監査の開始および検証の準備2026 年第 1 四半期。`docs/source/nexus_telemetry_remediation_plan.md` パラケ リリース エンジニアリング、テレメトリ操作、OS 所有者が SDK を確認して、`TRACE-TELEMETRY-BRIDGE` を確認します。

# マトリス・デ・ラクナス

| ID ダ ラクナ |シナルとガードレールの警告 |応答/エスカロナメント |プラゾ (UTC) |証拠と検証 |
|----------|----------------------|----------------------|-----------|----------------------|
| `GAP-TELEM-001` |ヒストグラム `torii_lane_admission_latency_seconds{lane_id,endpoint}` コム アラート **`SoranetLaneAdmissionLatencyDegraded`** ディスパランド クアンド `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` または 5 分 (`dashboards/alerts/soranet_lane_rules.yml`)。 | `@torii-sdk` (シグナル) + `@telemetry-ops` (アラート);オンコール ルーテッド トレース経由でエスカロナメントが Nexus を実行します。 | 2026-02-23 |テスト・デ・アラート `dashboards/alerts/tests/soranet_lane_rules.test.yml` レジストリ・ド・エンサイオ `TRACE-LANE-ROUTING` ほとんどのアラート、ディスパラド/リカバリー、スクレープ Torii `/metrics` arquivado em [Nexus 移行]注](./nexus-transition-notes)。 |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` com ガードレール `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` ブロックアンド デプロイ (`docs/source/telemetry.md`)。 | `@nexus-core` (instrumentacao) -> `@telemetry-ops` (alerta);公式の政府と公式のページ作成と編集の増分。 | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`; アルマゼナダ政府のドライランについて説明します。 Prometheus は、プロバンド クエリ `StateTelemetry::record_nexus_config_diff` の差分を記録するためのキャプチャを含むリリースのチェックリストです。 |
| `GAP-TELEM-003` |イベント `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) com アラート **`NexusAuditOutcomeFailure`** は 30 分を超える結果が持続します (`dashboards/alerts/nexus_audit_rules.yml`)。 | `@telemetry-ops` (パイプライン) `@sec-observability` からのエスカロナメント。 | 2026-02-27 | CI `scripts/telemetry/check_nexus_audit_outcome.py` arquiva ペイロード NDJSON と falha quando uma janela TRACE nao tem eventsto de sucesso のゲート。ルートトレース関連のアネクサダをキャプチャします。 |
| `GAP-TELEM-004` |ゲージ `nexus_lane_configured_total` com ガードレール `nexus_lane_configured_total != EXPECTED_LANE_COUNT` は、SRE のオンコール チェックリストに相当します。 | `@telemetry-ops` (ゲージ/エクスポート) `@nexus-core` に関するレポートは一貫性がありません。 | 2026-02-28 |スケジューラ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` を送信してテレメトリアをテストしてください。オペラドールの試験差分 Prometheus + ログ `StateTelemetry::set_nexus_catalogs` を追跡し、TRACE を実行します。 |

# Fluxo 運用

1. **問題の解決策。** 所有者は Nexus の進行状況を報告します。ブロッカーは、`status.md` の検査登録に関する警告です。
2. **警告の警告** 警告を表示するには、`dashboards/alerts/tests/*.test.yml` パラメータ CI を実行してください。`promtool test rules` は、ガードレール ミューダーを実行します。
3. **聴覚の証拠。** 継続的な監視 `TRACE-LANE-ROUTING` と `TRACE-TELEMETRY-BRIDGE`、オンコール キャプチャ結果の問い合わせ Prometheus、過去のアラートとスクリプト関連の発言 (`scripts/telemetry/check_nexus_audit_outcome.py`、 `scripts/telemetry/check_redaction_status.py` para sinais correlacionados) は、armazena com os artefatos Routed-trace として使用されます。
4. **エスカロナメント** 安全なガードレールは、緊急事態に見合ったものではありません。また、インシデント Nexus は、聴衆としての重要なスナップショットのスナップショットを含む、緊急事態に対応するための設備を備えています。

Com esta matriz publicada - e Referenciada em `roadmap.md` e `status.md` - o item de roadmap **B2** agora atende aos criteria de aceitacao "responsabilidade, prazo, alerta, verificacao".