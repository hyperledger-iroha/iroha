---
lang: ar
direction: rtl
source: docs/source/compliance/android/eu/legal_signoff_memo.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 8bb3e19ca5eb661d202b5e3b9cd118207ded277e8ff717e16a342b71e7a67857
source_last_modified: "2026-01-03T18:07:59.200257+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# AND6 نموذج مذكرة التوقيع القانوني للاتحاد الأوروبي

تسجل هذه المذكرة المراجعة القانونية المطلوبة بموجب بند خريطة الطريق **AND6** قبل
يتم تقديم حزمة بيانات الاتحاد الأوروبي (ETSI/GDPR) إلى الجهات التنظيمية. ينبغي للمحامي استنساخ
هذا القالب لكل إصدار، واملأ الحقول أدناه، وقم بتخزين النسخة الموقعة
إلى جانب القطع الأثرية غير القابلة للتغيير المشار إليها في المذكرة.

## ملخص

- **الإصدار/التدريب:** `<e.g., 2026.1 GA>`
- **تاريخ المراجعة:** `<YYYY-MM-DD>`
- **المستشار / المراجع:** `<name + organisation>`
- **النطاق:** `ETSI EN 319 401 security target, GDPR DPIA summary, SBOM attestation`
- **التذاكر المرتبطة:** `<governance or legal issue IDs>`

## قائمة التحقق من القطع الأثرية

| قطعة أثرية | شا-256 | الموقع / الرابط | ملاحظات |
|----------|--------|-----------------|-------|
| `security_target.md` | `<hash>` | `docs/source/compliance/android/eu/security_target.md` + أرشيف الإدارة | تأكيد معرفات الإصدار وتعديلات نموذج التهديد. |
| `gdpr_dpia_summary.md` | `<hash>` | نفس الدليل / مرايا التعريب | تأكد من تطابق مراجع سياسة التنقيح مع `sdk/android/telemetry_redaction.md`. |
| `sbom_attestation.md` | `<hash>` | نفس الدليل + حزمة التوقيع في دلو الأدلة | تحقق من توقيعات CycloneDX + المصدر. |
| صف سجل الأدلة | `<hash>` | `docs/source/compliance/android/evidence_log.csv` | رقم الصف `<n>` |
| حزمة طوارئ الأجهزة والمختبرات | `<hash>` | `artifacts/android/device_lab_contingency/<YYYYMMDD>/*.tgz` | يؤكد بروفة تجاوز الفشل المرتبطة بهذا الإصدار. |

> قم بإرفاق صفوف إضافية إذا كانت الحزمة تحتوي على المزيد من الملفات (على سبيل المثال، Privacy
> الملاحق أو ترجمات DPIA). يجب أن تشير كل قطعة أثرية إلى أنها غير قابلة للتغيير
> تحميل الهدف ومهمة Buildkite التي أنتجته.

## النتائج والاستثناءات

- `None.` *(استبدل بقائمة نقطية تغطي المخاطر المتبقية، وتعوض
  الضوابط، أو إجراءات المتابعة المطلوبة.)*

## موافقة

- **القرار:** `<Approved / Approved with conditions / Blocked>`
- **التوقيع / الطابع الزمني:** `<digital signature or email reference>`
- **أصحاب المتابعة:** `<team + due date for any conditions>`

قم بتحميل المذكرة النهائية إلى مجموعة أدلة الحوكمة، وانسخ SHA-256 إلى
`docs/source/compliance/android/evidence_log.csv`، وربط مسار التحميل فيه
`status.md`. إذا كان القرار "محظورًا"، فقم بالتصعيد إلى التوجيه AND6
خطوات معالجة اللجنة والوثيقة في كل من القائمة الساخنة لخارطة الطريق والقائمة الساخنة
سجل طوارئ مختبر الجهاز.