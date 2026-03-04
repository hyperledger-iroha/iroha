---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/transition-notes.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ملاحظات انتقال العلاقة
العنوان: Notas de transicion de Nexus
الوصف: Espejo de `docs/source/nexus_transition_notes.md`، الذي يقدم أدلة على انتقال المرحلة B، وتقويم السمع والتخفيف.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات النقل Nexus

تم تسجيل هذا العمل على أمل **المرحلة B - Nexus Transition Foundations** حتى تنتهي من قائمة التحقق من التخطيط متعدد المسارات. أكمل إدخالات المعالم في `roadmap.md` واحتفظ بالأدلة المرجعية لـ B1-B4 في مكان واحد فقط لإدارة SRE وقواعد SDK للتشارك في نفس الشيء الحقيقي.

## الكانس والإيقاع

- قم بتغطية المستمعين بتتبع التوجيه وحواجز الحماية عن بعد (B1/B2) ومجموعة نطاقات التكوين المناسبة للإدارة (B3) وملحقات التخطيط متعدد المسارات (B4).
- إعادة بناء ملاحظة الإيقاع الزمني التي كانت ستعيشها من قبل؛ من جلسة الاستماع للربع الأول من عام 2026، يوجد التقرير المفصل في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، بينما تحافظ هذه الصفحة على التقويم التشغيلي وسجل التخفيفات.
- تحديث اللوحات بعد كل نافذة توجيهية، أو التحكم في الصوت، أو التتبع. عندما يتم تغيير العناصر، فإنها تعكس الموقع الجديد في هذه الصفحة بحيث يمكن للمستندات اللاحقة (الحالة، ولوحات المعلومات، وبوابات SDK) أن تضيف ملحقًا ثابتًا.

## لقطة من الأدلة (الربع الأول من العام 2026 والربع الثاني من عام 2026)

| مسار العمل | ايفيدنسيا | المالك (المالكون) | حالة | نوتاس |
|------------|----------|----------|--------|-------|
| **B1 - عمليات تدقيق التتبع الموجهة** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`، `docs/examples/nexus_audit_outcomes/` | @telemetry-ops، @governance | كامل (الربع الأول من عام 2026) | ثلاث فتحات سمعية مسجلة؛ سيتم إغلاق TLS de `TRACE-CONFIG-DELTA` أثناء إعادة تشغيل Q2. |
| **B2 - معالجة أجهزة القياس عن بعد وحواجز الحماية** | `docs/source/nexus_telemetry_remediation_plan.md`، `docs/source/telemetry.md`، `dashboards/alerts/nexus_audit_rules.yml` | @sre-core، @telemetry-ops | كامل | حزمة تنبيهات، وروبوتات سياسية مختلفة، وحجم كبير من OTLP (سجل `nexus.scheduler.headroom` + لوحة Grafana من الإرتفاع) مرسلة؛ التنازل عن الخطيئة abiertos. |
| **B3 - مقاييس التكوين** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`، `defaults/nexus/config.toml`، `defaults/nexus/genesis.json` | @release-eng, @governance | كامل | التصويت على GOV-2026-03-19 مسجل؛ حزمة الطاقة الثابتة حزمة القياس عن بعد متوقفة. |
| **B4 - Ensayo de lanzamiento multi-lane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`، `docs/source/project_tracker/nexus_rehearsal_2026q1.md`، `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`، `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`، `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core، @sre-core | كامل (الربع الثاني 2026) | تؤدي إعادة تشغيل كناري Q2 إلى تخفيف عودة TLS؛ يلتقط بيان المدقق + `.sha256` نطاق الفتحات 912-936، وأعباء العمل الأساسية `NEXUS-REH-2026Q2`، وتجزئة ملف TLS المسجل في إعادة التشغيل. |

## Calendario Trimestral de Auditorias Routed-Trace

