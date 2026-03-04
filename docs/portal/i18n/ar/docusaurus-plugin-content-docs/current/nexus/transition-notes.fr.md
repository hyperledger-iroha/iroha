---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/transition-notes.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ملاحظات انتقال العلاقة
العنوان: Notes de Transition de Nexus
الوصف: مرآة `docs/source/nexus_transition_notes.md`، تغطي إجراءات المرحلة الانتقالية B، وتقويم التدقيق وعمليات التخفيف.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات الانتقال لـ Nexus

تتناسب هذه المجلة مع العمل المتبقي **المرحلة ب - Nexus أسس الانتقال** حتى نهاية قائمة التحقق من الرماح متعدد المسارات. أكمل مداخل المعالم الرئيسية في `roadmap.md` واحتفظ بالمراجع الأولية حسب B1-B4 في مكان واحد فقط بحيث تشارك الإدارة وSRE والقادة SDK في مصدر الحقيقة.

## بوريه وإيقاع

- قم بتغطية عمليات تدقيق التتبع الموجه وحواجز القياس عن بعد (B1/B2) ومجموعة نطاقات التكوين المعتمدة من خلال الإدارة (B3) ومتابعي التكرار متعدد المسارات (B4).
- استبدال الملاحظة المؤقتة للإيقاع الذي يعيش فيه؛ بعد تدقيق الربع الأول من عام 2026، توجد تفاصيل التقرير في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، بينما تحافظ هذه الصفحة على دوران التقويم وسجل عمليات التخفيف.
- Mettez a jour les Tables après chaque fenetre routing-trace, voit de goovernance ou reprise de lancement. عند زيادة العناصر، قم بإعادة الموضع الجديد في هذه الصفحة بحيث تكون المستندات المتاحة (الحالة، ولوحات المعلومات، وبوابات SDK) أكثر استقرارًا.

## لقطة من العروض (الربع الأول من العام 2026 والربع الثاني من عام 2026)

| مسار العمل | بريفز | المالك (المالكون) | النظام الأساسي | ملاحظات |
|------------|----------|----------|--------|-------|
| **B1 - عمليات تدقيق التتبع الموجهة** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`، `docs/examples/nexus_audit_outcomes/` | @telemetry-ops، @governance | مكتمل (الربع الأول من عام 2026) | ثلاثة نوافذ مسجلة للتدقيق؛ تم إغلاق TLS de `TRACE-CONFIG-DELTA` عند إعادة تشغيل Q2. |
| **B2 - معالجة أجهزة القياس عن بعد وحواجز الحماية** | `docs/source/nexus_telemetry_remediation_plan.md`، `docs/source/telemetry.md`، `dashboards/alerts/nexus_audit_rules.yml` | @sre-core، @telemetry-ops | أكمل | حزمة التنبيه، سياسة الروبوت المختلف وتفاصيل الكثير OTLP (سجل `nexus.scheduler.headroom` + لوحة Grafana من الإرتفاع) ؛ تنازل عن ouvert. |
| **B3 - موافقات دلتا التكوين** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`، `defaults/nexus/config.toml`، `defaults/nexus/genesis.json` | @release-eng, @governance | أكمل | التصويت GOV-2026-03-19 التسجيل; تحتوي الحزمة على حزمة بيانات القياس عن بعد بالإضافة إلى ذلك. |
| **B4 - تكرار الرمي متعدد المسارات** | `docs/source/runbooks/nexus_multilane_rehearsal.md`، `docs/source/project_tracker/nexus_rehearsal_2026q1.md`، `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`، `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`، `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core، @sre-core | كامل (الربع الثاني 2026) | تساعد إعادة تشغيل كناري Q2 على تخفيف تأخير TLS؛ يلتقط برنامج التحقق من الصحة + `.sha256` فتحة الفتحات 912-936، وبذرة عبء العمل `NEXUS-REH-2026Q2`، ويسجل تجزئة ملف تعريف TLS أثناء إعادة التشغيل. |

## Calendrier Trimestriel des Audits router-trace

| معرف التتبع | فينيتري (UTC) | النتيجة | ملاحظات |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | رويسي | قائمة الانتظار - القبول P95 تبقى جيدة على أقل من 750 مللي ثانية. يتطلب العمل Aucune. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | رويسي | تتميز إعادة تشغيل OTLP بالرقم `status.md`؛ يؤكد تكافؤ diff bot SDK على عدم الانجراف. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | الحل | يتم تأخير TLS أثناء إعادة تشغيل Q2؛ تقوم حزمة القياس عن بعد من أجل `NEXUS-REH-2026Q2` بتسجيل تجزئة الملف الشخصي TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (البحث عن `artifacts/nexus/tls_profile_rollout_2026q2/`) وبدون أي تأخير. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | رويسي | بذرة عبء العمل `NEXUS-REH-2026Q2`؛ حزمة القياس عن بعد + البيان/الملخص في `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة 912-936) مع جدول الأعمال في `artifacts/nexus/rehearsals/2026q2/`. |

