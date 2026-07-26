---

> NOTE: This translation has not yet been updated for the v1 DA availability (advisory). Refer to the English source for current semantics.
lang: ar
direction: rtl
source: docs/source/runbooks/nexus_lane_finality.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1d9fbcf5301bd36a0bdc8a010ae214f7b1561373237054ab3c8a45a411cf6c0e
source_last_modified: "2025-12-14T09:53:36.241505+00:00"
translation_last_reviewed: 2025-12-28
---

<div dir="rtl">

# دليل نهائية مسارات Nexus والأوراكل

**الحالة:** نشط — يحقق متطلبات NX-18 للوحة/الدليل التشغيلي.  
**الجمهور:** Core Consensus WG، SRE/Telemetry، Release Engineering، قادة المناوبة.  
**النطاق:** يغطي SLOs لمدة الفتحة، ونصاب DA، والأوراكل، ومخزن التسوية الذي يحكم
وعد النهاية خلال 1 ث. استخدمه مع `dashboards/grafana/nexus_lanes.json` وأدوات
القياس تحت `scripts/telemetry/`.

## لوحات المتابعة

- **Grafana (`dashboards/grafana/nexus_lanes.json`)** — ينشر لوحة “Nexus Lane Finality & Oracles”. تتبع اللوحات:
  - `histogram_quantile()` على `iroha_slot_duration_ms` (p50/p95/p99) مع مقياس آخر عينة.
  - `iroha_da_quorum_ratio` و `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` لإبراز اضطراب DA.
  - إشارات الأوراكل: `iroha_oracle_price_local_per_xor`, `iroha_oracle_staleness_seconds`, `iroha_oracle_twap_window_seconds`, `iroha_oracle_haircut_basis_points`.
  - لوحة مخزن التسوية (`iroha_settlement_buffer_xor`) التي تعرض خصومات لكل lane من إيصالات `LaneBlockCommitment`.
- **قواعد التنبيه** — تعيد استخدام بنود Slot/DA SLO من `ans3.md`. التنبيه عند:
  - p95 لمدة الفتحة > 1000 ms في نافذتين متتاليتين 5 م،
  - نسبة نصاب DA < 0.95 أو `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m]) > 0`،
  - Staleness للأوراكل > 90 ث أو نافذة TWAP ≠ 60 ث المضبوطة،
  - مخزن التسوية < 25 % (soft) / 10 % (hard) عند تفعيل المقياس.

## ملخص سريع للمقاييس

| المقياس | الهدف / التنبيه | ملاحظات |
|--------|----------------|-------|
| `histogram_quantile(0.95, iroha_slot_duration_ms)` | ≤ 1000 ms (hard)، 950 ms warning | استخدم لوحة القياس أو شغّل `scripts/telemetry/check_slot_duration.py` (`--json-out artifacts/nx18/slot_summary.json`) على تصدير Prometheus أثناء اختبارات chaos. |
| `iroha_slot_duration_ms_latest` | يعكس أحدث فتحة؛ حقّق إذا > 1100 ms حتى لو بدت الكوانتيالات سليمة. | صدّر القيمة عند فتح الحوادث. |
| `iroha_da_quorum_ratio` | ≥ 0.95 على نافذة متحركة 30 م. | مشتق من إعادة جدولة DA أثناء commits. |
| `increase(sumeragi_da_gate_block_total{reason="missing_local_data"}[5m])` | يجب أن يبقى 0 خارج rehearsals. | اعتبر أي زيادة مستمرة `missing-availability warning`. |

كل إعادة جدولة تطلق أيضاً تحذيراً في خط Torii مع `kind = "missing-availability warning"`. التقط هذه الأحداث مع ذروة القياس لتحديد رأس الكتلة المتأثر، محاولة الإعادة، وعدّادات requeue دون التنقيب في سجلات المصدّقين.【crates/iroha_core/src/sumeragi/main_loop.rs:5164】
| `iroha_oracle_staleness_seconds` | ≤ 60 ث. تنبيه عند 75 ث. | يشير إلى TWAP feeds قديمة 60 ث. |
| `iroha_oracle_twap_window_seconds` | 60 ث بالضبط ± سماحية 5 ث. | الانحراف يعني خطأ ضبط الأوراكل. |
| `iroha_oracle_haircut_basis_points` | يطابق فئة سيولة lane (0/25/75 bps). | صعّد إن ارتفعت haircuts دون مبرر. |
| `iroha_settlement_buffer_xor` | Soft 25 %، hard 10 %. فرض XOR‑only تحت 10 %. | اللوحة تعرض خصومات micro‑XOR لكل lane/dataspace؛ صدّر قبل تعديل سياسة الموجّه. |

## دليل الاستجابة