| معرف التتبع | فينتانا (التوقيت العالمي المنسق) | النتيجة | نوتاس |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | ابروبادو | يتم التحكم في قائمة الانتظار P95 بشكل كبير من خلال الهدف <= 750 مللي ثانية. لا يتطلب الأمر اتخاذ إجراء. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | ابروبادو | إعادة تشغيل OTLP للتجزئات الملحقة بـ `status.md`؛ يؤكد تطابق الروبوتات المختلفة لـ SDK الانجراف الكامل. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | ريسويلتو | سيتم إغلاق TLS الرجعي أثناء إعادة تشغيل Q2؛ تقوم حزمة القياس عن بعد لـ `NEXUS-REH-2026Q2` بتسجيل تجزئة الملف الشخصي TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (الإصدار `artifacts/nexus/tls_profile_rollout_2026q2/`) ويتم حذفها بالكامل. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | ابروبادو | بذرة عبء العمل `NEXUS-REH-2026Q2`؛ حزمة القياس عن بعد + البيان/الملخص في `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة 912-936) مع جدول الأعمال في `artifacts/nexus/rehearsals/2026q2/`. |

يجب أن تجمع الأشهر الثلاثة المستقبلية فصولًا جديدة وتحرك المدخلات المكتملة إلى ملحق عندما تبدأ الطاولة في الأشهر الثلاثة الحالية. مرجع هذا القسم من التقارير الموجهة أو دقائق الإدارة باستخدام القائمة `#quarterly-routed-trace-audit-schedule`.

## التخفيف والعناصر المتراكمة

| العنصر | الوصف | المالك | الهدف | حالة / نوتاس |
|------|-------------|--------|----------------|
| `NEXUS-421` | قم بإنهاء نشر ملف TLS الذي تمت إعادته خلال `TRACE-CONFIG-DELTA`، والتقاط أدلة إعادة التشغيل والعثور على سجل التخفيف. | @release-eng، @sre-core | تتبع مسار Ventana للربع الثاني من عام 2026 | Cerrado - تم التقاط تجزئة ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`؛ تؤكد إعادة التشغيل أنه ليس هناك أي إرجاع. |
| `TRACE-MULTILANE-CANARY` الإعدادية | قم ببرمجة اختبار Q2 والتركيبات الإضافية على حزمة القياس عن بعد والتأكد من أن أدوات SDK التي تستخدمها تعيد استخدام المساعد المعتمد. | @telemetry-ops، برنامج SDK | مكالمات الطائرة 2026-04-30 | مكتمل - تم تخزين جدول الأعمال في `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع بيانات تعريف الفتحة/عبء العمل؛ إعادة استخدام الحزام الموضح في المتعقب. |
| حزمة القياس عن بعد هضم التناوب | قم بتشغيل `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل كتابة/تحرير وتسجيل الملخصات جنبًا إلى جنب مع متتبع التكوين دلتا. | @القياس عن بعد | من أجل الافراج عن مرشح | مكتمل - `telemetry_manifest.json` + `.sha256` صادر في `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة `912-936`، البذرة `NEXUS-REH-2026Q2`)؛ يلخص النسخ في المتتبع ومؤشر الأدلة. |