يجب إضافة الخطوط الجديدة في الأشهر الثلاثة المقبلة وإزاحة المقبلات النهائية إلى ملحق عندما تتجاوز الطاولة الثلثين الرئيسيين. قم بالرجوع إلى هذا القسم بعد التقارير الموجهة أو محضر الإدارة باستخدام `#quarterly-routed-trace-audit-schedule`.

## عمليات التخفيف والعناصر المتراكمة

| العنصر | الوصف | المالك | سيبل | النظام الأساسي / ملاحظات |
|------|-------------|--------|----------------|
| `NEXUS-421` | قم بإنهاء نشر ملف تعريف TLS الذي يؤدي إلى تأخير `TRACE-CONFIG-DELTA`، والتقاط إجراءات إعادة التشغيل وإغلاق تسجيل التخفيف. | @release-eng، @sre-core | تتبع التتبع للنوافذ Q2 2026 | إغلاق - تجزئة الملف الشخصي TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` التقاط في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`؛ قم بإعادة التشغيل لتأكيد عدم وجود أي تأخيرات. |
| `TRACE-MULTILANE-CANARY` الإعدادية | قم ببرمجة التكرار Q2، وربط التركيبات بحزمة القياس عن بعد والتأكد من أن أدوات SDK التي يتم تسخيرها تعيد استخدام المساعد الصالح. | @telemetry-ops، برنامج SDK | طلب التخطيط 2026-04-30 | مكتمل - مخزون جدول الأعمال في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع فتحة/عبء العمل لبيانات التعريف؛ إعادة استخدام الحزام الملاحظ في جهاز التعقب. |
| حزمة القياس عن بعد هضم التناوب | قم بتنفيذ `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل تكرار/تحرير ثم قم بتسجيل الملخصات في متتبع تكوين دلتا. | @القياس عن بعد | مرشح الإصدار الاسمي | مكتمل - `telemetry_manifest.json` + `.sha256` emis dans `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة `912-936`، البذرة `NEXUS-REH-2026Q2`)؛ خلاصات النسخ في المتتبع وفي فهرس العروض. |

