---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/transition-notes.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ملاحظات انتقال العلاقة
العنوان: Заметки о перадоде Nexus
الوصف: Zercalo `docs/source/nexus_transition_notes.md`، مما يزيد من تأكيد المرحلة B، تدقيق الرسومات والتخفيف.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# إضافات Nexus

هذا السجل يتتبع العمل النهائي **المرحلة ب - Nexus أسس الانتقال** إلى قائمة المراجعة النهائية متعددة المسارات. يتم إضافة معلم رئيسي إلى `roadmap.md` ويؤدي إلى الإحالة، التي تتوافق مع B1-B4، في مكان واحد، للحوكمة، SRE، وأدوات تطوير البرامج (SDK) يمكن أن تستمتع بواحدة من المقالات التاريخية.

## السرعة والإيقاع

- الكشف عن عمليات فحص التتبع الموجهة وحواجز الحماية للقياس عن بعد (B1/B2)، ومجموعة دلتا التكوين، والحوكمة الجيدة (B3)، والمتابعة من خلال بروفة الإطلاق متعدد المسارات (B4).
- مذكرة إيقاعية مؤقتة، والتي تم إصدارها بعد مرور عام؛ مع التدقيق في الربع الأول من عام 2026، تم إدخال آخر جيد في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، وهو جزء من الرسم البياني العملي والتخفيف من آثاره.
- قم بإسقاط اللوحات بعد كل مرة يتم فيها توجيه التتبع أو إدارة المتابعة أو إطلاق البروفة. عندما تقوم العناصر بإعادة التدوير، قم بإلغاء توجيه جديد من هناك، بحيث يمكن للمستندات النهائية (الحالة، ولوحات المعلومات، وبوابات SDK) أن تتحول إلى استقرار مستقر.

## لقطة شاشة (2026 Q1-Q2)

| بوتوك | التصريح | المالك (المالكون) | الحالة | مساعدة |
|------------|----------|----------|--------|-------|
| **B1 - عمليات تدقيق التتبع الموجهة** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`، `docs/examples/nexus_audit_outcomes/` | @telemetry-ops، @governance | جديد (الربع الأول من عام 2026) | التحقق من صحة ثلاث مرات; تم إلغاء TLS lag `TRACE-CONFIG-DELTA` في إعادة التشغيل في الربع الثاني. |
| **B2 - معالجة القياس عن بعد وحواجز الحماية** | `docs/source/nexus_telemetry_remediation_plan.md`، `docs/source/telemetry.md`، `dashboards/alerts/nexus_audit_rules.yml` | @sre-core، @telemetry-ops | جدا | يتم إرسال حزمة التنبيه وروبوت السياسة المختلفة وحجم دفعة OTLP (سجل `nexus.scheduler.headroom` + مساحة رأس اللوحة Grafana) ؛ صافي التنازل المفتوح. |
| **B3 - موافقات دلتا التكوين** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`، `defaults/nexus/config.toml`، `defaults/nexus/genesis.json` | @release-eng, @governance | جدا | GOLOSOVANIE GOV-2026-03-19 التأمين; حزمة القياس عن بعد صغيرة جدًا. |
| **B4 - بروفة الإطلاق متعدد المسارات** | `docs/source/runbooks/nexus_multilane_rehearsal.md`، `docs/source/project_tracker/nexus_rehearsal_2026q1.md`، `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`، `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`، `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core، @sre-core | أخيرا (الربع الثاني من عام 2026) | إعادة تشغيل الكناري Q2 للتخفيف من ضغط TLS; بيان التحقق من الصحة + `.sha256` إصلاح الفتحة 912-936 وبذور عبء العمل `NEXUS-REH-2026Q2` وملف تعريف التجزئة TLS من إعادة التشغيل. |

## تدقيق الرسومات الجرافيكية الموجهة

| معرف التتبع | أوكنو (التوقيت العالمي المنسق) | النتيجة | مساعدة |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | متوفر | تم إلغاء قبول قائمة الانتظار P95 بشكل واضح <= 750 مللي ثانية. لا داعي للقلق. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | متوفر | تم تطبيق إعادة تشغيل OTLP للتجزئات من `status.md`; أدى التكافؤ SDK diff bot إلى تعزيز الانجراف الجديد. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | قرار | تأخر TLS في إعادة التشغيل في الربع الثاني؛ تعمل حزمة القياس عن بعد لـ `NEXUS-REH-2026Q2` على إصلاح ملف تعريف التجزئة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (باسم `artifacts/nexus/tls_profile_rollout_2026q2/`) ولا يوجد أي شيء آخر. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | متوفر | بذرة عبء العمل `NEXUS-REH-2026Q2`؛ حزمة القياس عن بعد + البيان/الملخص في `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة 912-936) مع جدول الأعمال في `artifacts/nexus/rehearsals/2026q2/`. |

