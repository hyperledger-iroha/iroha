---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/transition-notes.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: da55b3415e1b49d38ee8a7e612d77f3995b10959628298317ef0cea9212dceaf
source_last_modified: "2025-12-20T09:39:49.354671+00:00"
translation_last_reviewed: 2025-12-30
---

---
id: nexus-transition-notes
title: ملاحظات انتقال Nexus
description: نسخة مطابقة لـ `docs/source/nexus_transition_notes.md`، تغطي ادلة انتقال Phase B وجدول التدقيق وخطط التخفيف.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات انتقال Nexus

يتتبع هذا السجل العمل المتبقي من **Phase B - Nexus Transition Foundations** حتى تكتمل قائمة فحص اطلاق الـ multi-lane. وهو يكمل بنود المعالم في `roadmap.md` ويحفظ الادلة المشار اليها في B1-B4 في مكان واحد حتى تتمكن فرق الحوكمة وSRE وقادة SDK من مشاركة نفس مصدر الحقيقة.

## النطاق والوتيرة

- يغطي تدقيقات routed-trace وحواجز guardrails للتيليمتري (B1/B2)، ومجموعة deltas للتكوين المعتمدة من الحوكمة (B3)، ومتابعات تدريب الاطلاق متعدد lanes (B4).
- يستبدل ملاحظة الوتيرة المؤقتة التي كانت هنا؛ منذ تدقيق Q1 2026 يوجد التقرير المفصل في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، بينما تحتفظ هذه الصفحة بالجدول التشغيلي وسجل التخفيفات.
- حدث الجداول بعد كل نافذة routed-trace او تصويت حوكمة او تدريب اطلاق. عندما تتحرك artefacts، اعكس الموقع الجديد داخل هذه الصفحة كي تتمكن الوثائق اللاحقة (status, dashboards, بوابات SDK) من الارتباط بمرساة ثابتة.

## لقطة ادلة (2026 Q1-Q2)

| مسار العمل | الادلة | الملاك | الحالة | ملاحظات |
|------------|----------|----------|--------|-------|
| **B1 - Routed-trace audits** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`, `docs/examples/nexus_audit_outcomes/` | @telemetry-ops, @governance | مكتمل (Q1 2026) | تم تسجيل ثلاث نوافذ تدقيق؛ تم اغلاق فجوة TLS في `TRACE-CONFIG-DELTA` خلال rerun Q2. |
| **B2 - Telemetry remediation & guardrails** | `docs/source/nexus_telemetry_remediation_plan.md`, `docs/source/telemetry.md`, `dashboards/alerts/nexus_audit_rules.yml` | @sre-core, @telemetry-ops | مكتمل | تم تسليم alert pack وسياسة diff bot وحجم دفعة OTLP (`nexus.scheduler.headroom` log + لوحة headroom في Grafana)؛ لا توجد waivers مفتوحة. |
| **B3 - Config delta approvals** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`, `defaults/nexus/config.toml`, `defaults/nexus/genesis.json` | @release-eng, @governance | مكتمل | تم تسجيل تصويت GOV-2026-03-19؛ الحزمة الموقعة تغذي telemetry pack المذكور ادناه. |
| **B4 - Multi-lane launch rehearsal** | `docs/source/runbooks/nexus_multilane_rehearsal.md`, `docs/source/project_tracker/nexus_rehearsal_2026q1.md`, `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`, `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`, `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core, @sre-core | مكتمل (Q2 2026) | اغلق rerun الكاناري في Q2 تخفيف فجوة TLS؛ يقوم validator manifest + `.sha256` بالتقاط نطاق slots 912-936 وworkload seed `NEXUS-REH-2026Q2` وhash ملف TLS المسجل في rerun. |

## جدول تدقيق routed-trace ربع السنوي

| Trace ID | النافذة (UTC) | النتيجة | ملاحظات |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ناجح | ظل Queue-admission P95 اقل بكثير من الهدف <=750 ms. لا يلزم اي اجراء. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | ناجح | تم ارفاق OTLP replay hashes بـ `status.md`; اكدت parity الخاصة بـ SDK diff bot عدم وجود drift. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تم الحل | تم اغلاق فجوة TLS خلال rerun Q2؛ يسجل telemetry pack لـ `NEXUS-REH-2026Q2` hash ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (انظر `artifacts/nexus/tls_profile_rollout_2026q2/`) ولا توجد متخلفات. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | ناجح | Workload seed `NEXUS-REH-2026Q2`; telemetry pack + manifest/digest في `artifacts/nexus/rehearsals/2026q1/` (slot range 912-936) مع agenda في `artifacts/nexus/rehearsals/2026q2/`. |

يجب على الفصول القادمة اضافة صفوف جديدة ونقل الادخالات المكتملة الى ملحق عندما يتجاوز الجدول الربع الحالي. ارجع الى هذا القسم من تقارير routed-trace او محاضر الحوكمة باستخدام المرساة `#quarterly-routed-trace-audit-schedule`.