## تكامل حزمة دلتا التكوين

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` يُبقي السيرة الذاتية للفرق. عند وصول `defaults/nexus/*.toml` الجديد أو تغييرات التكوين، قم بتتبع يوم ce Tracker d'abord ثم قم بإعادة النقاط إلى cle ici.
- حزم التكوين الموقعة تتضمن حزمة القياس عن بعد للتكرار. يجب نشر الحزمة، الصالحة وفقًا لـ `scripts/telemetry/validate_nexus_telemetry_pack.py`، مع إجراءات تكوين دلتا حتى يتمكن المشغلون من تجديد المصنوعات اليدوية الدقيقة التي تستخدم قلادة B4.
- الحزم Iroha 2 موجودة بدون ممرات: يتم إعادة تكوين التكوينات مع `nexus.enabled = false` للحفاظ على تجاوزات المسار/مساحة البيانات/التوجيه إذا كان ملف التعريف Nexus نشطًا (`--sora`)، دون حذف الأقسام `nexus.*` للقوالب أحادية المسار.
- قم بحفظ سجل التصويت للحوكمة (GOV-2026-03-19) من خلال المتتبع وهذه ملاحظة من أجل أن الأصوات المستقبلية قادرة على نسخ التنسيق دون الحاجة إلى إعادة اكتشاف طقوس الموافقة.

## متابعة تكرار الرمح

- `docs/source/runbooks/nexus_multilane_rehearsal.md` يلتقط خطة الكناري وقائمة المشاركين وخطوات التراجع؛ استخدم دليل التشغيل يوميًا عند تغيير طوبولوجيا الممرات أو مصدري القياس عن بعد.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` قائمة كل العناصر التي تم التحقق منها أثناء التكرار في 9 أبريل والمحتوى الذي يحتفظ بالملاحظات/جدول أعمال الإعداد Q2. أضف التكرارات المستقبلية إلى متتبع الميمات بدلاً من فتح متتبعات مخصصة للحفاظ على النغمات الرتيبة الأولية.
- نشر مقتطفات من تجميع OTLP والصادرات Grafana (البحث عن `docs/source/telemetry.md`) عند تغيير إرساليات التجميع للمصدر؛ ميزه الربع الأول من العام تحتوي على 256 مفتاحًا لتجنب تنبيهات الإرتفاع.
- تعمل اختبارات CI/اختبارات المسارات المتعددة بشكل مستمر في `integration_tests/tests/nexus/multilane_pipeline.rs` وتدور داخل سير العمل `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)، بدلاً من المرجع المتقاعد `pytests/nexus/test_multilane_pipeline.py`؛ قم بحفظ التجزئة لـ `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) متزامنًا مع المتتبع أثناء تكثيف حزم التكرار.

## دورة تشغيل الممرات

- خطط دورة حياة الممرات في وقت التشغيل صالحة للحفاظ على روابط مساحة البيانات وإيقافها عند مطابقة Kura/stockage على الألواح، والسماح بتغيير الكتالوج. تقوم الأدوات المساعدة بإزالة ذاكرات التخزين المؤقت للممرات المتقاعدة حتى لا يتم إعادة استخدام دفتر الأستاذ المدمج بدون البراهين القديمة.
- تطبيق الخطط عبر مساعدات التكوين/دورة الحياة Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) لإضافة/سحب الممرات بدون إعادة رسمها؛ يتم إعادة شحن التوجيه واللقطات TEU وسجلات البيانات تلقائيًا بعد خطة جديدة.
- دليل المشغل: عند وجود خطة صدى، تحقق من مساحات البيانات الضعيفة أو جذور التخزين المستحيلة (الجذر البارد المتدرج/ذخيرة Kura par line). قم بتصحيح المسارات الأساسية وإعادة صياغتها؛ تُعيد الخطط الجديدة إصدار فرق القياس عن بعد/مساحة البيانات بحيث تعكس لوحات المعلومات الطوبولوجيا الجديدة.

## القياس عن بعد NPoS وقياس الضغط الخلفي

إن تكرار تكرار المرحلة B يتطلب التقاط لقطات قياس عن بعد مما يؤكد أن جهاز تنظيم ضربات القلب NPoS وأريكة القيل والقال تبقى في حدود الضغط الخلفي. يتم تنفيذ سيناريوهات التكامل في `integration_tests/tests/sumeragi_npos_performance.rs` واستئناف JSON (`sumeragi_baseline_summary::<scenario>::...`) عند وصول مقاييس جديدة. Lancez-le localement avec:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

قم بتعريف `SUMERAGI_NPOS_STRESS_PEERS`، `SUMERAGI_NPOS_STRESS_COLLECTORS_K` أو `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف الطبولوجيا بالإضافة إلى الضغوطات؛ تعكس القيم الافتراضية ملف تعريف المجمع 1 s/`k=3` المستخدم في B4.| السيناريو / الاختبار | كوفيرتور | القياس عن بعد |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | قم بحظر 12 جولة مع وقت التكرار لتسجيل مغلفات زمن الاستجابة EMA وأعمق الملفات ومقاييس التكرار التي يتم إرسالها قبل تسلسل حزمة Preuves. | `sumeragi_phase_latency_ema_ms`، `sumeragi_collectors_k`، `sumeragi_redundant_send_r`، `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | قم بإدخال ملف المعاملات للتأكد من أن تأجيلات القبول يتم تحديدها بشكل نشط وأن الملف يصدر مقاييس السعة/التشبع. | `sumeragi_tx_queue_depth`، `sumeragi_tx_queue_capacity`، `sumeragi_tx_queue_saturated`، `sumeragi_pacemaker_backpressure_deferrals_total`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | Echantillonne ارتعاش جهاز تنظيم ضربات القلب ومهلة الرؤية حتى يتم التأكد من أن النطاق +/-125 في الدقيقة مطبق. | `sumeragi_pacemaker_jitter_ms`، `sumeragi_pacemaker_view_timeout_target_ms`، `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | قم بدفع إجمالي حمولات RBC حتى يتم تخزين الحدود الناعمة/الصلبة من أجل مراقبة الجلسات وحواسب البايت الكبيرة والثابتة دون تجاوز المخزن. | `sumeragi_rbc_store_pressure`، `sumeragi_rbc_store_sessions`، `sumeragi_rbc_store_bytes`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | فرض عمليات إعادة الإرسال من أجل إرسال مقاييس النسبة الزائدة ووحدات التجميع على الهدف بشكل متقدم، مما يدل على أن القياس عن بعد المطلوب قديمًا يتفرع من النهاية إلى النهاية. | `sumeragi_collectors_targeted_current`، `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | اترك القطع على فترات زمنية محددة للتحقق من أن شاشات التراكم تشير إلى الأخطاء بدلاً من تجفيف الحمولات. | `sumeragi_rbc_backlog_sessions_pending`، `sumeragi_rbc_backlog_chunks_total`، `sumeragi_rbc_backlog_chunks_max`. |

قم بتشغيل خطوط JSON المطبوعة باستخدام أداة الالتقاط Prometheus عند التنفيذ كل مرة تطلب فيها الإدارة إجراءات بحيث تتوافق إنذارات الضغط الخلفي مع طوبولوجيا التكرار.

## قائمة التحقق من إعداد اليوم

1. قم بإضافة نوافذ جديدة لتتبع التوجيه وحذف الحواجز القديمة أثناء تشغيل الأشهر الثلاثة.
2. قم بالبدء في جدول التخفيف بعد كل مرة يتبعها مدير التنبيه، حتى لو كان الإجراء يتكون من إغلاق التذكرة.
3. عند تغيير تكوينات الدلتا، قم بمتابعة المتتبع يوميًا، وقم بكتابة هذه الملاحظة وقائمة خلاصات حزمة القياس عن بعد في طلب السحب.
4. استخدم كل أدوات التكرار/القياس عن بعد الجديدة حتى تتمكن الخرائط المستقبلية في يوم خريطة الطريق من الرجوع إلى مستند واحد بدلاً من الملاحظات المخصصة المتفرقة.

## فهرس العروض

| اكتيف | التمركز | ملاحظات |
|-------|----------|-------|
| تقرير التدقيق الموجه (الربع الأول من عام 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المصدر canonique pour les preuves المرحلة B1؛ مرآة للباب السفلي `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| تعقب دلتا التكوين | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | تحتوي على سير ذاتية للفرق TRACE-CONFIG-DELTA والأحرف الأولى من المراجعين وسجل التصويت GOV-2026-03-19. |
| خطة معالجة القياس عن بعد | `docs/source/nexus_telemetry_remediation_plan.md` | قم بتوثيق حزمة التنبيه وتفاصيل OTLP وحواجز ميزانية التصدير في B2. |
| متتبع التكرار متعدد المسارات | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | قم بإدراج العناصر التي تم التحقق منها خلال التكرار في 9 أبريل، ومدقق البيان/الملخص، والملاحظات/جدول الأعمال Q2، ومقدمات التراجع. |
| بيان/ملخص حزمة القياس عن بعد (الأحدث) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | قم بتسجيل الشاطئ 912-936، والبذرة `NEXUS-REH-2026Q2` والتجزئات الفنية لحزم الإدارة. |
| بيان ملف تعريف TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | موافقة تجزئة ملف تعريف TLS على الالتقاط وإعادة تشغيل Q2؛ citez-le dans les Annexes توجيه التتبع. |
| جدول الأعمال TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | ملاحظات التخطيط للتكرار Q2 (النوافذ، نطاق الفتحات، بذور عبء العمل، مالكي الإجراءات). |
| Runbook de تكرار الرمح | `docs/source/runbooks/nexus_multilane_rehearsal.md` | عملية قائمة التحقق من أجل التدريج -> التنفيذ -> التراجع؛ قم بالمتابعة يوميًا عندما تتغير بنية الممرات أو نصائح المصدرين. |
| مدقق حزمة القياس عن بعد | `scripts/telemetry/validate_nexus_telemetry_pack.py` | مرجع CLI على قدم المساواة مع B4 الرجعية؛ أرشفة الملخصات مع المتتبع كل مرة تتغير فيها الحزمة. |
| الانحدار متعدد المسارات | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | صالح `nexus.enabled = true` للتكوينات متعددة المسارات، واحتفظ بتجزئة الكتالوج Sora وقم بتوفير chemins Kura/merge-log par line (`blocks/lane_{id:03}_{slug}`) عبر `ConfigLaneRouter` قبل نشر ملخصات العناصر. |