### خرق مدة الفتحة
1. تأكد عبر اللوحة و`promql` (p95/p99).  
2. التقط مخرجات `scripts/telemetry/check_slot_duration.py --json-out <path>`
   (وملتقط المقاييس) لتمكين مراجعات CXO من التحقق من بوابة 1 ث.  
3. افحص مدخلات RCA: عمق صف mempool، إعادة جدولة DA، تتبعات IVM.  
4. افتح حادثة، أرفق لقطة Grafana، وجدول تمرين chaos إذا استمر التراجع.

### تدهور نصاب DA
1. تحقق من `iroha_da_quorum_ratio` وعدّاد reschedule؛ طابق مع سجلات `missing-availability warning`.  
2. إذا كان ratio <0.95، ثبّت المصدقين المتعثرين، وسّع معلمات العيّنة، أو انقل الملف إلى وضع XOR‑only.  
3. شغّل `scripts/telemetry/check_nexus_audit_outcome.py` أثناء rehearsals routed‑trace لإثبات استمرار مرور `nexus.audit.outcome` بعد المعالجة.  
4. أرشِف حزم إيصالات DA مع تذكرة الحادثة.

### Staleness الأوراكل / انحراف haircut
1. استخدم اللوحات 5–8 للتحقق من السعر، staleness، نافذة TWAP، وhaircut.  
2. عند staleness >90 ث: أعد تشغيل feed الأوراكل أو فعّل failover، ثم أعد تشغيل chaos harness.  
3. عند mismatch في haircut: راجع إعدادات ملف السيولة وتغييرات الحوكمة الأخيرة؛ أخطر الخزانة إذا لزم التدخل.

### تنبيهات مخزن التسوية
1. استخدم `iroha_settlement_buffer_xor` (مع إيصالات ليلية) لتأكيد الهامش قبل تعديل سياسة الموجه.  
2. عند تجاوز العتبة:
   - **Soft breach (<25 %)**: أشرك الخزانة، فكّر في تفعيل خطوط swap، وسجّل التنبيه.  
   - **Hard breach (<10 %)**: فرض XOR‑only، رفض الخطوط المدعومة، والتوثيق في `ops/drill-log.md`.  
3. راجع `docs/source/settlement_router.md` لاختيارات repo/reverse‑repo.

## الأدلة والأتمتة

- **CI** — اربط `scripts/telemetry/check_slot_duration.py --json-out artifacts/nx18/slot_summary.json` و`scripts/telemetry/nx18_acceptance.py --json-out artifacts/nx18/nx18_acceptance.json <metrics.prom>` بسير قبول RC حتى يصدر كل RC ملخص مدة الفتحة ونتائج بوابة DA/oracle/buffer مع لقطة القياس المذكورة أعلاه. يتم استدعاء الأداة بالفعل من `ci/check_nexus_lane_smoke.sh`.  
- **مطابقة اللوحات** — شغّل `scripts/telemetry/compare_dashboards.py dashboards/grafana/nexus_lanes.json <prod-export.json>` لضمان تطابق اللوحة المنشورة مع صادرات staging/prod.  
- **أرشفة التتبعات** — أثناء rehearsals TRACE أو drills NX-18، نفّذ `scripts/telemetry/check_nexus_audit_outcome.py` لأرشفة أحدث payload لـ `nexus.audit.outcome` (`docs/examples/nexus_audit_outcomes/`). أرفق الأرشيف ولقطات Grafana في سجل التدريب.
- **تجميع دليل الفتحة** — بعد توليد JSON الملخص، نفّذ `scripts/telemetry/bundle_slot_artifacts.py --metrics <prometheus.tgz-extract>/metrics.prom --summary artifacts/nx18/slot_summary.json --out-dir artifacts/nx18` حتى يسجل `slot_bundle_manifest.json` digests SHA‑256 للقطعتين. ارفع الدليل كما هو مع حزمة إثبات RC. ينفذ خط الإصدار ذلك تلقائياً (يمكن التخطي عبر `--skip-nexus-lane-smoke`) وينسخ `artifacts/nx18/` إلى مخرجات الإصدار.

## قائمة صيانة

- أبقِ `dashboards/grafana/nexus_lanes.json` متزامناً مع صادرات Grafana بعد كل تغيير مخطط؛ وثّق التعديلات في رسائل الالتزام مع الإشارة إلى NX-18.  
- حدّث هذا الدليل عند إضافة مقاييس جديدة (مثل gauges مخزن التسوية) أو عتبات جديدة.  
- سجّل كل rehearsal chaos (تأخر الفتحة، DA jitter، تعطل الأوراكل، استنزاف المخزن) باستخدام `scripts/telemetry/log_sorafs_drill.sh --log ops/drill-log.md --program NX-18 --status <status>`.

اتباع هذا الدليل يوفر أدلة “لوحات/أدلة تشغيل للمشغّل” المطلوبة في NX-18 ويضمن بقاء SLO النهائي قابلاً للتنفيذ قبل Nexus GA.

</div>