## التخفيفات وعناصر backlog

| العنصر | الوصف | المالك | الهدف | الحالة / الملاحظات |
|------|-------------|-------|--------|----------------|
| `NEXUS-421` | اكمال نشر ملف TLS الذي تاخر خلال `TRACE-CONFIG-DELTA`، التقاط ادلة rerun، واغلاق سجل التخفيف. | @release-eng, @sre-core | نافذة routed-trace Q2 2026 | مغلق - hash ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` ملتقط في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; اكد rerun عدم وجود متخلفات. |
| `TRACE-MULTILANE-CANARY` prep | جدولة تدريب Q2، ارفاق fixtures بحزمة التيليمتري وضمان اعادة استخدام SDK harnesses للمساعد المعتمد. | @telemetry-ops, SDK Program | اجتماع التخطيط 2026-04-30 | مكتمل - agenda محفوظة في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع metadata slot/workload؛ تم توثيق اعادة استخدام harness في tracker. |
| Telemetry pack digest rotation | تشغيل `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل تدريب/Release وتسجيل digests بجانب tracker الخاص بـ config delta. | @telemetry-ops | لكل release candidate | مكتمل - `telemetry_manifest.json` + `.sha256` صدرت في `artifacts/nexus/rehearsals/2026q1/` (slot range `912-936`, seed `NEXUS-REH-2026Q2`); تم نسخ digests الى tracker وفهرس الادلة. |

## دمج حزمة config delta

- يظل `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` الملخص القانوني للفروقات. عند وصول `defaults/nexus/*.toml` جديدة او تغييرات genesis، حدث هذا tracker اولا ثم اعكس ابرز النقاط هنا.
- تغذي signed config bundles حزمة تيليمتري التدريب. يجب نشر الحزمة، التي تم التحقق منها عبر `scripts/telemetry/validate_nexus_telemetry_pack.py`, بجانب ادلة config delta لكي يتمكن المشغلون من اعادة تشغيل نفس artefacts المستخدمة في B4.
- تبقى حزم Iroha 2 بدون lanes: configs مع `nexus.enabled = false` ترفض الان overrides لـ lane/dataspace/routing ما لم يتم تفعيل ملف Nexus (`--sora`)، لذا ازل اقسام `nexus.*` من قوالب single-lane.
- ابق سجل تصويت الحوكمة (GOV-2026-03-19) مرتبطا من tracker ومن هذه الملاحظة لكي تتمكن التصويتات المستقبلية من نسخ التنسيق دون اعادة اكتشاف طقوس الموافقة.

## متابعات تدريب الاطلاق

