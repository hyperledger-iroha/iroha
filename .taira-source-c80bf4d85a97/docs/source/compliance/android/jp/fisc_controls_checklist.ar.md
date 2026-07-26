---
lang: ar
direction: rtl
source: docs/source/compliance/android/jp/fisc_controls_checklist.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 2d8b4c90c94dddd8118fcb9c55f07c25000c6dab1f8d239570402023ab89e844
source_last_modified: "2026-01-03T18:07:59.237724+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# قائمة التحقق من ضوابط أمان FISC — Android SDK

| المجال | القيمة |
|-------|-------|
| النسخة | 0.1 (2026-02-12) |
| النطاق | أدوات مشغل Android SDK + المستخدمة في عمليات النشر المالي اليابانية |
| أصحاب | الامتثال والشؤون القانونية (دانيال بارك)، قائد برنامج Android |

## مصفوفة التحكم

| مراقبة FISC | تفاصيل التنفيذ | الأدلة / المراجع | الحالة |
|--------------|--------------------------------------|-------|--------|
| ** سلامة تكوين النظام ** | يفرض `ClientConfig` التجزئة الواضحة والتحقق من صحة المخطط والوصول إلى وقت التشغيل للقراءة فقط. تؤدي حالات فشل إعادة تحميل التكوين إلى إصدار أحداث `android.telemetry.config.reload` الموثقة في دليل التشغيل. | `java/iroha_android/src/main/java/org/hyperledger/iroha/android/client/ClientConfig.java`; `docs/source/android_runbook.md` §1–2. | ✅ تم التنفيذ |
| ** التحكم في الوصول والمصادقة ** | تحترم SDK سياسات Torii TLS والطلبات الموقعة `/v1/pipeline`؛ مرجع مسارات عمل المشغل دعم دليل التشغيل §4–5 للتصعيد بالإضافة إلى تجاوز البوابة عبر عناصر Norito الموقعة. | `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_redaction.md` (تجاوز سير العمل). | ✅ تم التنفيذ |
| **إدارة مفاتيح التشفير** | يضمن مقدمو خدمة StrongBox المفضلون والتحقق من صحة الشهادات وتغطية مصفوفة الجهاز امتثال KMS. تم أرشفة مخرجات مجموعة التصديق تحت `artifacts/android/attestation/` وتتبعها في مصفوفة الاستعداد. | `docs/source/sdk/android/key_management.md`; `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`; `scripts/android_strongbox_attestation_ci.sh`. | ✅ تم التنفيذ |
| ** التسجيل والمراقبة والاحتفاظ ** | تعمل سياسة تنقيح القياس عن بعد على تجزئة البيانات الحساسة، وتجميع سمات الجهاز، وفرض الاحتفاظ (نوافذ 7/30/90/365 يومًا). يصف دعم Playbook §8 حدود لوحة المعلومات؛ التجاوزات المسجلة في `telemetry_override_log.md`. | `docs/source/sdk/android/telemetry_redaction.md`; `docs/source/android_support_playbook.md`; `docs/source/sdk/android/telemetry_override_log.md`. | ✅ تم التنفيذ |
| **إدارة العمليات والتغيير** | يقوم إجراء تحويل GA (دعم قواعد اللعبة §7.2) بالإضافة إلى `status.md` بتحديث جاهزية إصدار المسار. إصدار الأدلة (حزم SBOM، Sigstore) المرتبطة عبر `docs/source/compliance/android/eu/sbom_attestation.md`. | `docs/source/android_support_playbook.md`; `status.md`; `docs/source/compliance/android/eu/sbom_attestation.md`. | ✅ تم التنفيذ |
| ** الاستجابة للحوادث والإبلاغ عنها ** | يحدد دليل التشغيل مصفوفة الخطورة، ونوافذ الاستجابة لاتفاقية مستوى الخدمة، وخطوات إشعار الامتثال؛ تجاوزات القياس عن بعد + تدريبات الفوضى تضمن إمكانية التكرار قبل الطيارين. | `docs/source/android_support_playbook.md` §§4–9; `docs/source/sdk/android/telemetry_chaos_checklist.md`. | ✅ تم التنفيذ |
| **مقر البيانات/التعريب** | يتم تشغيل مجمعات القياس عن بعد لعمليات نشر JP في منطقة طوكيو المعتمدة؛ حزم التصديق StrongBox المخزنة في المنطقة والمشار إليها من تذاكر الشركاء. تضمن خطة الترجمة إتاحة المستندات باللغة اليابانية قبل الإصدار التجريبي (AND5). | `docs/source/android_support_playbook.md` §9؛ `docs/source/sdk/android/developer_experience_plan.md` §5؛ `docs/source/sdk/android/readiness/android_strongbox_device_matrix.md`. | 🈺 قيد التقدم (التعريب مستمر) |

## ملاحظات المراجع

- تحقق من إدخالات مصفوفة الجهاز لجهاز Galaxy S23/S24 قبل تأهيل الشريك المنظم (راجع صفوف مستند الاستعداد `s23-strongbox-a`، `s24-strongbox-a`).
- تأكد من أن جامعي القياس عن بعد في عمليات نشر JP يفرضون نفس منطق الاحتفاظ/التجاوز المحدد في DPIA (`docs/source/compliance/android/eu/gdpr_dpia_summary.md`).
- احصل على تأكيد من المدققين الخارجيين بمجرد مراجعة الشركاء المصرفيين لقائمة المراجعة هذه.