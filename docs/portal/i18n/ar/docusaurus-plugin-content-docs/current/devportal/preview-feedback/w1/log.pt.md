---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-سجل
العنوان: سجل التعليقات والقياس عن بعد W1
Sidebar_label: سجل W1
الوصف: قائمة مجمعة ونقاط تفتيش القياس عن بعد وملاحظات المراجعين للعرض الأول عند معاينة الأجزاء.
---

هذا السجل يعتني بقائمة المكالمات ونقاط التفتيش للقياس عن بعد وتعليقات المراجعين
**معاينة العروض W1** المصاحبة لمهام التناول
[`preview-feedback/w1/plan.md`](./plan.md) وإدخال المتعقب من أجلك
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). قم بالتحديث عندما تدعو إلى إرسالها،
لقطة من القياس عن بعد للتسجيل أو عنصر من التعليقات لـ Triado حتى يتمكن المراجعون من إدارة الأمور
قم بإعادة إنتاج الأدلة دون تتبع التذاكر الخارجية.

## قائمة كورتي

| معرف الشريك | تذكرة التماس | NDA recebido | كونفيت إنفيادو (UTC) | تسجيل الدخول إلى Ack/primeiro (UTC) | الحالة | نوتاس |
| --- | --- | --- | --- | --- | --- | --- |
| شريك-w1-01 | `DOCS-SORA-Preview-REQ-P01` | موافق 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | الخاتمة 2025-04-26 | سورافس-المرجع-01؛ ركز على أدلة إثبات مستندات المنسق. |
| شريك-w1-02 | `DOCS-SORA-Preview-REQ-P02` | موافق 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | الخاتمة 2025-04-26 | سورافس-المرجع-02؛ وصلات متقاطعة صالحة Norito/telemetria. |
| شريك-w1-03 | `DOCS-SORA-Preview-REQ-P03` | موافق 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | الخاتمة 2025-04-26 | سورافس-المرجع-03؛ تنفيذ تدريبات تجاوز الفشل متعدد المصادر. |
| شريك-w1-04 | `DOCS-SORA-Preview-REQ-P04` | موافق 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | الخاتمة 2025-04-26 | توري-int-01; مراجعة كتاب الطبخ Torii `/v2/pipeline` + جربه. |
| شريك-w1-05 | `DOCS-SORA-Preview-REQ-P05` | موافق 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | الخاتمة 2025-04-26 | توري-int-02; acompanhou a atualizacao do Screenshot جربه (docs-preview/w1 #2). |
| شريك-w1-06 | `DOCS-SORA-Preview-REQ-P06` | موافق 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | الخاتمة 2025-04-26 | SDK-شريك-01; ردود الفعل على كتب الطبخ JS/Swift + عمليات التحقق من السلامة من خلال جسر ISO. |
| شريك-w1-07 | `DOCS-SORA-Preview-REQ-P07` | موافق 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | الخاتمة 2025-04-26 | SDK-شريك-02; تمت الموافقة على الامتثال بتاريخ 11-04-2025، وقم بالتركيز على ملاحظات Connect/telemetria. |
| شريك-w1-08 | `DOCS-SORA-Preview-REQ-P08` | موافق 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | الخاتمة 2025-04-26 | بوابة العمليات-01؛ Auditou o guia ops do gate + Fluxo anonimo do proxy جربه. |

احصل على الطوابع الزمنية من **Convite Envoyado** و **Ack** حتى تتمكن من مراسلتنا عبر البريد الإلكتروني من أجل إرسالها.
Ancore os horarios no cronograma UTC محدد في خطة W1.

## نقاط التفتيش للقياس عن بعد

| الطابع الزمني (التوقيت العالمي) | لوحات العدادات / المجسات | الرد | النتيجة | ارتيفاتو |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals` | مستندات/DevRel + Ops | تودو فيردي | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | نسخة من `npm run manage:tryit-proxy -- --stage preview-w1` | العمليات | نظموا | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | لوحات المعلومات أعلى + `probe:portal` | مستندات/DevRel + Ops | لقطة دعوة مسبقة، بدون تراجع | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | لوحات المعلومات أعلاه + فرق زمن الوصول للوكيل جربها | مستندات/DevRel الرصاص | نقطة تفتيش أفضل (0 تنبيهات؛ وقت الاستجابة جربها p95=410 مللي ثانية) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | لوحات العدادات هنا + مسبار صيدا | Docs/DevRel + اتصال الحوكمة | لقطة سعيدة، صفر تنبيهات معلقة | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

كما amostras diarias de ساعات العمل (2025-04-13 -> 2025-04-25) estao agrupadas como Exports NDJSON + PNG em
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` بأسماء الملف
`docs-preview-integrity-<date>.json` ولقطات الشاشة للمراسلين.

## سجل المشكلات المتعلقة بالتعليقات

استخدم هذه اللوحة لاستئناف إرسال المراجعين. Vincule cada intrada ao Ticket GitHub/discuss
مزيد من الصيغة تم التقاطها عبر
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| المرجعية | قطع | الرد | الحالة | نوتاس |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | منخفض | مستندات-core-02 | حل 2025-04-18 | قم بإعلان صياغة التنقل جربها + قم بإضافة الشريط الجانبي (`docs/source/sorafs/tryit.md` تم تحديثه بملصق جديد). |
| `docs-preview/w1 #2` | منخفض | مستندات-core-03 | حل 2025-04-19 | لقطة الشاشة: جربها + أسطورة تم تحديثها وفقًا للطلب؛ ارتيفاتو `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | معلومات | مستندات/DevRel الرصاص | فيشادو | تعليقات باقية على الأسئلة والأجوبة؛ لم يتم التقاط صيغة ردود الفعل من كل قطعة من `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## مرافقة المعرفة والتحقق من الدراسات الاستقصائية

1. قم بالتسجيل كملاحظات في الاختبار (الوصفية > = 90%) لكل مراجع؛ يتم تصدير ملحق ملف CSV إلى مجموعة من أدوات الدعوة.
2. احصل على ردود نوعية من خلال استطلاعات الرأي دون قالب التعليقات وشاركها
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. جدول مكالمات الإصلاح حتى يتم إنهاء الحد والتسجيل هنا.

مراجعو Todos os oito marcaram >=94% لم يتم التحقق من المعرفة (CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). نينهوما معالجة العلاج
ضروري؛ صادرات المسح لكل قطعة من الحياة
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## مخزون التحف

- واصف/المجموع الاختباري لمعاينة الحزمة: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- ملخص المسبار + فحص الارتباط: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- Log de mudanca do proxy جربه: `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- صادرات القياس عن بعد: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- حزمة مذكرات القياس عن بعد لساعات العمل: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- تصدير ردود الفعل + المسح: تلوين المعكرونة للمراجعة
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- ملخص CSV وفحص المعرفة: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

قم بحفظ المخزون المتزامن مع إصدار المتتبع. قم بإضافة تجزئات لنسخ المصنوعات لتذكرة الإدارة
حتى يتمكن المدققون من التحقق من الملفات دون الوصول إلى Shell.