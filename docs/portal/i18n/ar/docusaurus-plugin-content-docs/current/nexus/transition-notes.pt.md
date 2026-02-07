---
lang: ar
direction: rtl
source: docs/portal/docs/nexus/transition-notes.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: ملاحظات انتقال العلاقة
العنوان: Notas de transicao do Nexus
الوصف: تم توضيح `docs/source/nexus_transition_notes.md`، من خلال أدلة النقل من المرحلة B، أو تقويم السمع والتخفيف.
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# ملاحظات النقل Nexus

يرافق هذا السجل العمل المتوقع من **المرحلة ب - Nexus أسس الانتقال** بالإضافة إلى قائمة مرجعية للخط متعدد المسارات. إنه مكمل لإدخالات المعالم في `roadmap.md` ويحتفظ بالأدلة المرجعية لـ B1-B4 في مكان فريد لإدارة SRE وقيادة SDK للمشاركة في نفس المصدر الحقيقي.

## إسكوبو إي كادنسيا

- تشمل المدرجات ذات التتبع الموجه وحواجز الحماية عن بعد (B1/B2)، أو مجموعة من خطوط التكوين المعتمدة من قبل الإدارة (B3) والمرافقات المصاحبة للربط متعدد المسارات (B4).
- استبدال ملاحظة مؤقتة للإيقاع قبل أن تعيش هنا؛ من جلسة الاستماع للربع الأول من عام 2026، توجد العلاقة التفصيلية في `docs/source/nexus_routed_trace_audit_report_2026q1.md`، أثناء حفظ هذه الصفحة أو التقويم الحالي وسجل التخفيف.
- يمكنك تحديثه على شكل لوحات بعد كل عام من توجيه التتبع أو تصويت الإدارة أو التتبع. عند تحريك الأدوات، قم بإعادة تحديد موقع جديد في هذه الصفحة حتى تتمكن المستندات اللاحقة (الحالة، ولوحات المعلومات، وSDK المحمولة) من الارتباط بحالة ثابتة.

## لقطة من الأدلة (الربع الأول من العام 2026 والربع الثاني من عام 2026)

| مسار العمل | ايفيدنسيا | المالك (المالكون) | الحالة | نوتاس |
|------------|----------|----------|--------|-------|
| **B1 - عمليات تدقيق التتبع الموجهة** | `docs/source/nexus_routed_trace_audit_report_2026q1.md`، `docs/examples/nexus_audit_outcomes/` | @telemetry-ops، @governance | كامل (الربع الأول من عام 2026) | ثلاث سجلات سمعية مسجلة؛ تم حذف TLS من `TRACE-CONFIG-DELTA` أثناء أو إعادة تشغيل Q2. |
| **B2 - معالجة القياس عن بعد وحواجز الحماية** | `docs/source/nexus_telemetry_remediation_plan.md`، `docs/source/telemetry.md`، `dashboards/alerts/nexus_audit_rules.yml` | @sre-core، @telemetry-ops | كامل | حزمة التنبيهات، وروبوتات السياسة المختلفة، وحجم الكثير من OTLP (سجل `nexus.scheduler.headroom` + طلاء Grafana من الإرتفاع) مرسلة؛ التنازلات وSem em aberto. |
| **B3 - عمليات تحديد دلتا التكوين** | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md`، `defaults/nexus/config.toml`، `defaults/nexus/genesis.json` | @release-eng, @governance | كامل | التصويت على GOV-2026-03-19 مسجل؛ أو حزمة أغذية مقطوعة أو حزمة قياس عن بعد مقتبسة. |
| **B4 - Ensaio de lancamento multilane** | `docs/source/runbooks/nexus_multilane_rehearsal.md`، `docs/source/project_tracker/nexus_rehearsal_2026q1.md`، `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json`، `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json`، `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | @nexus-core، @sre-core | كامل (الربع الثاني 2026) | قم بإعادة تشغيل كناري Q2 للتخفيف من TLS; o بيان أداة التحقق من الصحة + `.sha256` التقاط الفاصل الزمني للفتحات 912-936، بذرة عبء العمل `NEXUS-REH-2026Q2` e o تجزئة ملف تسجيل TLS بدون إعادة التشغيل. |

