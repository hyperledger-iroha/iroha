---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/transition-notes.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ملاحظات انتقال العلاقة
العنوان: Nexus ٹرانزیشن نوتٹس
الوصف: `docs/source/nexus_transition_notes.md` هو هذا، وهو المتجر الفرنسي للمرحلة ب، والشيول، والأدوات اليدوية.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

#Nexus ٹرانزیشن نوتٹس

هذه ليست **المرحلة ب - Nexus أسس الانتقال** لم يتم الانتهاء من هذه الخطوة بعد الآن. يحتوي `roadmap.md` على معالم رئيسية في استكمال البطاقة وB1-B4 توفر دليلًا عالميًا شاملاً للحوكمة وSRE وSDK مصدرًا للحقيقة. كر سكي.

## اسكوپ والإيقاع

- نظام التتبع الموجه وحواجز الحماية للقياس عن بعد (B1/B2)، ونظام التحكم في التحكم، وتكوين دلتا سات (B3)، ومتابعة بروفة الإطلاق متعددة المسارات (B4).
- تم عرض هذا الإيقاع الجديد من قبل ليتا؛ الربع الأول من عام 2026، بعد تفاصيل التقرير `docs/source/nexus_routed_trace_audit_report_2026q1.md`، هناك صفحة وصفحة من سجل الشيول والتخفيف.
- بدء التتبع التوجيهي، أو إدارة الحوكمة، أو إطلاق البروفة بعد 20 دقيقة. عندما تنتقل العناصر إلى هذه الصفحة التي تعتمد على تقنية الانعكاس في المستندات النهائية (الحالة، ولوحات المعلومات، وبوابات SDK)، فإن هذا مرساة ثابتة لنكر سكاي.

## لمحة سريعة عن الأدلة (الربع الأول إلى الربع الثاني من عام 2026)

| العمل | ثبوت | ملكان | ٹیٹس | أخبار |
|------------|----------|----------|--------|-------|
| **B1 - عمليات تدقيق التتبع الموجهة** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`، `docs/examples/nexus_audit_outcomes/` | @telemetry-ops، @governance | مكمل (الربع الأول 2026) | تم تجديد هذا التسجيل؛ `TRACE-CONFIG-DELTA` يمكن إعادة تشغيل تأخر TLS في الربع الثاني. |
| **B2 - معالجة القياس عن بعد وحواجز الحماية** | `docs/source/nexus_telemetry_remediation_plan.md`، `docs/source/telemetry.md`، `dashboards/alerts/nexus_audit_rules.yml` | @sre-core، @telemetry-ops | مكمل | حزمة التنبيه، وسياسة الروبوتات المختلفة، وحجم دفعة OTLP (سجل `nexus.scheduler.headroom` + لوحة الارتفاع Grafana) لم يعد هناك أي وورز. |
| **B3 - موافقات دلتا التكوين** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`، `defaults/nexus/config.toml`، `defaults/nexus/genesis.json` | @release-eng, @governance | مكمل | GOV-2026-03-19 تسجيل صوتي؛ الحزمة الموقعة حزمة القياس عن بعد لتغذية كرتا. |
| **B4 - بروفة الإطلاق متعدد المسارات** | `docs/source/runbooks/nexus_multilane_rehearsal.md`، `docs/source/project_tracker/nexus_rehearsal_2026q1.md`، `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`، `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`، `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core، @sre-core | مكمل (الربع الثاني 2026) | إعادة تشغيل كناري Q2 ليس لها تأثير على تخفيف تأخر TLS؛ بيان المدقق + `.sha256` ليس في الفتحات 912-936، بذرة عبء العمل `NEXUS-REH-2026Q2` وإعادة تشغيل تجزئة ملف تعريف TLS للتقاطها. |

## هناك ميزة التتبع التوجيهي

| معرف التتبع | ونڈو (UTC) | نتیجہ | أخبار |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | پاس | قبول قائمة الانتظار P95 ہدف <=750 مللي ثانية سے کافی نیچے رہا۔ لا يوجد أي تصويت. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | پاس | إعادة تشغيل OTLP للتجزئات `status.md` التي تم إجراؤها؛ لا يوجد تكافؤ بين الروبوتات المختلفة لـ SDK أي انحراف صفري. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | حلہ | تم تأجيل إعادة تشغيل TLS lag Q2؛ `NEXUS-REH-2026Q2` تحتوي حزمة القياس عن بعد على تجزئة ملف تعريف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` مسجل و (`artifacts/nexus/tls_profile_rollout_2026q2/`) وصفر متشددون. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | پاس | بذرة عبء العمل `NEXUS-REH-2026Q2`؛ حزمة القياس عن بعد + البيان/الملخص `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة 912-936) وجدول الأعمال `artifacts/nexus/rehearsals/2026q2/`. |

تتضمن هذه القطارات الجديدة ألعابًا جديدة، كما يتم نقل ما هو متاح من إصدار إلى آخر من خلال استكمال مستندات الملحق. يتم استخدام تقرير التتبع الموجه أو محضر الحوكمة بواسطة مرساة `#quarterly-routed-trace-audit-schedule` التي تعد بمثابة ريفرنس كريلز.

