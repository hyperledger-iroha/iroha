---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/transition-notes.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ملاحظات انتقال العلاقة
العنوان: نصائح انتقال Nexus
الوصف: نسخة مطابقة لـ `docs/source/nexus_transition_notes.md`، بما في ذلك يعادل انتقال المرحلة B وجدول التدقيق والخطط لعدم ذلك.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# أفكار انتقال Nexus

يتتبع هذا السجل العمل أخيرا من **Phase B - Nexus Transition Foundations** حتى اكتملت قائمة فحص الـ multi-lane. وهو يكمل بنود بارز في `roadmap.md` ويحفظ المعادلة المشار إليها في B1-B4 في مكان واحد حتى يبدأ فرق ال تور وSRE وقادة SDK من مشاركة نفس مصدر الحقيقة.

## النطاق والوتيرة

- تغطية تدقيقات router-trace وحواجز الحماية للتيليمتري (B1/B2)، دلتا للتكوين المعتمد من الـتور (B3)، ومتابعات الاطلاق متعدد الممرات (B4).
- يستبدل ملاحظة الوتيرة المؤقتة التي كانت هنا؛ منذ تدقيق Q1 2026 يوجد التقرير المفصلي في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، بينما بقيت هذه الصفحة بالجدول التشغيلي ولم لا.
- حدث بعد كل تطبيقاتنا router-trace او اشتراك او تدريب. عندما تقوم بتصنيع المصنوعات اليدوية، عكس الموقع الجديد داخل هذه الصفحة كي الزراعية الوثائق التالية (الحالة، لوحات المعلومات، بوابات SDK) من الارتباط بمرساة الالتزام.

## لقطة بديلة (2026، الربع الأول - الربع الثاني)

| مسار العمل | العادله | الملاك | الحالة | تعليقات |
|------------|----------|----------|--------|-------|
| **B1 - عمليات تدقيق التتبع الموجهة** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`، `docs/examples/nexus_audit_outcomes/` | @telemetry-ops، @governance | مكتمل (الربع الأول 2026) | تم تسجيل ثلاث نافذة تدقيق؛ تم تصفية TLS في `TRACE-CONFIG-DELTA` خلال إعادة التشغيل في الربع الثاني. |
| **B2 - معالجة القياس عن بعد وحواجز الحماية** | `docs/source/nexus_telemetry_remediation_plan.md`، `docs/source/telemetry.md`، `dashboards/alerts/nexus_audit_rules.yml` | @sre-core، @telemetry-ops | مكتمل | تم تسليم حزمة التنبيه وسياسة diff bot وحجم دفعة OTLP (سجل `nexus.scheduler.headroom` + مساحة رأس اللوحة في Grafana)؛ لا توجد تنازلات مفتوحة. |
| **B3 - موافقات دلتا التكوين** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`، `defaults/nexus/config.toml`، `defaults/nexus/genesis.json` | @release-eng, @governance | مكتمل | تم تسجيل تصويت GOV-2026-03-19؛ حزمة الموقع تغذي telemetry pack وشكرا ادناه. |
| **B4 - بروفة الإطلاق متعدد المسارات** | `docs/source/runbooks/nexus_multilane_rehearsal.md`، `docs/source/project_tracker/nexus_rehearsal_2026q1.md`، `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`، `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`، `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core، @sre-core | مكتمل (الربع الثاني 2026) | اغلق إعادة تشغيل الكناري في الربع الثاني لاعتباره TLS؛ يقوم بإثبات صحة البيانات + `.sha256` بقاطع نطاق الفتحات 912-936 وworkloadseed `NEXUS-REH-2026Q2` وhash ملف TLS المسجل في إعادة التشغيل. |

## جدول تدقيق router-trace ربع للمنزل

| معرف التتبع | نافذة (التوقيت العالمي المنسق) | النتيجة | تعليقات |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ناجح | ظل قائمة الانتظار P95 أقل بكثير من الهدف <=750 مللي ثانية. لا تأخذ اي اجراء. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | ناجح | تم اصطياد OTLP replay hashes بـ `status.md`; تعلمت التكافؤ الخاص بـ SDK diff bot عدم وجود الانجراف. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | تم الحل | تم إغلاق TLS خلال إعادة تشغيل الربع الثاني؛ سجل حزمة القياس عن بعد لـ `NEXUS-REH-2026Q2` hash file TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (انظر `artifacts/nexus/tls_profile_rollout_2026q2/`) ولا يوجد متخلفات. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | ناجح | بذرة عبء العمل `NEXUS-REH-2026Q2`؛ حزمة القياس عن بعد + البيان/الملخص في `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة 912-936) مع جدول الأعمال في `artifacts/nexus/rehearsals/2026q2/`. |

