---
lang: ar
direction: rtl
source: docs/source/sumeragi_aggregators.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: ee79dba673794a3dd4f888d3daf39163a827443bb22d413ab4d7f2e252762293
source_last_modified: "2025-12-26T13:17:08.872635+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/source/sumeragi_aggregators.md -->

# توجيه مجمعات Sumeragi

## نظرة عامة

تسجل هذه المذكرة استراتيجية توجيه collectors ("aggregators") الحتمية التي يستخدمها Sumeragi بعد تحديث عدالة Phase 3. يحسب كل مدقق نفس ترتيب collectors لارتفاع وview بلوك محددين. يلغي التصميم الاعتماد على عشوائية ad hoc ويبقي fan-out الطبيعي للتصويت ضمن حدود قائمة collectors؛ وعند تعذر توفر collectors او تعثر quorum، تعيد اعادة البث المجدولة استخدام اهداف collectors مع fallback الى topologia commit.

## اختيار حتمي

- يعرض الموديول الجديد `sumeragi::collectors` الدالة `deterministic_collectors(topology, mode, k, seed, height, view)` التي تعيد `Vec<PeerId>` قابلا لاعادة الانتاج لزوج `(height, view)`.
- في نمط permissioned يتم اختيار collectors عبر PRF مع seed من حالة PRF/VRF للحقبة. يقوم المساعد باشتقاق ترتيب حتمي لكل `(height, view)` من الطوبولوجيا المعيارية ويستثني القائد. عند غياب seed PRF يعود الى مقطع الذيل المتصل للحفاظ على الحتمية.
- نمط NPoS يستمر باستخدام PRF لكل epoch لكن المساعد يوحد الحساب بحيث يتلقى كل مستدعي نفس الترتيب. يتم اشتقاق seed من عشوائية epoch المقدمة من `EpochManager`.
- يتتبع `CollectorPlan` استهلاك الاهداف المرتبة ويسجل ما اذا تم تفعيل fallback الخاص بـ gossip. تحديثات telemetria (`collect_aggregator_ms`, `sumeragi_redundant_sends_*`, `sumeragi_gossip_fallback_total`) تبين تواتر fallbacks ومدة fan-out المكرر.

## اهداف العدالة

1. **قابلية اعادة الانتاج:** يجب ان تؤدي نفس topologia المدققين ونمط الاجماع و seed PRF والزوج `(height, view)` الى نفس collectors الاساسيين/الثانويين على كل نظير. يخفي المساعد خصوصيات topologia (proxy tail, مدققي Set B) كي يكون الترتيب قابلا للنقل بين المكونات والاختبارات.
2. **التدوير:** اختيار PRF يدور collector الاساسي عبر الارتفاعات وviews في كلا النمطين، لمنع مدقق واحد من Set B من احتكار مهام التجميع. يتم استخدام fallback لمقطع الذيل المتصل فقط عند غياب seed PRF.
3. **قابلية الملاحظة:** تستمر telemetria في الابلاغ عن تعيينات collectors ويصدر مسار fallback تحذيرا عند تفعيل gossip كي يتمكن المشغلون من كشف collectors غير المنضبطين.

## اعادة المحاولة و backoff في gossip

- يحتفظ المدققون بخطة `CollectorPlan` في حالة المقترح؛ تسجل الخطة عدد collectors الذين تم الاتصال بهم وما اذا تم الوصول الى حد fan-out المكرر.
- يتم فهرسة خطط collectors بواسطة `(height, view)` ويعاد تهيئتها عند تغير الموضوع كي لا تعيد محاولات view-change القديمة استخدام الاهداف السابقة.
- يطبق redundant send (`r`) بشكل حتمي عبر التقدم في الخطة. عندما لا تتوفر collectors لزوج `(height, view)` تعود الاصوات الى topologia commit الكاملة (باستثناء الذات) لتجنب deadlock.
- عند تعثر quorum، يعيد مسار اعادة الجدولة بث الاصوات المخزنة عبر خطة collectors، ويعود الى topologia commit عندما تكون collectors فارغة او محلية فقط او دون quorum. هذا يوفر fallback "gossip" محدودا دون دفع كلفة broadcast كاملة في المسار السريع المستقر.
- كل اسقاط لمقترح بسبب بوابة locked QC يزيد `block_created_dropped_by_lock_total`; مسارات فشل تحقق header ترفع `block_created_hint_mismatch_total` و `block_created_proposal_mismatch_total`، ما يساعد المشغلين على ربط fallbacks المتكررة بمشاكل صحة leader. لقطة `/v1/sumeragi/status` تصدر ايضا احدث hashes لـ Highest/Locked QC حتى تربط لوحات المتابعة قمم الاسقاط بـ hashes بلوكات محددة.

## ملخص التنفيذ

- الموديول العام الجديد `sumeragi::collectors` يستضيف `CollectorPlan` و `deterministic_collectors` لكي تتمكن الاختبارات على مستوى crate والاختبارات التكاملية من التحقق من خصائص العدالة دون تشغيل ممثل الاجماع الكامل.
- يعيش `CollectorPlan` في حالة مقترح Sumeragi ويعاد ضبطه عند اكتمال خط انابيب المقترح.
- يبني `Sumeragi` خطط collectors عبر `init_collector_plan` ويستهدف collectors عند اصدار اصوات availability/precommit. تعود اصوات availability و precommit الى topologia commit عندما تكون collectors فارغة او محلية فقط او دون quorum، وتعود اعادات البث تحت نفس الشروط.
- تتحقق الاختبارات الوحدوية والتكاملية من حتمية PRF واختيار fallback وانتقالات حالة backoff.

## اعتماد المراجعة

- Reviewed-by: Consensus WG
- Reviewed-by: Platform Reliability WG

</div>
