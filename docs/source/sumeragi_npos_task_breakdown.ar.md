---
lang: ar
direction: rtl
source: docs/source/sumeragi_npos_task_breakdown.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: a1773b8fda6cda00e38b333096bfe5d6f6181c883ece5a62c11a190a09870d29
source_last_modified: "2025-12-12T12:49:53.638997+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/sumeragi_npos_task_breakdown.md -->

## تفصيل مهام Sumeragi + NPoS

توسع هذه المذكرة خارطة الطريق للمرحلة A الى مهام هندسية صغيرة كي نستطيع تنفيذ العمل المتبقي في Sumeragi/NPoS بشكل تدريجي. تتبع علامات الحالة الاصطلاح: `✅` مكتمل، `⚙️` جارٍ العمل، `⬜` لم يبدأ، و `🧪` يحتاج اختبارات.

### A2 - اعتماد الرسائل على مستوى wire
- ✅ اظهار انواع Norito `Proposal`/`Vote`/`Qc` في `BlockMessage` وتشغيل round-trip encode/decode (`crates/iroha_data_model/tests/consensus_roundtrip.rs`).
- ✅ تقييد الاطارات السابقة `BlockSigned/BlockCommitted`؛ تم ضبط مفتاح الهجرة على `false` قبل الايقاف.
- ✅ ازالة مفتاح الهجرة الذي كان يبدل رسائل البلوك القديمة؛ مسار Vote/commit certificate الان هو المسار الوحيد على wire.
- ✅ تحديث مسارات Torii واوامر CLI ومستلمي telemetria لتفضيل لقطات JSON `/v2/sumeragi/*` على الاطارات القديمة.
- ✅ تغطية التكامل تختبر endpoints `/v2/sumeragi/*` حصرا عبر مسار Vote/commit certificate (`integration_tests/tests/sumeragi_vote_qc_commit.rs`).
- ✅ ازالة الاطارات القديمة بعد تحقق التكافؤ الوظيفي واختبارات التشغيل البيني.

### خطة ازالة الاطارات
1. ✅ اجريت اختبارات soak متعددة العقد لمدة 72 h على حزم telemetria وCI؛ اظهرت لقطات Torii انتاجية proposer مستقرة وتشكيل commit certificate دون تراجعات.
2. ✅ تغطية اختبارات التكامل تعمل الان حصرا عبر مسار Vote/commit certificate (`sumeragi_vote_qc_commit.rs`) وتضمن وصول النظراء المختلطين الى اجماع دون الاطارات القديمة.
3. ✅ لم تعد وثائق المشغلين ومساعدة CLI تذكر مسار wire السابق؛ توجيه استكشاف الاعطال يشير الان الى telemetria الخاصة بـ Vote/commit certificate.
4. ✅ تم حذف متغيرات الرسائل ومقاييس telemetria ومخازن commit المعلقة؛ مصفوفة التوافق تعكس الان سطح Vote/commit certificate فقط.

### A3 - فرض المحرك و pacemaker
- ✅ تم فرض ثوابت Lock/Highestcommit certificate داخل `handle_message` (انظر `block_created_header_sanity`).
- ✅ تتبع توافر البيانات يتحقق من hash حمولة RBC عند تسجيل التسليم (`Actor::ensure_block_matches_rbc_payload`) بحيث لا تعامل الجلسات غير المتطابقة كتسليم صحيح.
- ✅ تم توصيل شرط Precommitcommit certificate (`require_precommit_qc`) في الاعدادات الافتراضية واضافة اختبارات سلبية (الافتراضي الان `true`؛ الاختبارات تغطي مسارات gated و opt-out).
- ✅ استبدال heuristics الخاصة بـ redundant-send على مستوى view بوحدات تحكم pacemaker مدعومة بـ EMA (اصبح `aggregator_retry_deadline` مشتقا من EMA الحي ويقود مواعيد redundant send).
- ✅ بوابة تجميع المقترحات على backpressure للطابور (`BackpressureGate` توقف pacemaker عند تشبع الطابور وتسجل deferrals في status/telemetry).
- ✅ تصدر اصوات availability بعد التحقق من المقترح عندما تكون DA مطلوبة (من دون انتظار `DELIVER` المحلي لـ RBC)، ويتم تتبع دليل availability عبر `availability evidence` كاثبات امان بينما يستمر commit دون انتظار. هذا يتجنب الانتظار الدائري بين نقل payload والتصويت.
- ✅ تغطية restart/liveness تختبر الان تعافي RBC بعد cold-start (`integration_tests/tests/sumeragi_da.rs::sumeragi_rbc_session_recovers_after_cold_restart`) واستئناف pacemaker بعد downtime (`integration_tests/tests/sumeragi_npos_liveness.rs::npos_pacemaker_resumes_after_downtime`).
- ✅ اضفنا اختبارات رجعية حتمية لـ restart/view-change تغطي تقارب lock (`integration_tests/tests/sumeragi_lock_convergence.rs`).