تعمل الوحدات السكنية على إضافة نقرات جديدة وإعادة الكتابة النهائية في التطبيق عندما يتم إعادة لصق اللوحة غرفة. يمكنك الاختيار من خلال التتبع الموجه أو دقائق الإدارة عبر `#quarterly-routed-trace-audit-schedule`.

## التخفيف والتراكم

| العنصر | الوصف | المالك | Цель | الحالة / Пrimечания |
|------|-------------|--------|----------------|
| `NEXUS-421` | قم بتوسيع ملف تعريف TLS الذي تم طرحه في `TRACE-CONFIG-DELTA`، وقم بإعادة تشغيل الأدلة وإلغاء تخفيف اليومية. | @release-eng، @sre-core | الربع الثاني من عام 2026 - التتبع التوجيهي أوكنو | الأصل - ملف تعريف تجزئة TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` zafiksirovan в `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`; أعد تشغيل النتيجة النهائية. |
| `TRACE-MULTILANE-CANARY` الإعدادية | قم بتخطيط بروفة الربع الثاني، واستخدم التركيبات في حزمة القياس عن بعد، وتأكد من استخدام SDK للمساعد الصالح. | @telemetry-ops، برنامج SDK | مخطط 2026-04-30 | أعلى - جدول الأعمال موجود في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع فتحة بيانات التعريف/عبء العمل؛ إعادة استخدام تسخير отмечен في تعقب. |
| حزمة القياس عن بعد هضم التناوب | قم بتشغيل `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل بروفة/إصدار وقم بإصلاح الملخصات باستخدام Tracker config delta. | @القياس عن بعد | На кадый إطلاق سراح المرشح | أعلى - `telemetry_manifest.json` + `.sha256` تم إرجاعه إلى `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة `912-936`، البذور `NEXUS-REH-2026Q2`)؛ ملخصات التنقيب في المتعقب والفهرس للمستندات. |

## تكامل حزمة دلتا التكوين

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` يلخص الفرق الكنسي. عندما تحصل على `defaults/nexus/*.toml` جديد أو تحديث Genesis، قم ببدء تشغيل Tracker، ثم قم بإسقاط النقاط الرئيسية هنا.
- حزم التكوين المرفقة تتضمن حزمة القياس عن بعد للبروفة. حزمة صالحة `scripts/telemetry/validate_nexus_telemetry_pack.py`، تم نشرها بالكامل عبر دلتا التكوين المتوافقة مع المشغلين الذين يمكنهم التحدث بشكل جيد المصنوعات اليدوية المستخدمة في B4.
- الحزم Iroha 2 يتم استبعادها من الممرات: التكوينات باستخدام `nexus.enabled = false` تقوم بإلغاء حجب المسار/مساحة البيانات/التوجيه، إذا لم يتم تضمين ملف تعريف Nexus (`--sora`)، هذا هو القسم `nexus.*` من أعمدة ذات مسار واحد.
- ديرجيت سجل إدارة الحوكمة (GOV-2026-03-19) مرتبط ومتتبع، ومعه هذا الإذن الذي يمكن من خلاله نسخ التنسيق بدون طقوس إعادة التدوير.

## المتابعات قبل بروفة الإطلاق