- يسجل `docs/source/runbooks/nexus_multilane_rehearsal.md` خطة الكاناري وقائمة المشاركين وخطوات rollback؛ حدث runbook عند تغير طوبولوجيا lanes او exporters التيليمتري.
- يسرد `docs/source/project_tracker/nexus_rehearsal_2026q1.md` كل artefact تم التحقق منه في تدريب 9 ابريل ويشمل الان ملاحظات/agenda تحضير Q2. اضف التدريبات المستقبلية الى نفس tracker بدلا من فتح trackers مستقلة للحفاظ على تسلسل الادلة.
- انشر snippets لمجمع OTLP وexports من Grafana (راجع `docs/source/telemetry.md`) عند تغير ارشادات batching للـ exporter؛ تحديث Q1 رفع batch size الى 256 عينة لتجنب تنبيهات headroom.
- ادلة CI/tests الخاصة بـ multi-lane اصبحت في `integration_tests/tests/nexus/multilane_pipeline.rs` وتعمل ضمن workflow `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)، لتحل محل المرجع المتقاعد `pytests/nexus/test_multilane_pipeline.py`; حافظ على hash لـ `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) متزامنا مع tracker عند تحديث bundles التدريب.

## دورة حياة lanes في وقت التشغيل

- خطط دورة حياة lanes في وقت التشغيل تتحقق الان من bindings الخاصة بـ dataspace وتتوقف عند فشل reconciliation لـ Kura/التخزين الطبقي، مع ترك الكتالوج دون تغيير. تقوم helpers بقص relays المخزنة لـ lanes المتقاعدة كي لا يعاد استخدام proofs قديمة في synthese merge-ledger.
- طبق الخطط عبر helpers الخاصة بـ config/lifecycle في Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) لاضافة/سحب lanes دون اعادة تشغيل؛ يعاد تحميل routing وTEU snapshots وmanifest registries تلقائيا بعد نجاح الخطة.
- ارشاد للمشغلين: عند فشل الخطة تحقق من dataspaces المفقودة او storage roots التي لا يمكن انشاؤها (tiered cold root/مجلدات Kura لكل lane). اصلح المسارات الاساسية وحاول مجددا؛ الخطط الناجحة تعيد اصدار diff تيليمتري لـ lane/dataspace كي تعكس dashboards الطوبولوجيا الجديدة.

## تيليمتري NPoS وادلة backpressure

طلبت مراجعة تدريب الاطلاق لمرحلة Phase B التقطات تيليمتري حتمية تثبت ان pacemaker الخاص بـ NPoS وطبقات gossip تبقى ضمن حدود backpressure. يقوم harness التكامل في `integration_tests/tests/sumeragi_npos_performance.rs` بتشغيل هذه السيناريوهات ويصدر ملخصات JSON (`sumeragi_baseline_summary::<scenario>::...`) عند ظهور قياسات جديدة. شغله محليا عبر:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