يجب على الفصول القادمة إضافة نماذج جديدة متوقعة الادخالات المكتملة الى اضافة عندما يتجاوز الجدول الكيان الحالي. ارجع الى هذا القسم من تقارير router-trace او محاضر ال تور باستخدام المرساة `#quarterly-routed-trace-audit-schedule`.

## بسببات وعناصر backlog

| حرق | الوصف | المالك | الهدف | الحالة / مذكرة |
|------|-------------|--------|----------------|
| `NEXUS-421` | أكمل نشر ملف TLS الذي تاخر خلال `TRACE-CONFIG-DELTA`، التقاط يعادل rerun، وعدم تسجيل اسمه. | @release-eng، @sre-core | نافذة تتبع التوجيه Q2 2026 | مغلق - ملف التجزئة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` مختار في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; طالما إعادة التشغيل عدم وجود تخليق. |
| `TRACE-MULTILANE-CANARY` الإعدادية | جدولة تدريب Q2، اصحاب التركيبات بزمة التيليمتري باستثناء إعادة استخدام أدوات SDK للمساعد المختص. | @telemetry-ops، برنامج SDK | الاجتماع الاجتماعي 2026-04-30 | مكتمل - جدول الأعمال محفوظ في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع فتحة بيانات التعريف/عبء العمل؛ تم تعزيز عملية إعادة استخدام تسخير في Tracker. |
| حزمة القياس عن بعد هضم التناوب | تشغيل `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل تدريب/إصدار وتسجيل الملخصات بجانب Tracker الخاص بـ config delta. | @القياس عن بعد | لكل مرشح الافراج | مكتمل - `telemetry_manifest.json` + `.sha256`ت صدرت في `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة `912-936`، البذرة `NEXUS-REH-2026Q2`)؛ تم نسخة هضم الى تعقب وفهرس المعادلة. |

##دمج حزمة config delta

- يختفي `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` الملخص للفروقات. عند وصول `defaults/nexus/*.toml` جديدة او ابتكارات Genesis، حدث هذا Tracker اولا ثم عكس ابرز النقاط هنا.
- تغذي حزم التكوين الموقعة حزمة تيليمتري التدريب. يجب نشر الحزمة، التي تم التحقق منها عبر `scripts/telemetry/validate_nexus_telemetry_pack.py`، معادلة config delta للبدء في إعادة تشغيل نفس المصنوعات اليدوية المستخدمة في B4.
- تبقى حزم Iroha 2 بدون ممرات: configs مع `nexus.enabled = false` ترفض الان تجاوزات لـ lane/dataspace/routing ما لم يتم تفعيل ملف Nexus (`--sora`)، لذا ازل اقسام `nexus.*` من قوالب أحادية المسار.
- ابق سجل تصويت التصويت (GOV-2026-03-19) مشاركا من Tracker ومن هذه المزايا للتصويتات المستقبلية من النسخة بدون إعادة ابتكار الابتكارات.

## متابعات تدريب الاطلاق

- تسجيل `docs/source/runbooks/nexus_multilane_rehearsal.md` خطة الكناري وقائمة المشاركين وخطوات التراجع؛ حدث runbook عند تغير طوبولوجيا الممرات او المصدرين التيليمتري.
- يسرد `docs/source/project_tracker/nexus_rehearsal_2026q1.md` كل قطعة أثرية تم التحقق منه في تدريب 9 متخصص في إعداد البيانات/جدول الأعمال Q2. اضف تدريبات المستقبل الى نفس المقتفي بدلات من فتحية المقتفي تباعا لمسلسل تسلسل المعادلة.
- نشر مقتطفات لمجمع OTLP وexports من Grafana (راجع `docs/source/telemetry.md`) عند تغير اتجاهات طرق الخلط للـexporter؛ تحديث Q1 رفع حجم الدفعة إلى 256 تحذيرًا جديدًا من تنبيهات الإرتفاع.
- بديل CI/tests الخاصة بـ multi-lane وبدأ في `integration_tests/tests/nexus/multilane_pipeline.rs` ضمن سير العمل `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)، لتحل محل المرجع المتقاعد `pytests/nexus/test_multilane_pipeline.py`; حافظ على التجزئة لـ `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) متزامنا مع Tracker عند تحديث حزم التدريب.

## دورة حياة الممرات في وقت التشغيل

-خططت لبدء حياة الممرات في وقت التشغيل لتتحقق الان من الارتباطات الخاصة بـ dataspace وتوقف عند فشل المصالحة لـ Kura/التخزين آشبي، مع ترك الكتالوج دون تغيير. تقوم بتشغيل المساعدين بقص المرحلات المخزنة للممرات المتقاعدة كي لا يعاد استخدام البراهين القديمة في دفتر الأستاذ المركب.
- أعلن عبر المساعدين الخاصين بـ config/lifecycle في Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) تمت إضافة/سحب الممرات دون إعادة تشغيل؛ يعاد تحميل التوجيه وTEU snapshots والسجلات الواضحة تلقائيا بعد نجاح البناء.
- الطريق للمشغلين: عند الفشل في البناء تحقق من مساحات البيانات س او جذور التخزين التي لا يمكن انشاؤها (tiered Cold root/مجلدات كورا لكل حارة). اصلح المسار الرئيسي ثم غاب؛ وقد نجحت في الاشتراك في diff تيليمتري لـ Lane/dataspace كي تعتبر لوحات المعلومات فلسفة الفلسفة الجديدة.

## تيليمتري NPoS وبديل الضغط الخلفي

طلبت مراجعة تدريب الاطلاق لمرحلة المرحلة ب التقطات تيليمتري حتمية تؤكد ان جهاز تنظيم ضربات القلب الخاص بـ NPoS وطبقات ثرثرة تستمر ضمن حدود الضغط الخلفي. ويقوم تسخير تكامل في `integration_tests/tests/sumeragi_npos_performance.rs` ويسمح لهذه السيناريوهات ويصدر ملخصات JSON (`sumeragi_baseline_summary::<scenario>::...`) عند ظهور قياسات جديدة. الوظيفة المحلية عبر:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

اضبط `SUMERAGI_NPOS_STRESS_PEERS` و`SUMERAGI_NPOS_STRESS_COLLECTORS_K` او `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات أشد ضغطا؛ القيم احتراما لقبول قبول 1 s/`k=3` المستخدم في B4.

| السيناريو / اختبار | تغطية | تيليمتري أساسية |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | يحجز 12 جولة مع كتلة زمنية خاصة بالتدريب لأظرف للـ EMA زمن الاستجابة وأعماق الطوابير ومقاييس لـ متكررة-إرسال قبل سلسلة السلسلة المعادلة. | `sumeragi_phase_latency_ema_ms`، `sumeragi_collectors_k`، `sumeragi_redundant_send_r`، `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | يغمر طابور المعاملات دائما تفعيل التأجيلات للقبول بشكل حتمي وتصدير العدادين للقدرة/التشبع. | `sumeragi_tx_queue_depth`، `sumeragi_tx_queue_capacity`، `sumeragi_tx_queue_saturated`، `sumeragi_pacemaker_backpressure_deferrals_total`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | يلتقط jitter للـ جهاز تنظيم ضربات القلب وtimeouts للـ عرض حتى يثبت تطبيق نطاق +/-125 permille. | `sumeragi_pacemaker_jitter_ms`، `sumeragi_pacemaker_view_timeout_target_ms`، `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | تدفع حمولات RBC كبيرة حتى حدود لينة/صعبة للتخزين ليظهر جلسات عمل وعددات بايت ثم تداخلها واستقرارها دون تجاوز المتجر. | `sumeragi_rbc_store_pressure`، `sumeragi_rbc_store_sessions`، `sumeragi_rbc_store_bytes`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | يفترض إعادة إرسالها حتى تتقدم المقاييس بنسبة الإرسال المتكرر وعدادات المجمعين على الهدف، نهائيا لانليمتري الأساسية علاقة من النهاية إلى النهاية. | `sumeragi_collectors_targeted_current`، `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | وبالتالي فإن القطع بفواصل حتمية متعددة من ان يراقبي تراكم التراكمات يطلقون الاخطاء بدائل من تفريغ الحمولات بصمت. | `sumeragi_rbc_backlog_sessions_pending`، `sumeragi_rbc_backlog_chunks_total`، `sumeragi_rbc_backlog_chunks_max`. |

ارفق اسطر JSON التي يطبعها تسخير مع كشط الخاص بـ Prometheus المختار أثناء التشغيل يطلب ال ال تعادل على ان انذارات الضغط الخلفي تطابق طوبولوجيا التدريب.

## قائمة التحديث

1. اضف نوافذ router-trace جديدة وانقل الأحداث القديمة عند الفصول.
2. حدث جدول متابعة بعد كل متابعة Alertmanager حتى لو كان الاجراء هو انتهاء التذكرة.
3. عند استخدام config deltas، حدث تعقب الخبرة والمزايا وقائمة الملخصات الخاصة بالـ telemetry pack في نفس طلب السحب.
4. ربط هنا اي artefact جديد المتدربين/التيليمتري كي تشير تحديثات خريطة الطريق المستقبلية الى وثائق واحدة بدلات من توقعات ad-hoc م تتوصلة.

## فهرس المعادلة| الاصل | الموقع | تعليقات |
|-------|----------|-------|
| تقرير تدقيق التتبع التوجيهي (الربع الأول من عام 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المصدر الساقي لمعادلة المرحلة B1؛ يتم إصدار النسخة للبوابة تحت `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| Tracker الخاص بـ config delta | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | يحتوي على ملخصات TRACE-CONFIG-DELTA وتاقيع المراجعين وسجل 8 GOV-2026-03-19. |
| خطة علاجية للتليمتري | `docs/source/nexus_telemetry_remediation_plan.md` | حزمة التنبيهات الجديرة بالثقة والتي ستعزز OTLP وguardrails لآثار تأجيلها بـ B2. |
| تدريب تعقب متعدد المسارات | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | يسرد المصنوعات اليدوية تدريب 9 شهير وmanifest/digest الخاص بالـ validator وملاحظات/جدول أعمال Q2 وبديل rollback. |
| بيان/ملخص حزمة القياس عن بعد (الأحدث) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | سجل نطاق الفتحة 912-936 والبذور `NEXUS-REH-2026Q2` وhashes artifacts لحزم ال تور. |
| بيان ملف تعريف TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | ملف تجزئة TLS مؤهل للمختار خلال إعادة تشغيل Q2؛ اشر اليه في ملاحقة التتبع التوجيهي. |
| جدول أعمال TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | خطط تخطيط لتدريب Q2 (النافذة، نطاق الفتحة، بذرة عبء العمل، مالكو شهري). |
| Runbook تدريب الاطلاق | `docs/source/runbooks/nexus_multilane_rehearsal.md` | قائمة التحقق من تشغيل التدريج -> التنفيذ -> التراجع؛ حدثها عند تغير طوبولوجيا الممرات او ارشادات المصدرين. |
| مدقق حزمة القياس عن بعد | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI مشار إليه في retro B4؛ اشف هضم Bex Tracker عند تغير اللعبة. |
| الانحدار متعدد المسارات | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | يثبت `nexus.enabled = true` لتكوينات multi-lane، ويحافظ على تجزئة كتالوج Sora ويهيئ مسارات Kura/merge-log لكل حارة (`blocks/lane_{id:03}_{slug}`) عبر `ConfigLaneRouter` قبل نشر الملخصات الخاصة بالمصنوعات اليدوية. |