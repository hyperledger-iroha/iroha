---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/telemetry-remediation.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معالجة العلاقة بين القياس عن بعد
العنوان: Nexus خطة الريمنجتون (B2)
الوصف: `docs/source/nexus_telemetry_remediation_plan.md` هو هذا، جو ليب ميركس وعمل خرافي في كرتا.
---

#جائزہ

روڈ ميب آياتم **B2 - ٹيليمتري جيپس مليكت** عبارة عن خطة شائعة مطلوبة ومطلوبة من قبل Nexus وهي في ما بعد ٹيليميري جيت، الجريدة الرسمية، أونر، أي شبكة إنترنت وإصدار افتراضي واضح، حتى الربع الأول من عام 2026، بدأت نهاية الأسبوع في نهاية المطاف. تحتوي هذه الصفحة `docs/source/nexus_telemetry_remediation_plan.md` على هندسة الإصدار وعمليات القياس عن بعد وSDK تتبع التتبع و`TRACE-TELEMETRY-BRIDGE` الذي يرسل أحدث برامج البحث.

# گيپ ميتركس

| معرف گیپ | السغنل والرٹ گرڈ رايل | اونر / اسکیلشن | عبر الإنترنت (UTC) | ثبوت و ویریفیکیشن |
|--------|----------------------------------------|-----|----------|-------------------------|
| `GAP-TELEM-001` | لقد تم إنشاء `torii_lane_admission_latency_seconds{lane_id,endpoint}` والرٹ **`SoranetLaneAdmissionLatencyDegraded`** لقد حان الوقت للمضي قدماً إلى `histogram_quantile(0.95, rate(bucket[5m])) * 1000 > 750` من خلال حل المشكلة (`dashboards/alerts/soranet_lane_rules.yml`). | `@torii-sdk` (سگنل) + `@telemetry-ops` (الرٹ)؛ Nexus تتبع التوجيه عند الطلب. | 2026-02-23 | القائمة `dashboards/alerts/tests/soranet_lane_rules.test.yml` تحت، ساتيه `TRACE-LANE-ROUTING` تشير إلى كمبيوتر محمول يعمل بالفحم/السجل الإضافي وTorii `/metrics` سكريپ [Nexus ملاحظات الانتقال](./nexus-transition-notes) محفوظه و۔ |
| `GAP-TELEM-002` | تم الترحيب بالرقم `nexus_config_diff_total{knob,profile}` والريل `increase(nexus_config_diff_total{profile="active"}[5m]) > 0` (`docs/source/telemetry.md`). | `@nexus-core` (انسٹرومينٹیشن) -> `@telemetry-ops` (الرٹ)؛ ستعود التوقعات المستقبلية للمحرك الكهربائي في المستقبل القريب. | 2026-02-26 | تشغيل التشغيل الجاف آؤٹ پٹس `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` وهو محفوظ؛ ريليز تشيك هي Prometheus وهي متوافقة مع سكرين الشات و`StateTelemetry::record_nexus_config_diff` diff emmit كرنفالية وهي لا تحتوي على نصوص شاملة. |
| `GAP-TELEM-003` | أيون `TelemetryEvent::AuditOutcome` (ميتر `nexus.audit.outcome`) والرلت **`NexusAuditOutcomeFailure`** بعد الفشل أو فقدان النتائج 30 شهرًا أكثر (`dashboards/alerts/nexus_audit_rules.yml`). | `@telemetry-ops` (خط الأنابيب) واسكيلشن `@sec-observability` ت. | 2026-02-27 | يتم حفظ حمولات CI `scripts/telemetry/check_nexus_audit_outcome.py` NDJSON وإذا لم يتم تنزيل TRACE ونڈمیابی کایونٹ؛ تم نشر تقرير التتبع الموجه للصفحة على الشاشة. |
| `GAP-TELEM-004` | يتم قطع كل من `nexus_lane_configured_total` وGRILL `nexus_lane_configured_total != EXPECTED_LANE_COUNT` SRE عند الطلب. | `@telemetry-ops` (قياس/تصدير) وسكيلشن `@nexus-core` قلم سريع لا يتوافق مع التقرير. | 2026-02-28 | جدولة ٹیليمیٹر ٹیست `crates/iroha_core/tests/scheduler_telemetry.rs::records_lane_catalog_size` إيميشن ثابت کرتا؛ البروتوكولات Prometheus diff + `StateTelemetry::set_nexus_catalogs` لاگرس المرسل TRACE پيكیج پیكیج یتم تسويته. |

# آبريشنل عمل فلو

1. **افتتاح حرب الحرب.** Nexus الاستعداد لنشر تقرير كامل؛ تم وضع حاصرات والقائمة المصورة `status.md` في الدرج.
2. **التشغيل الجاف الخلفي.** الدور الرئيسي هو `dashboards/alerts/tests/*.test.yml` بوابة داخلية ودرزة حماية بديلة لـ CI `promtool test rules`.
3. **آيوان.** `TRACE-LANE-ROUTING` و`TRACE-TELEMETRY-BRIDGE` يرسلان ما يدور حولهما إلى Prometheus ويتوافقان مع النتائج والصورة وما يتعلق بالسكربت. پٹس (`scripts/telemetry/check_nexus_audit_outcome.py`، `scripts/telemetry/check_redaction_status.py` الإشارات المترابطة) تم جمع الكرتا والتتبع الموجه آرٹیفیكٹس وهو محفوظات كرتا.
4. **اسكيلشن.** إذا تعلق الأمر بحادثة مواصلات Nexus في حادثة 21NT00000008X، فإن ميرك أسنيپ ٹ و تتضمن خطوات التخفيف المزيد من الخطوات الأولى.

تم نشر هذه العلامات التجارية - و`roadmap.md` و`status.md` حول حوالة يومية - روڤر ميب **B2** اب "ذم، عبر الإنترنت، الإنترنت، ويريفيكون" معاير پرپورا اترتا ہے۔