اضبط `SUMERAGI_NPOS_STRESS_PEERS` و`SUMERAGI_NPOS_STRESS_COLLECTORS_K` او `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات اشد ضغطا؛ القيم الافتراضية تعكس ملف جامعي 1 s/`k=3` المستخدم في B4.

| السيناريو / test | التغطية | تيليمتري اساسية |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | يحجز 12 جولة مع block time الخاص بالتدريب لتسجيل envelopes للـ EMA latency واعماق الطوابير وgauges لـ redundant-send قبل تسلسل bundle الادلة. | `sumeragi_phase_latency_ema_ms`, `sumeragi_collectors_k`, `sumeragi_redundant_send_r`, `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | يغمر طابور المعاملات لضمان تفعيل deferrals للـ admission بشكل حتمي وتصدير العدادين للقدرة/التشبع. | `sumeragi_tx_queue_depth`, `sumeragi_tx_queue_capacity`, `sumeragi_tx_queue_saturated`, `sumeragi_pacemaker_backpressure_deferrals_total`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | يلتقط jitter للـ pacemaker وtimeouts للـ view حتى يثبت تطبيق نطاق +/-125 permille. | `sumeragi_pacemaker_jitter_ms`, `sumeragi_pacemaker_view_timeout_target_ms`, `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | يدفع payloads RBC كبيرة حتى حدود soft/hard للـ store ليظهر ارتفاع جلسات وعدادات bytes ثم تراجعها واستقرارها دون تجاوز store. | `sumeragi_rbc_store_pressure`, `sumeragi_rbc_store_sessions`, `sumeragi_rbc_store_bytes`, `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | يفرض اعادة ارسال حتى تتقدم gauges نسبة redundant-send وعدادات collectors-on-target، مؤكدا ان التيليمتري المطلوبة مرتبطة end-to-end. | `sumeragi_collectors_targeted_current`, `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | يسقط chunks بفواصل حتمية للتحقق من ان مراقبي backlog يطلقون faults بدلا من تفريغ payloads بصمت. | `sumeragi_rbc_backlog_sessions_pending`, `sumeragi_rbc_backlog_chunks_total`, `sumeragi_rbc_backlog_chunks_max`. |

ارفق اسطر JSON التي يطبعها harness مع scrape الخاص بـ Prometheus الملتقط اثناء التشغيل كلما طلبت الحوكمة ادلة على ان انذارات backpressure تطابق طوبولوجيا التدريب.

## قائمة التحديث

1. اضف نوافذ routed-trace جديدة وانقل القديمة عند تغير الفصول.
2. حدث جدول التخفيف بعد كل متابعة Alertmanager حتى لو كان الاجراء هو اغلاق التذكرة.
3. عندما تتغير config deltas، حدث tracker وهذه الملاحظة وقائمة digests الخاصة بالـ telemetry pack في نفس pull request.
4. اربط هنا اي artefact جديد للتدريب/التيليمتري كي تشير تحديثات roadmap المستقبلية الى مستند واحد بدلا من ملاحظات ad-hoc متناثرة.

## فهرس الادلة

| الاصل | الموقع | ملاحظات |
|-------|----------|-------|
| تقرير تدقيق routed-trace (Q1 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المصدر القانوني لادلة Phase B1؛ يتم نسخه للبوابة تحت `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker الخاص بـ config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | يحتوي على ملخصات TRACE-CONFIG-DELTA وتواقيع المراجعين وسجل تصويت GOV-2026-03-19. |
| خطة remediation للتيليمتري | `docs/source/nexus_telemetry_remediation_plan.md` | توثق alert pack وحجم دفعة OTLP وguardrails ميزانية التصدير المرتبطة بـ B2. |
| Tracker تدريب multi-lane | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | يسرد artefacts تدريب 9 ابريل وmanifest/digest الخاص بالـ validator وملاحظات/agenda Q2 وادلة rollback. |
| Telemetry pack manifest/digest (latest) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | يسجل slot range 912-936 وseed `NEXUS-REH-2026Q2` وhashes artefacts لحزم الحوكمة. |
| TLS profile manifest | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | hash ملف TLS المعتمد الملتقط خلال rerun Q2؛ اشر اليه في ملاحق routed-trace. |
| TRACE-MULTILANE-CANARY agenda | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | ملاحظات التخطيط لتدريب Q2 (النافذة، slot range، workload seed، مالكو الاجراءات). |
| Runbook تدريب الاطلاق | `docs/source/runbooks/nexus_multilane_rehearsal.md` | Checklist تشغيلية لـ staging -> execution -> rollback؛ حدثها عند تغير طوبولوجيا lanes او ارشادات exporters. |
| Telemetry pack validator | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI مشار اليه في retro B4؛ ارشف digests بجانب tracker عند تغير الحزمة. |
| Multilane regression | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | يثبت `nexus.enabled = true` لconfigs multi-lane، ويحافظ على Sora catalog hashes ويهيئ مسارات Kura/merge-log لكل lane (`blocks/lane_{id:03}_{slug}`) عبر `ConfigLaneRouter` قبل نشر digests الخاصة بالartefacts. |