## Calendario Trimestral de Auditorias Routed-Trace

| معرف التتبع | جانيلا (التوقيت العالمي المنسق) | النتيجة | نوتاس |
|----------|--------------|---------|-------|
| `TRACE-LANE-ROUTING` | 2026-02-17 09:00-09:45 | أبروفادو | قائمة الانتظار - القبول P95 يمكن أن تكون منخفضة للغاية <= 750 مللي ثانية. نينهوما أكاو ضروريا. |
| `TRACE-TELEMETRY-BRIDGE` | 2026-02-24 10:00-10:45 | أبروفادو | إعادة تشغيل OTLP للتجزئات الملحقة بـ `status.md`؛ يؤكد paridade do SDK diff bot على عدم الانجراف. |
| `TRACE-CONFIG-DELTA` | 2026-03-01 12:00-12:30 | حل | تم حذف TLS مرة أخرى أثناء أو إعادة تشغيل Q2؛ o حزمة القياس عن بعد لتسجيل `NEXUS-REH-2026Q2` أو تجزئة ملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` (الإصدار `artifacts/nexus/tls_profile_rollout_2026q2/`) وعدم وجود أي مخالفات. |
| `TRACE-MULTILANE-CANARY` | 2026-05-05 09:12-10:14 | أبروفادو | بذرة عبء العمل `NEXUS-REH-2026Q2`؛ حزمة القياس عن بعد + البيان/الملخص في `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة 912-936) مع جدول الأعمال في `artifacts/nexus/rehearsals/2026q2/`. |

يجب أن تضيف الأشهر الثلاثة المستقبلية خطوطًا جديدة وتتحرك كإدخالات ختامية لتكملة عندما تنمو اللوحة على مدار الأشهر الثلاثة الحالية. يرجع هذا المرجع إلى علاقات التتبع أو عمليات الإدارة باستخدام `#quarterly-routed-trace-audit-schedule`.

## التخفيف من حدة العناصر المتراكمة

| العنصر | وصف | المالك | ألفو | الحالة / الملاحظات |
|------|-------------|--------|----------------|
| `NEXUS-421` | قم بإنهاء نشر ملف TLS الذي تم حذفه خلال `TRACE-CONFIG-DELTA`، والتقط الأدلة وأعد تشغيلها واحفظ سجل التخفيف. | @release-eng، @sre-core | جانيلا تتبع المسار للربع الثاني من عام 2026 | تسجيل - تم التقاط تجزئة الملف TLS `1fa0bd5974a78d680de68e744eab837e4328668d6aab8de1489c3fc3b5a0dbeb` في `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` + `.sha256`؛ أو قم بإعادة التشغيل لتأكيد عدم وجود أي خطأ. |
| `TRACE-MULTILANE-CANARY` الإعدادية | قم ببرمجة أو كتابة الربع الثاني، وربط التركيبات بحزمة القياس عن بعد والتأكد من أن أدوات تطوير البرمجيات (SDK) تستخدم لإعادة الاستخدام أو المساعدة الصالحة. | @telemetry-ops، برنامج SDK | موعد الطائرة 2026-04-30 | كامل - جدول الأعمال محفوظ على `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` مع فتحات/عبء العمل؛ reutilizacao تفعل تسخير anotada لا تعقب. |
| حزمة القياس عن بعد هضم التناوب | قم بتنفيذ `scripts/telemetry/validate_nexus_telemetry_pack.py` قبل كل إصدار/تحرير وتسجيل الملخصات بعد تعقب التكوين. | @القياس عن بعد | من أجل الافراج عن مرشح | كامل - `telemetry_manifest.json` + `.sha256` صادر من `artifacts/nexus/rehearsals/2026q1/` (نطاق الفتحة `912-936`، البذور `NEXUS-REH-2026Q2`)؛ يلخص النسخ ولا يتعقب ولا يوجد فهرس للأدلة. |

## التكامل مع حزمة دلتا التكوين

