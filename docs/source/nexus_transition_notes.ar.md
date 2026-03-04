<!--
  SPDX-License-Identifier: Apache-2.0
-->

---
lang: ar
direction: rtl
source: docs/source/nexus_transition_notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: baf4e91fdb2c447c453711170e7df58a9a4831b15957052818736e0e7914e8a3
source_last_modified: "2025-12-13T06:33:05.401788+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/nexus_transition_notes.md -->

# ملاحظات انتقال Nexus

يتابع هذا السجل العمل المتبقي ضمن **المرحلة B - اساسيات انتقال Nexus** حتى تكتمل قائمة
اطلاق المسارات المتعددة. يكمل ادخالات المعالم في `roadmap.md` ويجمع الادلة المشار
اليها في B1-B4 في مكان واحد كي تشارك الحوكمة وفرق SRE وقيادات SDK نفس مصدر الحقيقة.

## النطاق والايقاع

- يغطي تدقيقات routed-trace وحواجز telemetria (B1/B2)، ومجموعة دلتا الاعدادات
  المعتمدة من الحوكمة (B3)، ومتابعات تمارين اطلاق multi-lane (B4).
- يستبدل ملاحظة الايقاع المؤقتة التي كانت هنا؛ منذ تدقيق 2026 Q1 اصبح التقرير التفصيلي
  في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، بينما تحتفظ هذه الصفحة
  بالجدول الجاري وسجل التخفيف.
- حدث الجداول بعد كل نافذة routed-trace او تصويت حوكمة او تمرين اطلاق. عندما تنتقل
  الادلة، عكس الموقع الجديد داخل هذه الصفحة كي ترتبط المستندات اللاحقة (status,
  dashboards, SDK portals) بنقطة ارتكاز مستقرة.

## لقطة الادلة (2026 Q1-Q2)

| مسار العمل | الادلة | المالكون | الحالة | ملاحظات |
|------------|--------|---------|--------|---------|
| **B1 - تدقيقات routed-trace** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | مكتمل (Q1 2026) | تم تسجيل ثلاث نوافذ تدقيق؛ تاخر TLS في `TRACE-CONFIG-DELTA` اغلق خلال اعادة التشغيل في Q2. |
| **B2 - معالجة telemetria والحواجز** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مكتمل | حزمة تنبيهات وسياسة diff bot وحجم دفعة OTLP (`nexus.scheduler.headroom` log + لوحة headroom في Grafana) تم شحنها؛ لا استثناءات مفتوحة. |
| **B3 - موافقات دلتا الاعدادات** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مكتمل | تم توثيق تصويت GOV-2026-03-19؛ الحزمة الموقعة تغذي حزمة telemetria المذكورة ادناه. |
| **B4 - تمرين اطلاق multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مكتمل (Q2 2026) | اعادة التشغيل التجريبية في Q2 اغلقت معالجة تاخر TLS؛ manifest المدققين + `.sha256` توثق نطاق slots 912-936 وبذرة الحمل `NEXUS-REH-2026Q2` وتجزيء ملف TLS المسجل من الاعادة. |

## جدول تدقيق routed-trace الفصلي

| Trace ID | النافذة (UTC) | النتيجة | ملاحظات |
|----------|---------------|---------|---------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ناجح | بقي P95 لادخال الطابور ادنى بكثير من الهدف <=750 ms. لا اجراء مطلوب. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | ناجح | تم ارفاق OTLP replay hashes في `status.md`; اكد SDK diff bot عدم وجود drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تم الحل | تاخر TLS اغلق خلال اعادة التشغيل Q2؛ حزمة telemetria لـ `NEXUS-REH-2026Q2` تسجل تجزيء ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (انظر `artifacts/nexus/tls_profile_rollout_2026q2/`) وصفر متاخرين. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | ناجح | بذرة حمل `NEXUS-REH-2026Q2`; حزمة telemetria + manifest/digest في `artifacts/nexus/rehearsals/2026q1/` (نطاق slots 912-936) مع agenda في `artifacts/nexus/rehearsals/2026q2/`. |

على الفصول القادمة اضافة صفوف جديدة ونقل الادخالات المكتملة الى ملحق عندما
يتجاوز الجدول الربع الحالي. ارجع الى هذا القسم من تقارير routed-trace او محاضر
الحوكمة باستخدام مرساة `#quarterly-routed-trace-audit-schedule`.

## عناصر التخفيف والمتاخرات

