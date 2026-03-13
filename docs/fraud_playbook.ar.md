---
lang: ar
direction: rtl
source: docs/fraud_playbook.md
status: complete
translator: manual
source_hash: b3253ff47a513529c1dba6ef44faf38087ee0e5f5520f8c3fd770ab8d36c7786
source_last_modified: "2025-11-02T04:40:28.812006+00:00"
translation_last_reviewed: 2025-11-14
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/fraud_playbook.md (Fraud Governance Playbook) -->

# دليل حوكمة الاحتيال

يلخّص هذا المستند البنية الأساسية المطلوبة لمكدّس الاحتيال الخاص بـ PSP
بينما لا تزال خدمات الـ microservices وـ SDKs الكاملة قيد التطوير النشط. يوضّح
توقعات التحليلات، وسير عمل المدقّقين، وإجراءات الطوارئ (fallback) بحيث يمكن
للتطبيقات القادمة أن تتصل بالـ ledger بأمان.

## نظرة عامة على الخدمات

1. **بوابة الـ API (API Gateway)**: تستقبل payloads متزامنة من النوع
   `RiskQuery`، تُمرّرها إلى خدمة تجميع الـ features وتعيد ردود
   `FraudAssessment` إلى مسارات الـ ledger. يجب أن تكون عالية التوافر
   (active‑active)؛ استخدم أزواجًا إقليمية مع hashing حتمي لتجنّب انحراف
   التوزيع بين الطلبات.
2. **تجميع الـ Features**: تُركِّب متجهات (vectors) من الـ features بغرض
   الـ scoring. لا تُصدر إلا تجزئات `FeatureInput`؛ تبقى الـ payloads الحساسة
   خارج السلسلة (off‑chain). يجب أن تنشر منظومة الـ observability
   histogram للكمون، وgauge لعمق الصف، وعدّادات replays لكل tenant.
3. **محرك المخاطر (Risk Engine)**: يُقيِّم القواعد/النماذج ويُنتج مخرجات
   `FraudAssessment` حتمية. احرص على أن يكون ترتيب تنفيذ القواعد ثابتًا، وأن
   تُحفظ سجلات تدقيق (audit logs) لكل معرّف تقييم (assessment ID).

## التحليلات وترقية النماذج

- **كشف الشذوذ (Anomaly Detection)**: الحفاظ على job بث (streaming) يرصد
  الانحرافات في معدلات القرارات لكل tenant. تُغذَّى التنبيهات في لوحة
  حوكمة الاحتيال، وتُخزَّن ملخّصات للاجتماعات ربع السنوية.
- **تحليل الرسوم البيانية (Graph Analysis)**: تشغيل traversals لرسوم
  بيانية ليلاً على exports علائقية من أجل اكتشاف عناقيد التواطؤ. تُصدَّر
  النتائج إلى بوابة الحوكمة عبر `GovernanceExport` مع مراجع إلى الأدلة
  الداعمة.
- **استيعاب التغذية المرتدة (Feedback Ingestion)**: تنسيق نتائج المراجعة
  اليدوية وتقارير الـ chargeback، وتحويلها إلى deltas في الـ features
  وإدراجها في مجموعات بيانات التدريب. نشر مقاييس حالة الاستيعاب لكي يتمكن
  فريق المخاطر من اكتشاف الـ feeds المتوقفة.
- **خط ترقية النماذج (Model Promotion Pipeline)**: أتمتة تقييم النماذج
  المرشّحة (مقاييس offline، scoring تجريبي ـ canary، الاستعداد للـ rollback).
  يجب أن تصدر كل ترقية مجموعة عينات `FraudAssessment` موقّعة، وأن تحدّث
  الحقل `model_version` في `GovernanceExport`.

## سير عمل المدقّق

1. أخذ snapshot لأحدث `GovernanceExport` والتحقق من أن قيمة
   `policy_digest` تطابق الـ manifest الذي قدّمه فريق المخاطر.
2. التحقّق من أن تجميعات القواعد تتطابق مع مجاميع القرارات على جانب
   الـ ledger ضمن نافذة العينة.
3. مراجعة تقارير كشف الشذوذ وتحليل الرسوم البيانية بحثًا عن المشاكل
   المعلّقة. توثيق مسارات التصعيد (escalations) والمالكين المتوقعين لخُطط
   المعالجة.
4. توقيع checklist المراجعة وأرشفتها. تخزين الـ artifacts المرمَّزة بـ Norito
   في بوابة الحوكمة لضمان إمكانية إعادة الإنتاج.

## خطط الطوارئ (Fallback Playbooks)

- **تعطّل المحرك**: إذا أصبح محرك المخاطر غير متاح لمدة تزيد عن 60 ثانية،
  يجب على الـ gateway الانتقال إلى وضع مراجعة فقط (review‑only)،
  وإرجاع `AssessmentDecision::Review` لكل الطلبات مع تنبيه المشغّلين.
- **فجوة في التليمترية (Telemetry Gap)**: عند تأخّر المقاييس أو الـ traces
  (انقطاع لأكثر من 5 دقائق)، يجب إيقاف ترقيات النماذج التلقائية وإشعار
  مهندس المناوبة.