- `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` يتبع ذلك بملخص الاختلافات الكنسي. عند تشغيل `defaults/nexus/*.toml` الجديد أو تحديثات التكوين، قم بتحديث هذا المتتبع الأول ثم قم بإعادة اكتشاف الأضرار هنا.
- حزم التكوين الموقعة أو حزمة القياس عن بعد. الحزمة، التي تم التحقق منها بواسطة `scripts/telemetry/validate_nexus_telemetry_pack.py`، يجب أن يتم نشرها جنبًا إلى جنب مع دليل تكوين دلتا حتى يتمكن المشغلون من إعادة إنتاج المصنوعات اليدوية المستخدمة خلال B4.
- حزم Iroha مكونة من مسارين دائمين: التكوينات مع `nexus.enabled = false` تم إعادة تجاوز المسار/مساحة البيانات/التوجيه على الأقل مما تم تأهيل الملف Nexus (`--sora`)، بما في ذلك الإزالة بالثانية `nexus.*` das قوالب ذات مسار واحد.
- احتفظ بسجل تصويت الحكومة (GOV-2026-03-19) المرتبط بهذا القدر من التتبع كم لا يمكن للأصوات المستقبلية نسخ التنسيق دون إعادة تصميمه أو طقوس التخصيص.

## المرافقون للانسايو دي لانكامنتو

- `docs/source/runbooks/nexus_multilane_rehearsal.md` التقاط الكناري المستوي وقائمة المشاركين وخطوات التراجع؛ قم بتحديث دليل التشغيل عند طوبولوجيا الممرات أو مصدري القياس عن بعد.
- `docs/source/project_tracker/nexus_rehearsal_2026q1.md` قائمة بكل قطعة أثرية تمت صياغتها خلال أو في 9 أبريل وحتى الآن تتضمن ملاحظات/جدول أعمال إعداد Q2. Adicione ensaios futuros ao mesmo Tracker em vez de abrir Trackers المعزولة للحفاظ على الأدلة الرتيبة.
- المقتطفات العامة تقوم بتصدير OTLP e إلى Grafana (الإصدار `docs/source/telemetry.md`) أثناء توجيه التجميع إلى مصدر المصدر؛ تم تحديث Q1 ورفع حجم الدفعة إلى 256 درجة لتجنب تنبيهات الإرتفاع.
- دليل على CI/اختبارات متعددة المسارات تعيش الآن في `integration_tests/tests/nexus/multilane_pipeline.rs` ويتوقف عن سير العمل `Nexus Multilane Pipeline` (`.github/workflows/integration_tests_multilane.yml`)، ويستبدل بالمرجع المعتمد `pytests/nexus/test_multilane_pipeline.py`؛ قم بصيانة تجزئة `defaults/nexus/config.toml` (`nexus.enabled = true`, blake2b `d69eefa2abb8886b0f3e280e88fe307a907cfe88053b5d60a1d459a5cf8549e1`) من خلال المزامنة مع متعقب تحديث الحزم.

## دورة حياة الممرات في وقت التشغيل

- خطط دورة حياة الممرات في وقت التشغيل قبل التحقق من روابط مساحة البيانات وإيقافها عند التوفيق بين الكورا/التخزين في الكتالوجات الخاطئة، أو الحفاظ على الكتالوج غير المتغير. يمكن للمساعدين ترحيل الممرات في ذاكرة التخزين المؤقت للممرات المحددة حتى لا يقوم دفتر الأستاذ المدمج بإعادة استخدام البراهين القديمة.
- تطبيقات مساعدة للتكوين/دورة الحياة لـ Nexus (`State::apply_lane_lifecycle`, `Queue::apply_lane_lifecycle`) لإضافة/سحب الممرات بدون تجديد؛ يتم إعادة تشغيل التوجيه واللقطات TEU وسجلات البيانات تلقائيًا بعد خطة ناجحة.
- التوجيه للمشغلين: عندما يكون مخططًا صحيحًا، التحقق من وصول مساحات البيانات أو جذور التخزين التي لا يمكن أن تكون كريادوس (جذر بارد متدرج/موجهات سريعة حسب المسار). قاعدة Corrija os caminhos وخيمتها الجديدة; ستتم إعادة تصميم المخططات أو فرق القياس عن بعد للمسار/مساحة البيانات حتى تتمكن لوحات المعلومات من إعادة فتح طوبولوجيا جديدة.