## عمليات التخفيف والعناصر المتراكمة

| ئٹم | تفاصيل | مالك | ہدف | ٹیٹس / ملاحظة |
|------|-------------|--------|----------------|
| `NEXUS-421` | `TRACE-CONFIG-DELTA` تم حذف ملف تعريف TLS للنشر وإعادة تشغيل سجل التقاط الأدلة وسجل التخفيف. | @release-eng، @sre-core | Q2 2026 توجيه التتبع ونڈو | بند - تجزئة ملف تعريف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` إلى `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256` يمكن التقاطها؛ أعد التشغيل دون أي صفر من المتطرفين. |
| `TRACE-MULTILANE-CANARY` الإعدادية | بروفة Q2 للبطاقات، وحزمة القياس عن بعد والتركيبات الثابتة منسل للكريات، وأفضل الأدوات التي تستخدمها أدوات SDK للتحقق من صحة إعادة استخدام المساعد للبطاقات. | @telemetry-ops، برنامج SDK | 30-04-2026 نداء التخطيط | مكمل - جدول الأعمال `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` محفوظ وهو عبارة عن بيانات وصفية للفتحة/عبء العمل تتضمن ہے؛ المزيد من المعلومات حول أداة تعقب إعادة استخدام الحزام. |
| حزمة القياس عن بعد هضم التناوب | تم الانتهاء من البروفة/الإصدار `scripts/telemetry/validate_nexus_telemetry_pack.py` وملخصات تكوين Delta Tracker التي لا تزال مستمرة. | @القياس عن بعد | ہر إطلاق سراح المرشح | مكمل - `telemetry_manifest.json` + `.sha256` `artifacts/nexus/rehearsals/2026q1/` میں جاری ہوئے (نطاق الفتحة `912-936`، البذور `NEXUS-REH-2026Q2`)؛ يمكن العثور على أداة تعقب الملخصات ومؤشر الأدلة. |

## تكامل حزمة دلتا التكوين

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` ملخص الفرق القانوني ہے۔ فيما يتعلق بالجديد `defaults/nexus/*.toml` أو Genesis Changes، يمكنك تعقب المزيد من الأنشطة والخلاصات التي تتضمنها اللعبة.
- حزم التكوين الموقعة بروفة القياس عن بعد حزمة تغذية البطاقة. تحتوي هذه الحزمة على `scripts/telemetry/validate_nexus_telemetry_pack.py` للتحقق من صحة البيانات، ودليل دلتا التكوين ونشرها، كما يمكن لمشغلي الأجهزة B4 استخدامها مرة أخرى وإعادة تشغيل العناصر الاصطناعية.
- Iroha 2 حزم الممرات التي تم إنشاؤها: `nexus.enabled = false` والتكوينات تجاوز المسار/مساحة البيانات/التوجيه ورفض البطاقة عند تمكين ملف تعريف Nexus (`--sora`) لا يوجد لدينا قوالب ذات مسار واحد هي أقسام `nexus.*`.
- سجل تصويت الحوكمة (GOV-2026-03-19) وهو متعقب ولا يمكن استخدام هذه الملاحظة في المستقبل لأصوات المزرعة التي يتم الحصول عليها مرة أخرى.

## إطلاق المتابعات التدريبية