- **تراجع أداء النموذج (Model Regression)**: إذا أظهر feedback ما بعد
  النشر ارتفاعًا في خسائر الاحتيال، يجب التراجع إلى حزمة النموذج الموقّعة
  السابقة وتحديث الـ roadmap بخطوات تصحيحية.

## اتفاقيات مشاركة البيانات

- الحفاظ على ملاحق (appendices) خاصة بكل اختصاص قضائي تغطي الاحتفاظ
  بالبيانات، والتشفير، وـ SLA للإبلاغ عن الاختراقات. يجب أن يوقّع
  الشركاء على الملحق قبل تلقّي exports من نوع `FraudAssessment`.
- توثيق ممارسات تقليل البيانات (data minimization) لكل تكامل، مثل تجزئة
  معرفات الحسابات أو تقصير أرقام البطاقات.
- تجديد الاتفاقيات سنويًا أو عند تغيّر المتطلبات التنظيمية.

## تمارين Red‑Team

- تُنفَّذ التمارين كل ثلاثة أشهر. الجلسة التالية مجدولة بتاريخ
  **2026‑01‑15** مع سيناريوهات تشمل تسميم الـ features، تضخيم الـ replays
  ومحاولات تزوير التواقيع.
- تُسجَّل النتائج في نموذج تهديدات الاحتيال، وتُضاف المهام الناتجة إلى
  `roadmap.md` ضمن مسار العمل
  «Fraud & Telemetry Governance Loop».

## مخططات الـ API (API Schemas)

يُقدّم الـ gateway الآن أغلفة JSON ملموسة تتوافق واحد‑لواحد مع أنواع
Norito المطبقة في `crates/iroha_data_model::fraud`:

- **استلام طلبات المخاطر (Risk intake)** – تستقبل
  `POST /v2/fraud/query` بنية `RiskQuery`:
  - `query_id` (`[u8; 32]`، مُمثَّلة بصيغة hex)
  - `subject` (`AccountId`, `domainless encoded literal; canonical I105 only (i105-default `sora...` rejected)`)
  - `operation` (enum مُعنون يتطابق مع `RiskOperation`؛ الحقل `type` في
    JSON يعكس نوع الـ enum)
  - `related_asset` (`AssetId` اختياري)
  - `features` (مصفوفة `{ key: String, value_hash: hex32 }` مشتقة من
    `FeatureInput`)
  - `issued_at_ms` (`u64`)
  - `context` (`RiskContext`؛ يحتوي على `tenant_id`، و`session_id` اختياري،
    و`reason` اختياري)
- **قرار المخاطر (Risk decision)** – تستقبل
  `POST /v2/fraud/assessment` الـ payload من النوع `FraudAssessment`
  (وهو أيضًا ما يُستخدَم في exports الحوكمة):
  - `query_id`, `engine_id`, `risk_score_bps`, `confidence_bps`,
    `decision` (enum `AssessmentDecision`)، `rule_outcomes`
    (مصفوفة `{ rule_id, score_delta_bps, rationale? }`)
  - `generated_at_ms`
  - `signature` (قيمة base64 اختيارية تغلّف تقييم `FraudAssessment`
    المرمَّز بـ Norito)
- **تصدير الحوكمة (Governance export)** – يعيد `GET /v2/fraud/governance/export`
  البنية `GovernanceExport` عند تفعيل الـ feature المسماة `governance`، مع
  تجميع الإعدادات الفعّالة، وآخر enactment، وإصدار النموذج، وـ policy
  digest، وhistogram من نوع `DecisionAggregate`.

تضمن اختبارات round‑trip في
`crates/iroha_data_model/src/fraud/types.rs` بقاء هذه المخططات متوافقة
ثنائيًا مع كودك Norito؛ بينما يختبر
`integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
مسار الإدخال/القرار (intake/decision) كاملاً من البداية إلى النهاية.

## مراجع SDK الخاصة بـ PSP

تتبع نماذج اللغات التالية أمثلة التكامل الموجّهة إلى PSP:

- **Rust** – يستخدم
  `integration_tests/tests/fraud_monitoring_requires_assessment_bands.rs`
  عميل `iroha` (ضمن الـ workspace) لبناء metadata من نوع `RiskQuery` والتحقق
  من حالات النجاح/الفشل في القبول.
- **TypeScript** – يشرح
  `docs/source/governance_api.md` سطح REST الذي يستهلكه
  بوابة Torii الخفيفة المستخدمة في لوحة تحكم الـ PSP التجريبية؛ ويعيش
  العميل المبرمج في
  `scripts/ci/schedule_fraud_scoring.sh` لاستخدامه في تجارب الـ smoke.
- **Swift و Kotlin** – تستند الـ SDKs الحالية (`IrohaSwift` والمراجع في
  `crates/iroha_cli/docs/multisig.md`) إلى Hooks Torii اللازمة لإلحاق الحقول
  `fraud_assessment_*`. تُتابع الـ helpers الخاصة بـ PSP ضمن الـ milestone
  «Fraud & Telemetry Governance Loop» في `status.md` وتعيد استخدام
  builders المعاملات الخاصة بهذه الـ SDKs.

ستُحافظ هذه المراجع على التزامن مع بوابة الـ microservice بحيث يكون لدى
منفّذي PSP دومًا مخطط محدث ومسار كود نموذجي لكل لغة مدعومة.

</div>

