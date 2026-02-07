---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/telemetry-remediation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : lien-télémétrie-remédiation
titre : Nexus ٹیلیمیٹری ریمیڈی ایشن پلان (B2)
description: `docs/source/nexus_telemetry_remediation_plan.md` آئینہ، جو ٹیلیمیٹری گیپ میٹرکس اور آپریشنل ورک فلو دستاویز کرتا ہے۔
---

# جائزہ

روڈ میپ آئٹم **B2 - ٹیلیمیٹری گیپس کی ملکیت** ایک شائع شدہ پلان کا تقاضا کرتا ہے جو Nexus کی ہر زیر التواء ٹیلیمیٹری گیپ کو سگنل، الرٹ گارڈ ریل، اونر، ڈیڈ لائن اور ویریفیکیشن آرٹیفیکٹ سے جوڑے، تاکہ Q1 2026 کی آڈٹ ونڈوز شروع ہونے سے پہلے سب کچھ واضح ہو۔ Pour `docs/source/nexus_telemetry_remediation_plan.md`, il s'agit de l'ingénierie des versions, des opérations de télémétrie et du SDK, ainsi que du routé-trace et de `TRACE-TELEMETRY-BRIDGE`. ریہرسل سے پہلے کوریج کی تصدیق کر سکیں۔

# گیپ میٹرکس| گیپ ID | سگنل اور الرٹ گارڈ ریل | اونر / اسکیلشن | ڈیڈ لائن (UTC) | ثبوت اور ویریفیکیشن |
|--------|-------------------------|----------|---------------|-------------------------|
| `GAP-TELEM-001` | ہسٹوگرام `torii_lane_admission_latency_seconds{lane_id,endpoint}` اور الرٹ **`SoranetLaneAdmissionLatencyDegraded`** est également disponible pour `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` پانچ منٹ تک برقرار رہے (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ)؛ Nexus routé-trace sur appel | 2026-02-23 | La carte `dashboards/alerts/tests/soranet_lane_rules.test.yml` est en cours de mise à jour `TRACE-LANE-ROUTING` La carte mère est disponible en ligne. دکھایا گیا ہو اور Torii `/metrics` اسکریپ [Nexus notes de transition](./nexus-transition-notes) میں محفوظ ہو۔ |
| `GAP-TELEM-002` | Le `nexus_config_diff_total{knob,profile}` et le `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` sont également disponibles (`docs/source/telemetry.md`). | `@nexus-core` (انسٹرومنٹیشن) -> `@telemetry-ops` (الرٹ)؛ غیر متوقع بڑھوتری پر گورننس ڈیوٹی آفیسر پیج ہوتا ہے۔ | 2026-02-26 | Fonctionnement à sec avec `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` pour le fonctionnement à sec Le capteur Prometheus s'allume en direction de `StateTelemetry::record_nexus_config_diff` pour émettre un diff. شامل ہو۔ || `GAP-TELEM-003` | ایونٹ `TelemetryEvent::AuditOutcome` (میٹرک `nexus.audit.outcome`) اور الرٹ **`NexusAuditOutcomeFailure`** échecs et résultats manquants 30 minutes de lecture (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (pipeline) et `@sec-observability` | 2026-02-27 | Les charges utiles CI `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON sont disponibles pour TRACE et TRACE pour les utilisateurs actuels. جاتا ہے؛ Il s'agit d'un service de trace acheminée qui est en cours de route. |
| `GAP-TELEM-004` | `nexus_lane_configured_total` pour le service `nexus_lane_configured_total != EXPECTED_LANE_COUNT` pour SRE de garde en cas d'urgence | `@telemetry-ops` (jauge/export) `@nexus-core` (jauge/exportation) کریں۔ | 2026-02-28 | planificateur ٹیلیمیٹری ٹیسٹ `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` ایمیشن ثابت کرتا ہے؛ آپریٹرز Prometheus diff + `StateTelemetry::set_nexus_catalogs` لاگ اقتباس TRACE ریہرسل پیکیج کے ساتھ جوڑتے ہیں۔ |

# آپریشنل ورک فلو1. ** ہفتہ وار ٹریاج۔** اونرز Nexus readiness کال میں پیش رفت رپورٹ کرتے ہیں؛ blockers اور الرٹ ٹیسٹ آرٹیفیکٹس `status.md` میں درج ہوتے ہیں۔
2. ** Fonctionnement à sec ** Le garde-corps `dashboards/alerts/tests/*.test.yml` est relié au garde-corps CI `promtool test rules` چلائے۔
3. **`TRACE-LANE-ROUTING` et `TRACE-TELEMETRY-BRIDGE` `TRACE-TELEMETRY-BRIDGE` Prometheus Prometheus Prometheus Prometheus Les signaux d'alarme et les signaux corrélés `scripts/telemetry/check_nexus_audit_outcome.py`, `scripts/telemetry/check_redaction_status.py` pour les signaux corrélés) Il s'agit d'un service de trace acheminée
4. **اسکیلشن۔** اگر کوئی garde-corps ریہرسل ونڈو کے باہر فائر ہو تو متعلقہ ٹیم اس پلان کے حوالہ L'incident Nexus est en cours de préparation et les mesures d'atténuation sont prises en compte. کرتی ہے۔

اس میٹرکس کی اشاعت - اور `roadmap.md` et `status.md` میں حوالہ دینے کے ساتھ - روڈ میپ آئٹم **B2** اب "ذمہ داری، ڈیڈ لائن، الرٹ، ویریفیکیشن" کی قبولیت معیار پر پورا اترتا ہے۔