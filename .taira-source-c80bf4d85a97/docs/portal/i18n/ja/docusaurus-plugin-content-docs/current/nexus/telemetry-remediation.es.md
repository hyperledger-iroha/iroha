---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-telemetry-remediation
タイトル: Nexus のテレメトリア修復計画 (B2)
説明: `docs/source/nexus_telemetry_remediation_plan.md` の説明、遠隔操作のマトリックスとフルホ オペラのドキュメント。
---

# 履歴書全般

ロードマップ **B2 - テレメトリアの所有権** には、Nexus に関する計画の公開、警告のガードレール、責任のない制限、検証前の検証が必要です。 2026 年第 1 四半期の監査。`docs/source/nexus_telemetry_remediation_plan.md` パラケ リリース エンジニアリング、テレメトリ運用の責任者が SDK を確認し、ルートトレース `TRACE-TELEMETRY-BRIDGE` を確認しました。

# マトリス・デ・ブレカス

| ID デ ブリーチ |セナルとガードレールの警告 |責任者 / エスカラミエント |フェチャ (協定世界時) |証拠と検証 |
|----------|----------------------|----------------------|-----------|----------------------|
| `GAP-TELEM-001` |ヒストグラム `torii_lane_admission_latency_seconds{lane_id,endpoint}` は **`SoranetLaneAdmissionLatencyDegraded`** を 5 分以内に表示します (`dashboards/alerts/soranet_lane_rules.yml`)。 | `@torii-sdk` (センナル) + `@telemetry-ops` (アラート); Nexus のルーテッド トレースのオンコール経由のエスカラー。 | 2026-02-23 | `dashboards/alerts/tests/soranet_lane_rules.test.yml` マス ラ キャプチャ デル エンサヨ `TRACE-LANE-ROUTING` ほとんどのアラート、ディスパラダ/回復、および Torii `/metrics` アーカイブ [Nexus 移行] のプルーバス注](./nexus-transition-notes)。 |
| `GAP-TELEM-002` | Contador `nexus_config_diff_total{knob,profile}` コン ガードレール `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` que bloquea despliegues (`docs/source/telemetry.md`)。 | `@nexus-core` (計測器) -> `@telemetry-ops` (警告);安全な公式ページを作成し、形式的な情報を追加します。 | 2026-02-26 | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` を実行するための予行演習を実行します。リリースのチェックリストには、Prometheus ログの抽出、プルエバ クエリ、`StateTelemetry::record_nexus_config_diff` の差分出力が含まれています。 |
| `GAP-TELEM-003` |イベント `TelemetryEvent::AuditOutcome` (metrica `nexus.audit.outcome`) **`NexusAuditOutcomeFailure`** の警告が 30 分を超えて継続します (`dashboards/alerts/nexus_audit_rules.yml`)。 | `@telemetry-ops` (パイプライン) は `@sec-observability` にエスカラミエントします。 | 2026-02-27 | CI `scripts/telemetry/check_nexus_audit_outcome.py` アーカイブ ペイロード NDJSON が、イベントで終了する TRACE ケアを実行します。ルートトレースレポートの付属機能としてアラートをキャプチャします。 |
| `GAP-TELEM-004` |ゲージ `nexus_lane_configured_total` とガードレール `nexus_lane_configured_total != EXPECTED_LANE_COUNT` は、SRE のオンコール チェックリストに含まれています。 | `@telemetry-ops` (ゲージ/エクスポート) `@nexus-core` cuando los nodos reportan tamanos de categoryo inconsistentes に関する問題。 | 2026-02-28 |スケジューラー `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` のテレメトリア プルエバ。 los operadores adjuntan Prometheus diff + extracto de log de `StateTelemetry::set_nexus_catalogs` al paquete del ensayo TRACE。 |

#フルーホ・オペラティーボ

1. **トリアージセマンナル。** ロスオーナーは、Nexus の準備状況の進捗状況を報告します。ブロッカーおよび警告登録 `status.md` のプルエバスのアーティファクト。
2. **警告メッセージ** 警告メッセージが表示され、`dashboards/alerts/tests/*.test.yml` パラ ケ CI が出力されます。`promtool test rules` はガードレールを監視します。
3. **聴覚の証拠** Durante los ensayos `TRACE-LANE-ROUTING` y `TRACE-TELEMETRY-BRIDGE` el on-call captura los resultados deConsultas de Prometheus、el historial de warningas y las salidas importantes de scripts (`scripts/telemetry/check_nexus_audit_outcome.py`、`scripts/telemetry/check_redaction_status.py` パラ セナーレス コレラシオナダ) ロス アルマセナ コン ロス アーティファクト ルーテッド トレース。
4. **エスカラミエント** シ アルガン ガードレールは、緊急事態に備えて、事故のチケット Nexus の責任を負い、エステ プランを参照し、メトリックのスナップショットを含む、安全性を確保するための安全性を確保します。

公開された情報 - `roadmap.md` と `status.md` - ロードマップ **B2** の項目は、「責任、制限、通知、検証」の承認基準を満たしています。