| العنصر | الوصف | المالك | الهدف | الحالة / ملاحظات |
|--------|-------|--------|-------|------------------|
| `NEXUS-421` | انهاء نشر ملف TLS الذي تاخر خلال `TRACE-CONFIG-DELTA`، وتوثيق دليل الاعادة، واغلاق سجل التخفيف. | @release-eng, @sre-core | نافذة routed-trace Q2 2026 | مغلق - تجزيء ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` موثق في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; الاعادة اكدت عدم وجود متاخرين. |
| تحضير `TRACE-MULTILANE-CANARY` | جدولة تمرين Q2، ارفاق fixtures بحزمة telemetria، وضمان اعادة استخدام SDK harness للاداة الموثقة. | @telemetry-ops, SDK Program | مكالمة تخطيط 2026-04-30 | مكتمل - agenda محفوظة في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع بيانات slot/الحمل؛ تمت الاشارة لاعادة استخدام harness في tracker. |
| تدوير digests لحزمة telemetria | تشغيل `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل تمرين/اصدار وتسجيل digests بجانب tracker دلتا الاعدادات. | @telemetry-ops | لكل release candidate | مكتمل - `telemetry_manifest.json` + `.sha256` صدرت في `artifacts/nexus/rehearsals/2026q1/` (نطاق slots `912-936`, بذرة `NEXUS-REH-2026Q2`); تم نسخ digests الى tracker وفهرس الادلة. |

## تكامل حزمة دلتا الاعدادات

- يبقى `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` الملخص
  المرجعي للفروق. عند وصول `defaults/nexus/*.toml` جديدة او تغييرات genesis،
  حدث tracker اولا ثم اعكس الملخص هنا.
- الحزم الموقعة للاعدادات تغذي حزمة telemetria للتمرين. يجب نشر الحزمة، التي
  تتحقق بواسطة `scripts/telemetry/validate_nexus_telemetry_pack.py`, بجانب
  ادلة دلتا الاعدادات كي يتمكن المشغلون من اعادة تشغيل نفس الادلة المستخدمة في B4.
- تبقى حزم Iroha 2 بلا lanes: الاعدادات مع `nexus.enabled = false` ترفض الان
  overrides لـ lane/dataspace/routing الا اذا كان ملف Nexus مفعلا (`--sora`)،
  لذا احذف اقسام `nexus.*` من قوالب single-lane.
- ابق سجل تصويت الحوكمة (GOV-2026-03-19) مرتبطا من tracker ومن هذه المذكرة
  كي تنسخ الاصوات القادمة التنسيق دون اعادة اكتشاف طقوس الموافقة.

## متابعات تمرين الاطلاق

- `docs/source/runbooks/nexus_multilane_rehearsal.md` يوثق خطة canary وقائمة
  المشاركين وخطوات rollback؛ حدث الدليل عند تغير طوبولوجيا lanes او exporters telemetria.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` يسرد كل الادلة التي
  تمت مراجعتها في تمرين 9 ابريل ويحتوي الان على ملاحظات/agenda Q2. اضف التمارين
  القادمة الى نفس tracker بدلا من فتح trackers منفصلة للحفاظ على تتابع الادلة.
- انشر مقاطع OTLP collector وتصديرات Grafana (انظر `docs/source/telemetry.md`)
  عند تغير ارشاد batching؛ تحديث Q1 رفع حجم الدفعة الى 256 عينة لتجنب تنبيهات headroom.
- ادلة CI/test multi-lane تعيش الان في
  `integration_tests/tests/nexus/multilane_pipeline.rs` وتعمل تحت workflow
  `Nexus Multilane Pipeline`
  (`.github/workflows/integration_tests_multilane.yml`)، بدلا من المرجع المتقاعد
  `pytests/nexus/test_multilane_pipeline.py`; حافظ على تجزيء
  `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b
  `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) متزامنا
  مع tracker عند تحديث bundles التمرين.

## دورة حياة lane اثناء runtime

- خطط دورة حياة lane اثناء runtime تتحقق الان من bindings dataspaces وتوقف التنفيذ
  عند فشل reconciliation بين Kura/التخزين الطبقي، وتترك الكتالوج دون تغيير. تقوم
  helpers بازالة lane relays المخزنة للـ lanes المتقاعدة حتى لا يعاد استخدام proofs قديمة.
- طبق الخطط عبر helpers Nexus config/lifecycle (`State::apply_lane_lifecycle`,
  `Queue::apply_lane_lifecycle`) لاضافة/ازالة lanes دون اعادة تشغيل؛ يعاد تحميل routing
  و TEU snapshots و manifest registries تلقائيا بعد الخطة الناجحة.
- ارشاد للمشغلين: عند فشل الخطة، تحقق من dataspaces المفقودة او roots التخزين التي
  لا يمكن انشاؤها (cold root طبقي/مجلدات Kura للـ lane). اصلح المسارات واعاود المحاولة؛
  الخطط الناجحة تعيد بث diff telemetria للـ lane/dataspace كي تعكس dashboards الطوبولوجيا الجديدة.

## ادلة telemetria و backpressure لـ NPoS

طلبت مراجعة تمرين الاطلاق لمرحلة B ادلة telemetria حتمية تثبت بقاء pacemaker NPoS
وطبقات gossip ضمن حدود backpressure. يقوم harness التكامل في
`integration_tests/tests/sumeragi_npos_performance.rs` بتشغيل هذه السيناريوهات ويطبع
ملخصات JSON (`sumeragi_baseline_summary::<scenario>::...`) عند وصول مقاييس جديدة.
شغله محليا عبر:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

اضبط `SUMERAGI_NPOS_STRESS_PEERS`, `SUMERAGI_NPOS_STRESS_COLLECTORS_K` او
`SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات اعلى ضغطا؛ القيم
الافتراضية تعكس ملف مجمعي 1 s/`k=3` المستخدم في B4.