## تكامل حزمة دلتا التكوين

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` سيبقيك على علم باستئناف الاختلافات. عند إضافة `defaults/nexus/*.toml` الجديد أو تغييرات التكوين، يتم تحديث هذا المتتبع الأول ويعيد النظر في النقاط الرئيسية هنا.
- حزم التكوين الموقعة هي حزمة القياس عن بعد للتعلم. يجب نشر الحزمة، التي تم التحقق من صحتها بواسطة `scripts/telemetry/validate_nexus_telemetry_pack.py`، جنبًا إلى جنب مع أدلة التكوين دلتا حتى يتمكن المشغلون من إعادة إنتاج المصنوعات الدقيقة المستخدمة خلال B4.
- حزم Iroha ذات مسارين بدون ممرات: التكوينات مع `nexus.enabled = false` تم إعادة ضبطها الآن لتجاوزات المسار/مساحة البيانات/التوجيه حتى يكون الملف Nexus جاهزًا (`--sora`)، كما يتم حذفه أقسام `nexus.*` من النباتات ذات المسار الواحد.
- احتفظ بسجل تصويت الحكومة (GOV-2026-03-19) مفعلًا من المتتبع كملاحظة حتى تتمكن المسابقات المستقبلية من نسخ التنسيق دون إعادة اكتشاف طقوس الموافقة.

## تتبع خطوات التنظيف

- `docs/source/runbooks/nexus_multilane_rehearsal.md` يلتقط خطة الكناري وقائمة المشاركين وخطوات التراجع؛ تحديث دليل التشغيل عند تغيير طوبولوجيا الممرات أو مصدري القياس عن بعد.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` قائمة كل قطعة أثرية تمت مراجعتها خلال فترة البحث في 9 أبريل والآن تتضمن ملاحظات/جدول أعمال الإعداد Q2. جمع الدراسات المستقبلية بنفس الطريقة التي يتم بها فتح أدوات التتبع للحفاظ على الأدلة الرتيبة.
- نشر مقتطفات من مُجمِّع OTLP وتصدير Grafana (الإصدار `docs/source/telemetry.md`) عند تغيير دليل تجميع المُصدِّر؛ تحديث الربع الأول يجعل حجم الدفعة يصل إلى 256 قطعة لتجنب تنبيهات الإرتفاع.
- دليل CI/اختبارات المسارات المتعددة يعيش الآن في `integration_tests/tests/nexus/multilane_pipeline.rs` ويتبع سير العمل `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)، مع استبدال المرجع السابق `pytests/nexus/test_multilane_pipeline.py`؛ احتفظ بالتجزئة لـ `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) وقم بالمزامنة مع المتعقب لتحديث حزم البحث.

## دورة حياة الممرات في وقت التشغيل

- مستويات دورة حياة الممرات في وقت التشغيل بعد التحقق من ارتباطات مساحة البيانات وإيقافها عندما تفشل تسوية Kura/التخزين حسب المستويات، بعد ترك الكتالوج بدون تغيير. يمكن للمساعدين ترحيل الممرات في ذاكرة التخزين المؤقت للممرات المسحوبة، بحيث لا يؤدي دمج دفتر الأستاذ المدمج إلى إعادة استخدام البراهين القديمة.
- يطبق التطبيق من خلال مساعدي التكوين/دورة حياة Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) لإعادة دمج/سحب الممرات دون إعادة إنشائها؛ يتم إعادة شحن التوجيه واللقطات TEU وسجلات البيانات تلقائيًا من خلال خطة خروج.
- دليل المشغلين: عند فشل الخطة، قم بمراجعة مساحات البيانات الخاطئة أو جذور التخزين التي لا يمكن أن تنمو (الجذر البارد المتدرج/المجلدات حسب المسار). تصحيح القاعدة وإعادة التدوير; تُعيد الطائرات الموجودة في الخارج فرق القياس عن بعد للمسار/مساحة البيانات بحيث تعكس لوحات المعلومات الطوبولوجيا الجديدة.

## القياس عن بعد لـ NPoS وأدلة الضغط الخلفي

إن تجربة إطلاق المرحلة B القديمة تلتقط عددًا من لقطات القياس عن بعد التي تحدد أن جهاز تنظيم ضربات القلب NPoS وأغطية القيل والقال تحافظ على حدود الضغط الخلفي الخاصة بك. يقوم حزام التكامل في `integration_tests/tests/sumeragi_npos_performance.rs` بإخراج هذه السيناريوهات وإصدار السيرة الذاتية JSON (`sumeragi_baseline_summary::<scenario>::...`) عند إضافة مقاييس جديدة. التنفيذ المحلي مع:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

التكوين `SUMERAGI_NPOS_STRESS_PEERS`، `SUMERAGI_NPOS_STRESS_COLLECTORS_K` أو `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات معظم هذه الكائنات؛ تعكس القيم المعيبة ملف الذاكرة 1 s/`k=3` المستخدم في B4.| السيناريو / الاختبار | كوبرتورا | القياس عن بعد |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | قم بحظر 12 دائرة مع وقت الكتابة لتسجيل مغلفات زمن الوصول EMA وأعماق الكولا ومقاييس التكرار لإرسال حزمة الأدلة قبل تسلسلها. | `sumeragi_phase_latency_ema_ms`، `sumeragi_collectors_k`، `sumeragi_redundant_send_r`، `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | قم بملء الكولا من المعاملات للتأكد من أن تأجيلات القبول يتم تفعيلها بشكل محدد وأن الكولا تصدر متحكمات السعة/التشبع. | `sumeragi_tx_queue_depth`، `sumeragi_tx_queue_capacity`، `sumeragi_tx_queue_saturated`، `sumeragi_pacemaker_backpressure_deferrals_total`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | تظهر اهتزازات جهاز تنظيم ضربات القلب ومهلات المشاهدة أن النطاق +/-125 في الدقيقة مطبق. | `sumeragi_pacemaker_jitter_ms`، `sumeragi_pacemaker_view_timeout_target_ms`، `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | تحتوي حمولات RBC الكبيرة على حدود التخزين الناعمة/الصلبة لإظهار الجلسات ومفاتيح البايت الفرعية، وإعادتها وتثبيتها دون الحاجة إلى فتح المتجر. | `sumeragi_rbc_store_pressure`، `sumeragi_rbc_store_sessions`، `sumeragi_rbc_store_bytes`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | تعمل عمليات إعادة الإرسال القوية على إرسال مقاييس النسبة المتكررة وعدادات المجمعين على الهدف على التقدم، مما يوضح أن القياس عن بعد يتم توصيله من البداية إلى النهاية. | `sumeragi_collectors_targeted_current`، `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | قم بإزالة أجزاء من الفواصل الزمنية المحددة للتحقق من أن شاشات التراكم تفشل في مكان سحب الحمولات بشكل صامت. | `sumeragi_rbc_backlog_sessions_pending`، `sumeragi_rbc_backlog_chunks_total`، `sumeragi_rbc_backlog_chunks_max`. |

قم بإضافة خطوط JSON التي يتم تشغيلها جنبًا إلى جنب مع أداة المسح Prometheus التي تم التقاطها أثناء عملية الإخراج التي تستمر في الحصول على أدلة على أن إنذارات الضغط الخلفي تتزامن مع طوبولوجيا الدراسة.

## قائمة المراجعة للتحديث

1. أدخل التتبع التوجيهي للنوافذ الجديدة واسحب القديم عند تعفن الأشهر الثلاثة.
2. قم بتحديث جدول التخفيف بعد كل تتبع لـ Alertmanager، حتى في حالة إغلاق الإجراء.
3. عندما تقوم بتغيير مجموعات التكوين، وتحديث المتتبع، وهذه الملاحظة وقائمة ملخصات حزمة القياس عن بعد في نفس طلب السحب.
4. قم بملء أي قطعة أثرية جديدة للأبحاث/القياس عن بعد حتى تتمكن تحديثات خارطة الطريق المستقبلية من الرجوع إلى مستند واحد فقط بعد الملاحظات المخصصة المشتتة.

## فهرس الأدلة

| اكتيف | أوبيكاسيون | نوتاس |
|-------|----------|-------|
| تقرير التتبع الموجه (الربع الأول من عام 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | المصدر الأساسي لدليل المرحلة B1؛ تم عرضه على البوابة الإلكترونية `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| تعقب دلتا التكوين | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | تحتوي على السيرة الذاتية للفرق TRACE-CONFIG-DELTA والمراجعة الأولية وسجل التصويت GOV-2026-03-19. |
| خطة علاج القياس عن بعد | `docs/source/nexus_telemetry_remediation_plan.md` | قم بتوثيق حزمة التنبيهات وحجم الكثير من OTLP وحواجز حماية التصدير المسبقة إلى B2. |
| Tracker de ensayo متعدد المسارات | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | قم بإدراج المصنوعات اليدوية الصادرة في 9 أبريل، وبيان/ملخص المدقق، والملاحظات/جدول الأعمال للربع الثاني، وأدلة التراجع. |
| بيان/ملخص حزمة القياس عن بعد (الأحدث) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | قم بتسجيل نطاق الفتحة 912-936، والبذرة `NEXUS-REH-2026Q2`، وتجزئة القطع الأثرية لحزم الإدارة. |
| بيان ملف تعريف TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | تم التقاط تجزئة ملف تعريف TLS أثناء إعادة تشغيل الربع الثاني؛ citar وملحق تتبع التوجيه. |
| جدول الأعمال TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | ملاحظات تخطيط التصميم Q2 (النافذة، نطاق الفتحة، بذور عبء العمل، أصحاب الإجراءات). |
| Runbook de Ensayo de Lanzamiento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | قائمة التحقق التشغيلية للتدريج -> القذف -> التراجع؛ قم بالتحديث عند تغيير طوبولوجيا الممرات أو دليل المصدرين. |
| مدقق حزمة القياس عن بعد | `scripts/telemetry/validate_nexus_telemetry_pack.py` | مرجع CLI للرجعية B4؛ أرشيفي يهضم Junto Al Tracker عندما تكون الحزمة Cambia. |
| الانحدار متعدد المسارات | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | اختبار `nexus.enabled = true` للتكوينات متعددة المسارات، والحفاظ على تجزئات الكتالوج Sora وتوفير مسارات Kura/merge-log للمسار (`blocks/lane_{id:03}_{slug}`) عبر `ConfigLaneRouter` قبل نشر ملخصات المصنوعات. |