### A4 - مسار collectors والاعتمادية العشوائية
- ✅ المساعدات الحتمية لتدوير collectors موجودة في `collectors.rs`.
- ✅ GA-A4.1 - اختيار collectors المدعوم بـ PRF يسجل الان seeds حتمية و height/view في `/status` و telemetria؛ hooks تحديث VRF تمرر السياق بعد commits و reveals. المالكون: `@sumeragi-core`. المتتبع: `project_tracker/npos_sumeragi_phase_a.md` (مغلق).
- ✅ GA-A4.2 - اظهار telemetria مشاركة reveal + اوامر CLI للفحص وتحديث manifests الخاصة بـ Norito. المالكون: `@telemetry-ops`, `@torii-sdk`. المتتبع: `project_tracker/npos_sumeragi_phase_a.md:6`.
- ✅ GA-A4.3 - تقنين تعافي late-reveal واختبارات epochs بدون مشاركة ضمن `integration_tests/tests/sumeragi_randomness.rs` (`npos_late_vrf_reveal_clears_penalty_and_preserves_seed`, `npos_zero_participation_epoch_reports_full_no_participation`)، مع اختبار telemetria لتنظيف العقوبات. المالكون: `@sumeragi-core`. المتتبع: `project_tracker/npos_sumeragi_phase_a.md:7`.

### A5 - اعادة تهيئة مشتركة و evidence
- ✅ بنية evidence وحفظ WSV وعمليات Norito roundtrip تغطي الان double-vote و invalid proposal و invalid commit certificate ومتغيرات double exec مع ازالة تكرار حتمية وتقليص افق (`sumeragi::evidence`).
- ✅ GA-A5.1 - تفعيل joint-consensus (المجموعة القديمة توقع، والجديدة تتفعل في البلوك التالي) مع تغطية تكامل مستهدفة.
- ✅ GA-A5.2 - تحديث وثائق governance وتدفقات CLI الخاصة بـ slashing/jailing مع اختبارات مزامنة mdBook لتثبيت defaults وصياغة evidence horizon.
- ✅ GA-A5.3 - اختبارات evidence للمسارات السلبية (duplicate signer, forged signature, stale epoch replay, mixed manifest payloads) مع fixtures fuzz تضاف وتعمل ليلا لحماية تحقق Norito roundtrip.

### A6 - الادوات والوثائق والتحقق
- ✅ تم تفعيل telemetria/reporting لـ RBC؛ تقرير DA يولد مقاييس حقيقية (بما فيها عدادات eviction).
- ✅ GA-A6.1 - اختبار happy-path لـ NPoS مع VRF و4 peers يعمل الان في CI مع فرض حدود pacemaker/RBC عبر `integration_tests/tests/sumeragi_npos_happy_path.rs`. المالكون: `@qa-consensus`, `@telemetry-ops`. المتتبع: `project_tracker/npos_sumeragi_phase_a.md:11`.
- ✅ GA-A6.2 - التقاط خط اساس اداء NPoS (بلوكات 1 s، k=3) ونشره في `status.md`/وثائق المشغلين مع seeds قابلة لاعادة الانتاج ومصفوفة العتاد. المالكون: `@performance-lab`, `@telemetry-ops`. التقرير: `docs/source/generated/sumeragi_baseline_report.md`. المتتبع: `project_tracker/npos_sumeragi_phase_a.md:12`. تم تسجيل التشغيل الحي على Apple M2 Ultra (24 cores, 192 GB RAM, macOS 15.0) باستخدام الامر في `scripts/run_sumeragi_baseline.py`.
- ✅ GA-A6.3 - تم نشر ادلة troubleshooting للمشغلين حول ادوات RBC/pacemaker/backpressure (`docs/source/telemetry.md:523`)، وصارت مواءمة السجلات تتم عبر `scripts/sumeragi_backpressure_log_scraper.py` كي يتمكن المشغلون من استخراج ازواج pacemaker deferral/missing-availability دون grep يدوي. المالكون: `@operator-docs`, `@telemetry-ops`. المتتبع: `project_tracker/npos_sumeragi_phase_a.md:13`.
- ✅ تمت اضافة سيناريوهات اداء RBC store/chunk-loss (`npos_rbc_store_backpressure_records_metrics`, `npos_rbc_chunk_loss_fault_reports_backlog`)، وتغطية fan-out مكرر (`npos_redundant_send_retries_update_metrics`)، و harness jitter محدود (`npos_pacemaker_jitter_within_band`) لكي تختبر مجموعة A6 deferrals للحدود اللينة في store، و drops حتمية للـ chunks، و telemetria لـ redundant-send، ونطاقات jitter للـ pacemaker تحت الضغط. [integration_tests/tests/sumeragi_npos_performance.rs:633] [integration_tests/tests/sumeragi_npos_performance.rs:760] [integration_tests/tests/sumeragi_npos_performance.rs:800] [integration_tests/tests/sumeragi_npos_performance.rs:639]

### الخطوات التالية الفورية
1. ✅ harness jitter المحدود يختبر مقاييس jitter للـ pacemaker (`integration_tests/tests/sumeragi_npos_performance.rs::npos_pacemaker_jitter_within_band`).
2. ✅ ترقية assertions الخاصة بـ deferral في `npos_queue_backpressure_triggers_metrics` عبر تهيئة ضغط حتمي على RBC store (`integration_tests/tests/sumeragi_npos_performance.rs::npos_queue_backpressure_triggers_metrics`).
3. ✅ توسيع soak لـ `/v2/sumeragi/telemetry` ليغطي epochs طويلة و collectors عدائيين، مع مقارنة snapshots بعدادات Prometheus عبر عدة heights. مغطى بواسطة `integration_tests/tests/sumeragi_telemetry.rs::npos_telemetry_soak_matches_metrics_under_adversarial_collectors`.

تتبع هذه القائمة هنا يبقي `roadmap.md` مركزا على المعالم مع منح الفريق قائمة تحقق حية للانجاز. حدث الادخالات (وضع علامة الاكتمال) عند وصول التغييرات.

</div>