- `docs/source/runbooks/nexus_multilane_rehearsal.md` خطة الكناري، قائمة المشاركين وخطوات التراجع كمحفوظ كرتا ہے؛ فيما يتعلق بطوبولوجيا المسار أو مصدري القياس عن بعد، لا يمكنك استخدام دليل التشغيل الخاص بك.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` 9 أبريل، تم إجراء بروفة على قطعة أثرية من الورق وملاحظات / جدول أعمال الإعداد للربع الثاني. يمكن أن يشتمل متتبع التدريبات المستقبلية هذا على أدلة رتيبة.
- مقتطفات جامع OTLP وصادرات Grafana (`docs/source/telemetry.md`) تب نشر إرشادات التجميع للمصدر بدلاً من ذلك؛ يحتوي تحديث الربع الأول على تنبيهات للمساحة الرئيسية مما يزيد من حجم الدفعة التي تصل إلى 256 عينة.
- دليل اختبار CI/اختبار متعدد المسارات على `integration_tests/tests/nexus/multilane_pipeline.rs` وسير عمل `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`) الموجود تحت چلتا، جيس ليس `pytests/nexus/test_multilane_pipeline.py` ريفيرس كا؛ `defaults/nexus/config.toml` هو hash (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) وهو عبارة عن أداة تعقب متزامنة متزامنة مع حزم البروفة.

## دورة حياة حارة وقت التشغيل

- خطط دورة حياة حارة وقت التشغيل، حيث يتم التحقق من صحة ارتباطات مساحة البيانات وفشل تسوية تخزين Kura/التخزين المتدرج قبل إحباط عملية الإلغاء، ويتم تحديث كتالوج جيس. تساعد الممرات الخلفية والمرحلات المخزنة مؤقتًا على تقليم الملفات وتوليف دفتر الأستاذ المدمج لإعادة استخدام البراهين.
- Nexus config/lifecycle helpers (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) حيث يتم تطبيق الخطط على الممرات المحددة لإعادة التشغيل لإضافة/تقاعد إلى أي مكان؛ التوجيه، لقطات TEU وسجلات البيان خطة التحويل التي يتم تنفيذها بعد إعادة التحميل.
- عوامل التشغيل: إذا فشلت الخطة، فستفقد مساحات بيانات أو جذور تخزين چیك کریں جو بن سكتے (جذر بارد متدرج / أدلة حارة كورا). المسارات الأساسية صحيحة ومتكررة؛ تختلف خطط القياس عن بعد للمسار/مساحة البيانات عن بعد بإصدار بطاقات ولوحة معلومات جديدة وطوبولوجيا جديدة.

## القياس عن بعد لـ NPoS وأدلة الضغط الخلفي

تلتقط بروفة إطلاق المرحلة B الرجعية والقياس عن بعد الحتمي ما هو ثابت مثل جهاز تنظيم ضربات القلب NPoS وطبقات القيل والقال من الضغط الخلفي على نطاق واسع. `integration_tests/tests/sumeragi_npos_performance.rs` وأداة التكامل تنبعث من سيناريوهات العناصر والمقاييس الجديدة وملخصات JSON (`sumeragi_baseline_summary::<scenario>::...`). ما هو الحل:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

`SUMERAGI_NPOS_STRESS_PEERS` أو `SUMERAGI_NPOS_STRESS_COLLECTORS_K` أو `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` تحتوي على عدد كبير من طبولوجيا الضغط والضغط؛ القيم الافتراضية B4 تستخدم 1 s/`k=3` جامع پروفيل کو تعكس کرتے ہیں.| السيناريو / الاختبار | التغطية | القياس عن بعد الرئيسي |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | يصل وقت كتلة التدريب إلى 12 جولة ومغلفات زمن استجابة EMA وأعماق قائمة الانتظار ومقاييس الإرسال الزائدة يتم تسجيلها بالإضافة إلى حزمة الأدلة التي يتم تسلسلها. | `sumeragi_phase_latency_ema_ms`، `sumeragi_collectors_k`، `sumeragi_redundant_send_r`، `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | قائمة انتظار المعاملات التي تزيد من عدد الأشخاص الذين يؤدي تأجيل القبول بشكل حتمي إلى زيادة سعة قائمة الانتظار / تصدير عدادات التشبع. | `sumeragi_tx_queue_depth`، `sumeragi_tx_queue_capacity`، `sumeragi_tx_queue_saturated`، `sumeragi_pacemaker_backpressure_deferrals_total`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | ارتعاش جهاز تنظيم ضربات القلب وعرض عينة المهلات لمدة تصل إلى +/-125 بيرميل نافذ لا يوجد أي شيء. | `sumeragi_pacemaker_jitter_ms`، `sumeragi_pacemaker_view_timeout_target_ms`، `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | تقوم حمولات RBC بتخزين نقرات ناعمة/صلبة بنطاق دفع وجلسات وعدادات بايت تعمل على حفظ البيانات والبيانات وتدفق الفائض لتحقيق الاستقرار في كل مرة. | `sumeragi_rbc_store_pressure`، `sumeragi_rbc_store_sessions`، `sumeragi_rbc_store_bytes`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | يعيد إرسال مقاييس القوة والقوة ونسبة الإرسال الزائدة وعدادات المجمعات على الهدف مرة أخرى، وإظهار القياس عن بعد والقياس عن بعد من النهاية إلى النهاية. | `sumeragi_collectors_targeted_current`، `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | قطع متباعدة بشكل حتمي تسقط الشاشات المتراكمة وتؤدي إلى زيادة أخطاء التصريف المتراكم. | `sumeragi_rbc_backlog_sessions_pending`، `sumeragi_rbc_backlog_chunks_total`، `sumeragi_rbc_backlog_chunks_max`. |