## القياس عن بعد لـ NPoS وأدلة الضغط الخلفي

لقد قمت بتتبع إطلاق المرحلة B من خلال التقاط لقطات قياس عن بعد تحدد أن جهاز تنظيم ضربات القلب NPoS وكاميرات القيل والقال تستمر داخل حدود الضغط الخلفي الخاصة بك. يقوم جهاز التكامل في `integration_tests/tests/sumeragi_npos_performance.rs` بممارسة هذه السيناريوهات وإصدار سيرة ذاتية JSON (`sumeragi_baseline_summary::<scenario>::...`) عند إجراء مقاييس جديدة. تنفيذ localmente com:

```bash
cargo test -p integration_tests sumeragi_npos_performance -- --nocapture
```

قم بتعريف `SUMERAGI_NPOS_STRESS_PEERS`، `SUMERAGI_NPOS_STRESS_COLLECTORS_K` أو `SUMERAGI_NPOS_STRESS_REDUNDANT_SEND_R` لاستكشاف طوبولوجيات الضغط الأكبر؛ تعكس القيم الأساسية ملف التجميع 1 s/`k=3` المستخدم في B4.| السيناريو / الاختبار | كوبرتورا | القياس عن بعد |
| --- | --- | --- |
| `npos_baseline_1s_k3_captures_metrics` | قم بحظر 12 قطعة من الوقت مثل حظر الوقت لتسجيل مغلفات زمن الوصول EMA وعمق الملفات ومقاييس التكرار لإرسالها قبل التسلسل أو حزمة الأدلة. | `sumeragi_phase_latency_ema_ms`، `sumeragi_collectors_k`، `sumeragi_redundant_send_r`، `sumeragi_bg_post_queue_depth*`. |
| `npos_queue_backpressure_triggers_metrics` | قم بملء ملف المعاملات لضمان تأجيل القبول بشكل محدد وتصدير قواطع السعة/التشبع. | `sumeragi_tx_queue_depth`، `sumeragi_tx_queue_capacity`، `sumeragi_tx_queue_saturated`، `sumeragi_pacemaker_backpressure_deferrals_total`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_pacemaker_jitter_within_band` | أظهرت زيادة اهتزاز جهاز تنظيم ضربات القلب ومهلات العرض أن النطاق +/-125 في الدقيقة ومطبق. | `sumeragi_pacemaker_jitter_ms`، `sumeragi_pacemaker_view_timeout_target_ms`، `sumeragi_pacemaker_jitter_frac_permille`. |
| `npos_rbc_store_backpressure_records_metrics` | تعمل حمولات RBC الكبيرة على تخزين الحدود الناعمة/الصلبة لإظهار الجلسات ووحدات تحكم البايت الموجودة فيها، واستردادها وتثبيتها دون تجاوز المخزن. | `sumeragi_rbc_store_pressure`، `sumeragi_rbc_store_sessions`، `sumeragi_rbc_store_bytes`، `sumeragi_rbc_backpressure_deferrals_total`. |
| `npos_redundant_send_retries_update_metrics` | قم بإعادة الإرسال حتى يتم إرسال مقاييس النسبة الزائدة ووحدات التجميع على الهدف مسبقًا، مما يؤدي إلى توصيل القياس عن بعد من البداية إلى النهاية. | `sumeragi_collectors_targeted_current`، `sumeragi_redundant_sends_total`. |
| `npos_rbc_chunk_loss_fault_reports_backlog` | قم بتفريغ أجزاء على فترات زمنية محددة للتحقق من أن مراقبي الأعمال المتراكمة في بلاد الشام يفشلون أثناء تصفية الحمولات بشكل صامت. | `sumeragi_rbc_backlog_sessions_pending`، `sumeragi_rbc_backlog_chunks_total`، `sumeragi_rbc_backlog_chunks_max`. |

قم بإرفاق ملف JSON الذي يقوم بالطباعة جنبًا إلى جنب مع التقاط Prometheus أثناء التنفيذ باستمرار، حيث تطلب الإدارة أدلة على أن إنذارات الضغط الخلفي تتوافق مع طوبولوجيا العمل.

## قائمة المراجعة للتحديث

1. إضافة إضافات جديدة لتتبع التوجيه والانسحاب كمضادات أثناء الأشهر الثلاثة الماضية.
2. قم بتحديث لوحة التخفيف بعد كل مرافقة لـ Alertmanager، حتى يتم إحضار التذكرة.
3. عند إجراء تغييرات على إعدادات التكوين، قم بتحديث المتتبع، وقم بإدراج هذه الملاحظة وقائمة ملخصات حزمة القياس عن بعد بدون طلب سحب فقط.
4. قم بالربط بين كل قطعة جديدة من أدوات البحث/القياس عن بعد حتى تتمكن خرائط الطريق المستقبلية من الرجوع إلى مستند فريد من خلال ملاحظات متفرقة مخصصة.

## فهرس الأدلة

| أتيفو | لوكاليزاكاو | نوتاس |
|-------|----------|-------|
| Relatorio de Auditoria توجيه التتبع (الربع الأول من عام 2026) | `docs/source/nexus_routed_trace_audit_report_2026q1.md` | Fonte canonica da evidencia de المرحلة B1؛ تجول في البوابة على `docs/portal/docs/nexus/nexus-routed-trace-audit-2026q1.md`. |
| تعقب دلتا التكوين | `docs/source/project_tracker/nexus_config_deltas/2026Q1.md` | مع ملخصات الاختلافات TRACE-CONFIG-DELTA وبدء المراجعة وتسجيل التصويت GOV-2026-03-19. |
| خطة علاج القياس عن بعد | `docs/source/nexus_telemetry_remediation_plan.md` | قم بتوثيق حزمة التنبيه ونطاق الكثير من OTLP وحواجز الحماية الخاصة بتصديرها إلى B2. |
| Tracker de ensaio متعدد المسارات | `docs/source/project_tracker/nexus_rehearsal_2026q1.md` | قائمة بالمصنوعات الصادرة بتاريخ 9 أبريل، والبيان/ملخص المدقق، والملاحظات/جدول الأعمال Q2، وأدلة التراجع. |
| بيان/ملخص حزمة القياس عن بعد (الأحدث) | `artifacts/nexus/rehearsals/2026q1/telemetry_manifest.json` (+ `.sha256`) | قم بالتسجيل في نطاق الفتحة 912-936، والبذرة `NEXUS-REH-2026Q2`، وتجزئة القطع الأثرية للحزم الحاكمة. |
| بيان ملف تعريف TLS | `artifacts/nexus/tls_profile_rollout_2026q2/tls_profile_manifest.json` (+ `.sha256`) | تمت الموافقة على تجزئة ملف TLS الذي تم التقاطه خلال الربع الثاني أو إعادة تشغيله؛ استشهد بملاحق تتبع التوجيه. |
| جدول الأعمال TRACE-MULTILANE-CANARY | `artifacts/nexus/rehearsals/2026q2/TRACE-MULTILANE-CANARY-agenda.md` | Notes de Planjamento para o Ensaio Q2 (الصفحة، نطاق الفتحات، بذرة عبء العمل، أصحاب الأصول). |
| Runbook de Ensaio de Lancamento | `docs/source/runbooks/nexus_multilane_rehearsal.md` | قائمة المراجعة التشغيلية للتدريج -> التنفيذ -> التراجع؛ تحديث عند طوبولوجيا الممرات أو توجيه المصدرين. |
| مدقق حزمة القياس عن بعد | `scripts/telemetry/validate_nexus_telemetry_pack.py` | CLI مرجع بيلو ريترو B4؛ يمكنك الحصول على الملخصات من خلال التتبع المستمر الذي تقوم به في حزمة الطين. |
| الانحدار متعدد المسارات | `ci/check_nexus_multilane.sh` + `integration_tests/tests/nexus/multilane_router.rs` | قم بتجربة `nexus.enabled = true` للتكوينات متعددة المسارات، واحتفظ بالتجزئات في كتالوج Sora وقم بتوفير مسارات Kura/merge-log للمسار (`blocks/lane_{id:03}_{slug}`) عبر `ConfigLaneRouter` قبل نشر ملخصات العناصر. |