- `docs/source/runbooks/nexus_multilane_rehearsal.md` إصلاح خطة الكناري وقائمة المكافآت والتراجع؛ قم بتحديث دليل التشغيل من خلال تحسين طبولوجيا الممرات أو أجهزة القياس عن بعد الخاصة بالمصدر.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` يستعرض جميع المصنوعات، ويختبر التكرارات في 9 أبريل، ويتضمن ذلك الملاحظات/جدول الأعمال الإعدادية للربع الثاني. أضف المزيد من التكرار إلى المتتبع الذي يعد متعقبًا فريدًا مما يجعله رتيبًا.
- نشر مقتطفات أداة تجميع OTLP وصادرات Grafana (بحجم `docs/source/telemetry.md`) من خلال تصدير إرشادات التجميع؛ زيادة حجم الدُفعة في Q1 إلى 256 عينة لتحفيز تنبيهات الإرتفاع.
- توصيل CI/اختبارات الحياة الحرارية متعددة المسارات في `integration_tests/tests/nexus/multilane_pipeline.rs` والبدء من خلال سير العمل `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)، محمي سخية ссылку `pytests/nexus/test_multilane_pipeline.py`; قم بالمزامنة مع التجزئة `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) مع المتعقب من خلال حزم التدريب.

## دورة حياة حارة وقت التشغيل

- خطط دورة حياة حارة وقت التشغيل للتحقق من صحة ارتباطات مساحة البيانات والتحويل من خلال تسوية Kura/التخزين المتدرج، وإلغاء الكتالوج دون تغيير. يقوم المساعدون بمراقبة مرحلات الممرات المقفلة للممرات المتقاعدة بحيث لا يؤدي دمج دفتر الأستاذ إلى استخدام البراهين.
- استخدم الخطط من خلال مساعدي التكوين/دورة الحياة Nexus (`State::apply_lane_lifecycle`، `Queue::apply_lane_lifecycle`) لتوصيل/إعادة تشغيل الممرات دون إعادة التشغيل؛ يتم إعادة تسجيل التوجيه ولقطات TEU وسجلات البيانات تلقائيًا بعد خطة ناجحة.
- مشغل الشبكة: من خلال خطتك، تحقق من اكتشاف مساحات البيانات أو جذور التخزين التي لا ترغب في إنشائها (أدلة الجذر البارد/أدلة حارة كورا). قم بتنفيذ الطرق والطرق الأساسية; تقوم الخطط الناجحة مرة أخرى بحذف حارة/مساحة البيانات للقياس عن بعد، مما يؤدي إلى ظهور لوحات المعلومات في طوبولوجيا جديدة.

## القياس عن بعد وقياس الضغط الخلفي NPoS

تم إطلاق بروفة المرحلة الرجعية B من خلال التقاط لقطات للقياس عن بعد، مما يدل على أن جهاز تنظيم ضربات القلب NPoS وقنوات القيل والقال تسبق الضغط الخلفي. يعمل حزام التكامل في `integration_tests/tests/sumeragi_npos_performance.rs` على تمديد هذه السيناريوهات وعرض ملخصات JSON (`sumeragi_baseline_summary::<scenario>::...`) باستخدام مقياس جديد. الاتصال محليًا:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

قم بتثبيت `SUMERAGI_NPOS_STRESS_PEERS` أو `SUMERAGI_NPOS_STRESS_COLLECTORS_K` أو `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لإخراج طبولوجيا الضغط الأعلى؛ بداية من الملف الشخصي 1 s/`k=3`، مستخدم في B4.| السيناريو / الاختبار | عملية | مفاتيح القياس عن بعد |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | قم بحظر 12 جولة مع وقت كتلة التدريب لتدوين مظاريف زمن استجابة EMA والمراقبة اللامعة وأجهزة قياس الإرسال الزائدة قبل تسلسل حزمة الأدلة. | `sumeragi_phase_latency_ema_ms`، `sumeragi_collectors_k`، `sumeragi_redundant_send_r`، `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | قم بمتابعة المعاملات لضمان تحديد تأجيلات القبول وسعة/تشبع الصادرات. | `sumeragi_tx_queue_depth`، `sumeragi_tx_queue_capacity`، `sumeragi_tx_queue_saturated`، `sumeragi_pacemaker_backpressure_deferrals_total`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | قم بتضمين ارتعاش جهاز تنظيم ضربات القلب وعرض المهلات، دون الحاجة إلى إضافة نقاط مراقبة +/-125 في الدقيقة. | `sumeragi_pacemaker_jitter_ms`، `sumeragi_pacemaker_view_timeout_target_ms`، `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | قم بحماية مجموعة حمولات RBC من متجر محدود النطاق، لإظهار الثبات والحذف وتحقيق الاستقرار في ذاكرة الجلسة/البايت بدون مخزن التحميل. | `sumeragi_rbc_store_pressure`، `sumeragi_rbc_store_sessions`، `sumeragi_rbc_store_bytes`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | نطلب إعادة الإرسال لقياس نسبة الإرسال المتكرر وعدادات المجمعين على الهدف، مما يوفر أجهزة قياس عن بعد من النهاية إلى النهاية. | `sumeragi_collectors_targeted_current`، `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | قم بتوزيع القطع بشكل محدد لكي تقوم أجهزة المراقبة المتراكمة بإخفاء الأخطاء في جميع الحمولات النافعة. | `sumeragi_rbc_backlog_sessions_pending`، `sumeragi_rbc_backlog_chunks_total`، `sumeragi_rbc_backlog_chunks_max`. |

استخدم خطوط JSON التي يتم تشغيلها من خلال Prometheus Scrape، وهي محمية خلال فترة الإيقاف، في أي وقت بمجرد بدء الإدارة يشير إلى أن أجهزة إنذار الضغط الخلفي تدعم طبولوجيا البروفة.

## اختيار القائمة

1. قم بإضافة التتبع التوجيهي الجديد على الفور، ثم قم بإعادة التتبع من خلال مجرد مربعات صغيرة.
2. قم بإدراج لوحة التخفيف بعد كل متابعة Alertmanager، حتى لو كان الأمر كذلك - قم بسداد التذكرة.
3. عند ظهور دلتا التكوين، قم بترقية المتعقب، وهذا هو العنوان والقائمة التي تهضم حزمة القياس عن بعد في طلب سحب واحد.
4. قم بالبدء في بروفة جديدة/أدوات القياس عن بعد لتتمكن من إنشاء خريطة طريق جديدة على مستند واحد، وليس مختلفًا قروض مخصصة.

## مؤشر الإحالة

| نشط | الموقع | مساعدة |
|-------|----------|-------|
| تقرير تدقيق التتبع التوجيهي (الربع الأول من عام 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المعيار الكنسي للتوثيق المرحلة B1؛ يتم التحقق من خلال البوابة عبر `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| تكوين تعقب دلتا | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | قم بالاشتراك في ملخصات TRACE-CONFIG-DELTA المختلفة والمراجعين الأوليين وتسجيل الدخول GOV-2026-03-19. |
| خطة علاج القياس عن بعد | `docs/source/nexus_telemetry_remediation_plan.md` | قم بتوثيق حزمة التنبيه وحجم دفعة OTLP وحواجز حماية ميزانية التصدير، المرتبطة بـ B2. |
| متعدد المسارات بروفة تعقب | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | قائمة التكرارات المصنوعة في 9 أبريل، بيان/ملخص المدقق، ملاحظات/جدول أعمال Q2 وأدلة التراجع. |
| بيان/ملخص حزمة القياس عن بعد (الأحدث) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | مجموعة الفتحات الثابتة 912-936، والبذور `NEXUS-REH-2026Q2`، وحزم الحوكمة المصنوعة من التجزئة. |
| بيان ملف تعريف TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | تم إنشاء ملف تعريف TLS مميز، تم إغلاقه في إعادة التشغيل في الربع الثاني؛ يمكنك الوصول إلى ملاحق التتبع الموجهة. |
| جدول أعمال TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | خطط لبروفة الربع الثاني (النافذة، نطاق الفتحة، بذرة عبء العمل، أصحاب الإجراء). |
| إطلاق دليل التدريب | `docs/source/runbooks/nexus_multilane_rehearsal.md` | مرحلة فحص العمليات -> التنفيذ -> التراجع؛ قم بالتحديث عند تغيير طبولوجيا الممرات أو مصدري التوجيه. |
| مدقق حزمة القياس عن بعد | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI، المعزز في B4 rettro؛ يمكنك أرشفة الملخصات مباشرة باستخدام أداة التعقب ضمن أي حزمة تعديل. |
| الانحدار متعدد المسارات | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | تحقق من `nexus.enabled = true` للتكوينات متعددة المسارات، وقم بدعم تجزئات كتالوج Sora وتمكين مسارات Kura/merge-log للحارة المحلية (`blocks/lane_{id:03}_{slug}`) من خلال `ConfigLaneRouter` نشر ملخصات المصنوعات اليدوية. |