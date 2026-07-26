---
lang: ar
direction: rtl
source: docs/examples/android_partner_sla_notes_template.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f1d3e5d2b42f7e6f9c2de4f2be56b6994b4b88b109f70edc7e6f04ec0f3465ac
source_last_modified: "2025-11-12T08:32:28.349523+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- الترجمة العربية لـ docs/examples/android_partner_sla_notes_template.md -->

# ملاحظات اكتشاف SLA لشريك Android - قالب

استخدم هذا القالب لكل جلسة اكتشاف SLA الخاصة بـ AND8. خزّن النسخة المعبأة تحت
`docs/source/sdk/android/partner_sla_sessions/<partner>/<date>/minutes.md`
وارفق artefacts الداعمة (اجابات الاستبيان، الاقرارات، المرفقات) في نفس الدليل.

```
Partner: <Name>                      Date: <YYYY-MM-DD>  Time: <UTC>
Primary contact(s): <names, roles, email>
Android attendees: <Program Lead / Partner Eng / Support Eng / Compliance>
Meeting link / ticket: <URL or ID>
```

## 1. جدول الاعمال والسياق

- هدف الجلسة (نطاق تجريبي، نافذة اصدار، توقعات التليمترية).
- المستندات المرجعية المشتركة قبل المكالمة (support playbook، تقويم الاصدار،
  لوحات التليمترية).

## 2. نظرة عامة على الحمل

| الموضوع | ملاحظات |
|---------|---------|
| الاحمال المستهدفة / السلاسل | |
| حجم المعاملات المتوقع | |
| نوافذ عمل حرجة / فترات blackout | |
| الانظمة التنظيمية (GDPR، MAS، FISC، الخ) | |
| اللغات المطلوبة / التوطين | |

## 3. مناقشة SLA

| فئة SLA | توقع الشريك | فرق عن baseline? | الاجراء المطلوب |
|---------|--------------|------------------|-----------------|
| اصلاح حرج (48 h) | | Yes/No | |
| شدة عالية (5 business days) | | Yes/No | |
| صيانة (30 days) | | Yes/No | |
| اشعار cutover (60 days) | | Yes/No | |
| وتيرة تواصل الحوادث | | Yes/No | |

وثّق اي بنود SLA اضافية يطلبها الشريك (مثل جسر هاتفي مخصص، تصدير تليمترية اضافي).

## 4. متطلبات التليمترية والوصول

- احتياجات الوصول الى Grafana / Prometheus:
- متطلبات تصدير logs/traces:
- توقعات الادلة offline او الدوسيه:

## 5. ملاحظات الامتثال والقانون

- متطلبات الاشعار القضائي (النص + التوقيت).
- جهات الاتصال القانونية المطلوبة لتحديثات الحوادث.
- قيود اقامة البيانات / متطلبات التخزين.

## 6. القرارات وعناصر العمل

| العنصر | المالك | الاستحقاق | ملاحظات |
|--------|--------|-----------|---------|
| | | | |

## 7. الاقرار

- اقر الشريك بـ SLA الاساسي؟ (Y/N)
- طريقة الاقرار بالمتابعة (email / ticket / signature):
- ارفق بريد التأكيد او محضر الاجتماع في هذا الدليل قبل الاغلاق.

</div>
