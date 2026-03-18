---
lang: ru
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
идентификатор: Nexus-Telemetry-Remediation
title: Nexus ٹیلیمیٹری ریمیڈی ایشن پلان (B2)
описание: `docs/source/nexus_telemetry_remediation_plan.md` کا آئینہ، جو ٹیلیمیٹری گیپ میٹرکس اور آپریشنل ورک فلو دستاویز کرتا ہے۔
---

# جائزہ

روڈ میپ آئٹم **B2 - ٹیلیمیٹری گیپس کی ملکیت** ایک شائع شدہ کا تقاضا Если у вас есть Nexus, вы можете получить доступ к информации, которая вам нужна. Годовой отчет в первом квартале 2026 года. آڈٹ ونڈوز شروع ہونے سے پہلے سب کچھ واضح ہو۔ Используйте `docs/source/nexus_telemetry_remediation_plan.md` для разработки релизов, телеметрии и SDK для маршрутизированной трассировки `TRACE-TELEMETRY-BRIDGE`. کی ریہرسل سے پہلے کوریج کی تصدیق کر سکیں۔

# گیپ میٹرکس

| ID ID | سگنل اور الرٹ گارڈ ریل | اونر / اسکیلشن | ڈیڈ لائن (UTC) | ثبوت اور ویریفیکیشن |
|--------|-------------------------|--------------------|-----------|-------------------------|
| `GAP-TELEM-001` | ہسٹوگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` اور الرٹ **`SoranetLaneAdmissionLatencyDegraded`** جو اس وقت فائر ہوتا ہے جب `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` Для этого необходимо установить (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ); Nexus маршрутизация по вызову | 2026-02-23 | Используйте `dashboards/alerts/tests/soranet_lane_rules.test.yml`, чтобы установить `TRACE-LANE-ROUTING`, чтобы узнать, как это сделать. Torii `/metrics` اسکریپ [Nexus примечания к переходу](./nexus-transition-notes) میں محفوظ ہو۔ |
| `GAP-TELEM-002` | Установите `nexus_config_diff_total{knob,profile}` и установите `increase(nexus_config_diff_total{profile="active"}[5m]) > 0`, чтобы установить его (`docs/source/telemetry.md`). | `@nexus-core` (انسٹرومنٹیشن) -> `@telemetry-ops` (الرٹ)؛ غیر متوقع بڑھوتری پر گورننس ڈیوٹی آفیسر پیج ہوتا ہے۔ | 26 февраля 2026 г. | Для пробного прогона используется `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`. Используйте Prometheus для создания различий в `StateTelemetry::record_nexus_config_diff`. اقتباس شامل ہو۔ |
| `GAP-TELEM-003` | ایونٹ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) اور الرٹ **`NexusAuditOutcomeFailure`** сбои, отсутствующие результаты 30 минут назад Установите флажок (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (конвейер) | 2026-02-27 | CI `scripts/telemetry/check_nexus_audit_outcome.py` Полезные нагрузки NDJSON в зависимости от типа TRACE или TRACE ہو تو فیل ہو جاتا ہے؛ Воспользуйтесь функцией Routed-Trace, чтобы узнать больше о маршрутизации. |
| `GAP-TELEM-004` | `nexus_lane_configured_total` или `nexus_lane_configured_total != EXPECTED_LANE_COUNT` для SRE по вызову, когда вам нужно | `@telemetry-ops` (измеритель/экспорт) `@nexus-core` может быть использован для настройки параметров مطابقت رپورٹ کریں۔ | 28 февраля 2026 г. | планировщик ٹیلیمیٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` Используйте Prometheus diff + `StateTelemetry::set_nexus_catalogs` для проверки TRACE. ہیں۔ |

# آپریشنل ورک فلو1. **В случае проверки** Nexus готовность к проверке может быть отключена. Блокировщики ارٹیفیکٹس `status.md` میں درج ہوتے ہیں۔
2. **Сухой прогон** ہر الرٹ رول کے ساتھ `dashboards/alerts/tests/*.test.yml` انٹری دی جاتی ہے تاکہ ограждение بدلنے پر CI `promtool test rules` چلائے۔
3. **Задайте вопрос** `TRACE-LANE-ROUTING` или `TRACE-TELEMETRY-BRIDGE`, чтобы установить Prometheus Если вы хотите, чтобы ваш компьютер был установлен (`scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` обеспечивает коррелированные сигналы).
4. **Установите ограждение, которое можно использовать для ограждения, которое можно использовать в качестве защитного ограждения. В случае инцидента Nexus вы можете узнать, какие меры можно предпринять для смягчения последствий инцидента. کر کے آڈٹس دوبارہ شروع کرتی ہے۔

В качестве примера можно указать `roadmap.md` и `status.md`, чтобы узнать больше о том, как это сделать. Если вы хотите, чтобы **B2** был выбран, вы можете использовать его как **B2**. پر پورا اترتا ہے۔