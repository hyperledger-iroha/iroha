---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-telemetry-remediation
タイトル: خطة معالجة تيليمترية Nexus (B2)
説明: `docs/source/nexus_telemetry_remediation_plan.md` の評価と評価。
---

# いいえ

**B2 - ملكية فجوات القياس** يتطلب خطة منشورة تربط كل فجوة قياس متبقية في Nexus باشارة وحاجز تنبيه ومالك وموعد نهائي واثر تحقق قبل بدء نوافذ تدقيق Q1 2026. تعكس هذه الصفحة `docs/source/nexus_telemetry_remediation_plan.md` لكي يتمكن فريق هندسة الاصدار وعمليات القياس ومالكو SDK منルートトレース و `TRACE-TELEMETRY-BRIDGE`。

# और देखें

| और देखें और देखें और देखें世界時間 (UTC) | और देखें
|----------|----------------------|----------------------|-----------|----------------------|
| `GAP-TELEM-001` | مخطط تكراري `torii_lane_admission_latency_seconds{lane_id,endpoint}` مع تنبيه **`SoranetLaneAdmissionLatencyDegraded`** يعمل عندما `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` لمدة 5 دقائق (`dashboards/alerts/soranet_lane_rules.yml`)。 | `@torii-sdk` (アストラル) + `@telemetry-ops` (評価)オンコール ルーテッド トレース Nexus。 | 2026-02-23 | ختبارات التنبيه تحت `dashboards/alerts/tests/soranet_lane_rules.test.yml` مع لقطة تمرين `TRACE-LANE-ROUTING` التي تظهر التنبيه عند Torii `/metrics` في [Nexus 移行ノート](./nexus-transition-notes)。 |
| `GAP-TELEM-002` | عداد `nexus_config_diff_total{knob,profile}` مع حاجز `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` يمنع عمليات النشر (`docs/source/telemetry.md`)。 | `@nexus-core` (国際) -> `@telemetry-ops` (国際) عرض المزيد بشكل غير متوقع. | 2026-02-26 |予行演習 `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`؛ Prometheus مقتطف سجل يثبت ان `StateTelemetry::record_nexus_config_diff` الفارق. |
| `GAP-TELEM-003` | حدث `TelemetryEvent::AuditOutcome` (المترية `nexus.audit.outcome`) مع تنبيه **`NexusAuditOutcomeFailure`** عندما تستمر حالات الفشل او النتائج 30 月 30 日 (`dashboards/alerts/nexus_audit_rules.yml`)。 | `@telemetry-ops` (パイプライン) `@sec-observability`。 | 2026-02-27 | CI `scripts/telemetry/check_nexus_audit_outcome.py` ペイロード NDJSON のペイロード NDJSON のテスト TRACE テストルートトレースを追跡します。 |
| `GAP-TELEM-004` |ゲージ `nexus_lane_configured_total` حاجز `nexus_lane_configured_total != EXPECTED_LANE_COUNT` يغذي قائمة التحقق الخاصة بـ SRE المناوب。 | `@telemetry-ops` (ゲージ/エクスポート) مع تصعيد الى `@nexus-core` عندما تبلغ العقد عن احجام كتالوج غير متناسقة。 | 2026-02-28 | `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` يثبت الاصدار؛ Prometheus + مقتطف سجل `StateTelemetry::set_nexus_catalogs` بحزمة تمرين TRACE。 |

# और देखें

1. **فرز اسبوعي.** يبلغ المالكون عن التقدم في مكالمة جاهزية Nexus؛ `status.md` です。
2. **تجارب التنبيه.** تشحن كل قاعدة تنبيه مع مدخل `dashboards/alerts/tests/*.test.yml` بحيث ينفذ CI الامر `promtool test rules`最高です。
3. ** ادلة التدقيق.** خلال تمارين `TRACE-LANE-ROUTING` و `TRACE-TELEMETRY-BRIDGE` يلتقط المناوب نتائج استعلامات Prometheus (`scripts/telemetry/check_nexus_audit_outcome.py`、`scripts/telemetry/check_redaction_status.py` 日本語版)ルートトレースを実行します。
4. **تصعيد.** اذا تم اطلاق اي حاجز خارج نافذة تمرين، يفتح الفريق المالك تذكرة حادث Nexus تشير الهذه الخطة، مع تضمين لقطة المترية وخطوات التخفيف قبل استئناف عملياتああ。

ロードマップ **B2** ロードマップ **B2** ロードマップ **B2** ロードマップ **B2** ロードマップ **B2** ロードマップ **B2** ロードマップ **B2** القبول "المسؤولية، الموعد النهائي، التنبيه، التحقق"。