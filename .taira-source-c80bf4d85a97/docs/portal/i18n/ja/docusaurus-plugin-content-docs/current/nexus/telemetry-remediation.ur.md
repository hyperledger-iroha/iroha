---
lang: ja
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
ID: nexus-telemetry-remediation
title: Nexus ٹیلیمیٹری ریمیڈی ایشن پلان (B2)
説明: `docs/source/nexus_telemetry_remediation_plan.md` کا آئینہ، جو ٹیلیمیٹری گیپ میٹرکس اور آپریشنل ورک فلو دستاویز کرتا ❁❁❁❁
---

# और देखें

**B2 - ٹیلیمیٹری گیپس کی ملکیت** ایک شائع شدہ پلان کا تقاضا کرتا ہے جو Nexus ٩ی ہر زیر التواء ٹیلیمیٹری گیپ کو سگنل، الرٹ گارڈ ریل، النر، ڈیڈ لائن اور ویریفیکیشن آرٹیفیکٹ سے جوڑے، تاکہ 2026 Q1 کی آڈٹ ونڈوز شروع ہونے سے پہلے سب ٩چھ واضح ہو۔ `docs/source/nexus_telemetry_remediation_plan.md` リリース エンジニアリング テレメトリ オペレーション SDK ルーテッド トレース `TRACE-TELEMETRY-BRIDGE` リリース エンジニアリングپہرسل سے پہلے کوریج کی تصدیق کر سکیں۔

# और देखें

| ID | سگنل اور الرٹ گارڈ ریل | और देखें世界標準 (UTC) | ثبوت اور ویریفیکیشن |
|----------|----------------------|----------------------|-----------|----------------------|
| `GAP-TELEM-001` | ہسٹوگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` الرٹ **`SoranetLaneAdmissionLatencyDegraded`** جو اس وقت فائر ہوتا ہے جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` پانچ منٹ تک (`dashboards/alerts/soranet_lane_rules.yml`)。 | `@torii-sdk` (国際) + `@telemetry-ops` (国際) Nexus ルーテッド トレース オンコール| 2026-02-23 | الرٹ ٹیسٹس `dashboards/alerts/tests/soranet_lane_rules.test.yml` کے تحت، ساتھ `TRACE-LANE-ROUTING` ریہرسل کی کیپچر جس میں الرٹ فائر/ریکور دکھایا گیا ہو اور Torii `/metrics` اسکریپ [Nexus 移行メモ](./nexus-transition-notes) میں حفوظ ہو۔ |
| `GAP-TELEM-002` | ٩اؤنٹر `nexus_config_diff_total{knob,profile}` اور گارڈ ریل `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` جو ڈپلائز کو گیٹ کرتا ہے (`docs/source/telemetry.md`)。 | `@nexus-core` (انسٹرومنٹیشن) -> `@telemetry-ops` (الرٹ) غیر متوقع بڑھوتری پر گورننس ڈیوٹی آفیسر پیج ہوتا ہے۔ | 2026-02-26 |予行演習 `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ساتھ محفوظ؛ Prometheus کوئری اسکرین شاٹ اور `StateTelemetry::record_nexus_config_diff` کے diff 放出 کرنے کا لاگ اقتباس और देखें |
| `GAP-TELEM-003` | `TelemetryEvent::AuditOutcome` (`nexus.audit.outcome`) **`NexusAuditOutcomeFailure`** 失敗数 結果が見つからない 30 個(`dashboards/alerts/nexus_audit_rules.yml`)。 | `@telemetry-ops` (パイプライン) `@sec-observability` テスト| 2026-02-27 | CI گیٹ `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON ペイロード محفوظ کرتا ہے اور اگر TRACE ونڈو میں کامیابی کا ایونٹ نہ ہو تو فیل ہو認証済みルートトレースのルートトレースのルート|
| `GAP-TELEM-004` | گیج `nexus_lane_configured_total` اور گارڈ ریل `nexus_lane_configured_total != EXPECTED_LANE_COUNT` جو SRE オンコール چیک لسٹ کو فیڈ کرتا ہے۔ | `@telemetry-ops` (ゲージ/エクスポート) اور اسکیلشن `@nexus-core` کی طرف جب نوڈز کیٹلاگ سائز میں عدم مطابقتありがとうございます| 2026-02-28 |スケジューラー ٹیلیمیٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` ایمیشن ثابت کرتا ہے؛ Prometheus diff + `StateTelemetry::set_nexus_catalogs` لاگ اقتباس TRACE ریہرسل پیکیج کے ساتھ جوڑتے ہیں۔ |

# آپریشنل ورک فلو

1. **ہفتہ وار ٹریاج۔** اونرز Nexus 準備完了 ةال میں پیش رفت رپورٹ کرتے ہیں؛ブロッカー اور الرٹ ٹیسٹ آرٹیفیکٹس `status.md` میں درج ہوتے ہیں۔
2. **予行演習** ہر الرٹ رول کے ساتھ `dashboards/alerts/tests/*.test.yml` انٹری دی جاتی ہے تاکہ ガードレール بدلنے پر CI `promtool test rules`ああ
3. **آڈٹ ایویڈنس۔** `TRACE-LANE-ROUTING` اور `TRACE-TELEMETRY-BRIDGE` ریہرسل کے دوران آن کال Prometheus کوئریز کے相関信号 (`scripts/telemetry/check_nexus_audit_outcome.py`、`scripts/telemetry/check_redaction_status.py` 相関信号)ルートトレース ルートトレース ルートトレース ルートトレース ルートトレース ルートトレース ルートトレース ルートトレース
4. **اسکیلشن۔** اگر کوئی ガードレール ریہرسل ونڈو کے باہر فائر ہو تو متعلقہ ٹیم اس پلان کے حوالہ Nexus インシデント ٹکٹ فائل کرتی ہے، میٹرک اسنیپ شاٹ اور 緩和措置 شامل کر کے آڈٹس دوبارہ شروع کرتی ہے۔

اس میٹرکس کی اشاعت - اور `roadmap.md` و `status.md` میں حوالہ دینے کے ساتھ - روڈ میپ آئٹم **B2** اب "ذمہ داری، ڈیڈ لائن، الرٹ، ویریفیکیشن" کی قبولیت معیار پر پورا اترتا ہے۔