| السيناريو / الاختبار | التغطية | telemetria الاساسية |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | يحجز 12 جولة بزمن الكتلة الخاص بالتمرين لتسجيل اغطية كمون EMA واعمق الطوابير ومقاييس redundant-send قبل تسلسل حزمة الادلة. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | يغمر طابور المعاملات لضمان تفعيل تأجيلات admission بشكل حتمي وتصدير عدادات السعة/التشبع. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | يرصد jitter الخاص بالpacemaker وtimeouts للعرض حتى يثبت ان نطاق +/-125 permille مطبق. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | يدفع payloads RBC كبيرة حتى حدود store اللينة/الصلبة لاظهار ارتفاع العدادات ثم تراجعها واستقرارها دون تجاوز store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | يجبر اعادات الارسال كي تتقدم مقاييس redundant-send وعدادات collectors-on-target، مؤكدا ان telemetria موصولة من الطرف الى الطرف. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | يسقط chunks بمسافات حتمية للتحقق من ان مراقبات backlog ترفع الاعطال بدلا من تصريف payloads بصمت. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

ارفق اسطر JSON التي يطبعها harness مع لقطة Prometheus الملتقطة اثناء التشغيل عندما
تطلب الحوكمة دليلا على ان انذارات backpressure تطابق طوبولوجيا التمرين.

## قائمة التحديث

1. اضف نوافذ routed-trace جديدة وازل القديمة عند تبدل الفصول.
2. حدث جدول التخفيف بعد كل متابعة Alertmanager حتى لو كان الاجراء اغلاق التذكرة.
3. عند تغير دلتا الاعدادات، حدث tracker وهذه المذكرة وقائمة digests لحزمة telemetria
   في نفس pull request.
4. اربط اي ادلة تمرين/telemetria جديدة هنا كي تشير تحديثات status المستقبلية الى وثيقة
   واحدة بدلا من ملاحظات متفرقة.

## فهرس الادلة

| الاصل | الموقع | ملاحظات |
|-------|--------|---------|
| تقرير تدقيق routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المصدر المرجعي لادلة Phase B1؛ نسخة للبوابة في `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| tracker دلتا الاعدادات | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | يحتوي ملخصات TRACE-CONFIG-DELTA، واحرف المراجعين، وسجل تصويت GOV-2026-03-19. |
| خطة معالجة telemetria | `docs/source/nexus_telemetry_remediation_plan.md` | توثق حزمة التنبيهات وحجم دفعة OTLP وحواجز ميزانية التصدير المرتبطة بـ B2. |
| tracker تمرين multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | يسرد ادلة تمرين 9 ابريل، وmanifest/digest للمدققين، وملاحظات/agenda Q2 وادلة rollback. |
| manifest/digest لحزمة telemetria (الاخير) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | يسجل نطاق slots 912-936 وبذرة `NEXUS-REH-2026Q2` وتجزيئات الادلة لحزم الحوكمة. |
| manifest ملف TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | تجزيء ملف TLS المعتمد الذي تم توثيقه خلال اعادة التشغيل Q2؛ يذكر في ملاحق routed-trace. |
| agenda TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | ملاحظات تخطيط تمرين Q2 (النافذة، نطاق slots، بذرة الحمل، مالكو المهام). |
| runbook تمرين الاطلاق | `docs/source/runbooks/nexus_multilane_rehearsal.md` | قائمة تشغيل للتهيئة -> التنفيذ -> rollback؛ حدثها عند تغير طوبولوجيا lanes او ارشادات exporters. |
| مدقق حزمة telemetria | `scripts/telemetry/validate_nexus_telemetry_pack.py` | اداة CLI مطلوبة من مراجعة B4؛ ارشفة digests بجانب tracker عند تغير الحزمة. |
| Regression multi-lane | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | يثبت `nexus.enabled = true` لاعدادات multi-lane، ويحافظ على تجزيئات كتالوج Sora، ويجهز مسارات Kura/merge-log المحلية (`blocks/lane_{id:03}_{slug}`) عبر `ConfigLaneRouter` قبل نشر digests الادلة. |

</div>