بالإضافة إلى ذلك، هناك شيء يشبه الحوكمة، وهو عبارة عن طوبولوجيا بروفة لأجهزة إنذار الضغط الخلفي، ومطابقة البطاقة، وتسخير خطوط JSON التي يتم كشطها بواسطة Prometheus بواسطة منسلك كريدج.

## تحديث قائمة التحقق

1. تشتمل صفحات التتبع الموجه الجديدة على القراءة، وعندما تنتقل إلى ما هو أبعد من ذلك، ستنتقل حركاتك إلى مرحلة جديدة.
2. متابعة مدير التنبيهات بعد جدول التخفيف في كل مرة، لم تعد هناك تذكرة عمل تذكرة العمل.
3. فيما يتعلق بخيارات دلتا التكوين، والمتعقب، والجديد، وملخصات حزمة القياس عن بعد، فإن هذا هو أول طلب سحب يتم تنفيذه.
4. أي بروفة جديدة/أداة قياس عن بعد ستساعدك على تحسين تحديثات خارطة الطريق المستقبلية، ولا توجد ملاحظة مخصصة أخرى.

## فهرس الأدلة

| اثاثہ | مقام | أخبار |
|-------|----------|-------|
| تقرير تدقيق التتبع التوجيهي (الربع الأول من عام 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | دليل المرحلة B1 کے لئے مصدر قانوني؛ يوجد مرآة في المنفذ `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| تكوين تعقب دلتا | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | TRACE-CONFIG-DELTA ملخصات الفرق والمراجعين والأحرف الأولى وسجل التصويت GOV-2026-03-19 شامل. |
| خطة علاج القياس عن بعد | `docs/source/nexus_telemetry_remediation_plan.md` | حزمة التنبيه، وحجم دفعة OTLP، وحواجز حماية ميزانية التصدير B2. |
| متعدد المسارات بروفة تعقب | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | 9 أعمال بروفة أبريل، بيان/ملخص المدقق، ملاحظات/جدول أعمال Q2 وأدلة التراجع. |
| بيان/ملخص حزمة القياس عن بعد (الأحدث) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | نطاق الفتحة 912-936، والبذرة `NEXUS-REH-2026Q2` وحزم الإدارة وتجزئة القطع الأثرية. |
| بيان ملف تعريف TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | إعادة تشغيل Q2 أثناء الالتقاط تم إغلاق تجزئة ملف تعريف TLS؛ يمكن الاستشهاد بملاحق التتبع الموجهة. |
| جدول أعمال TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | مذكرة تخطيط بروفة الربع الثاني (الموقع، نطاق الفتحة، بذرة عبء العمل، أصحاب الإجراء). |
| إطلاق دليل التدريب | `docs/source/runbooks/nexus_multilane_rehearsal.md` | التدريج -> التنفيذ -> قائمة التحقق من التراجع؛ طوبولوجيا المسار أو إرشادات المصدر تتوسع في المستقبل. |
| مدقق حزمة القياس عن بعد | `scripts/telemetry/validate_nexus_telemetry_pack.py` | B4 retro حوالة ديا جيا CLI؛ قم بتجديد حزمة الملخصات والتعقب التي ستصدرها مجلة كريكايو. |
| الانحدار متعدد المسارات | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | توفير تكوينات متعددة المسارات لـ `nexus.enabled = true` ثابت وتجزئة كتالوج Sora محفوظات رکھتا، و `ConfigLaneRouter` توفير مسارات Kura/دمج سجل المسار المحلي (`blocks/lane_{id:03}_{slug}`) کے هضم المصنوعات اليدوية شاع کرتا ہے۔ |