---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/log.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-سجل
العنوان: مجلة التغذية الراجعة والقياس عن بعد W1
Sidebar_label: المجلة W1
الوصف: توافق القائمة ونقاط القياس عن بعد وملاحظات المراجعين من أجل العرض الأول للشراكة الغامضة.
---

تحافظ هذه المجلة على قائمة الدعوات ونقاط التفتيش الخاصة بالقياس عن بعد وتعليقات المراجعين من أجلها
**شركاء المعاينة W1** الذين يرافقون مستندات القبول في
[`preview-feedback/w1/plan.md`](./plan.md) ودخول متتبع الغموض في
[`../../preview-invite-tracker.md`](../../preview-invite-tracker.md). Mettez-le a jour quand uneدعوة estمبعوثة,
عندما يتم تسجيل لقطة القياس عن بعد، أو عندما تتم تجربة عنصر التعليقات حتى يتمكن المراجعون من التحكم بشكل فعال
استمتع بالطلبات بدون إرسال التذاكر بعد التذاكر الخارجية.

## قائمة المجموعة

| معرف الشريك | تذكرة الطلب | التجمع الوطني الديمقراطي | دعوة المبعوث (UTC) | تسجيل الدخول / تسجيل الدخول المميز (UTC) | النظام الأساسي | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| شريك-w1-01 | `DOCS-SORA-Preview-REQ-P01` | موافق 2025-04-03 | 2025-04-12 15:00 | 2025-04-12 15:11 | تيرمين 2025-04-26 | سورافس-المرجع-01؛ ركز على Preuves de Parite de Docs Orchestra. |
| شريك-w1-02 | `DOCS-SORA-Preview-REQ-P02` | موافق 2025-04-03 | 2025-04-12 15:03 | 2025-04-12 15:15 | تيرمين 2025-04-26 | سورافس-المرجع-02؛ صالحة للروابط المتقاطعة Norito/telemetrie. |
| شريك-w1-03 | `DOCS-SORA-Preview-REQ-P03` | موافق 2025-04-04 | 2025-04-12 15:06 | 2025-04-12 15:18 | تيرمين 2025-04-26 | سورافس-المرجع-03؛ تنفيذ تدريبات تجاوز الفشل متعدد المصادر. |
| شريك-w1-04 | `DOCS-SORA-Preview-REQ-P04` | موافق 2025-04-04 | 2025-04-12 15:09 | 2025-04-12 15:21 | تيرمين 2025-04-26 | توري-int-01; مراجعة كتاب الطبخ Torii `/v1/pipeline` + جربه. |
| شريك-w1-05 | `DOCS-SORA-Preview-REQ-P05` | موافق 2025-04-05 | 2025-04-12 15:12 | 2025-04-12 15:23 | تيرمين 2025-04-26 | توري-int-02; مرافقة لرحلة الالتقاط اليومية جربها (docs-preview/w1 #2). |
| شريك-w1-06 | `DOCS-SORA-Preview-REQ-P06` | موافق 2025-04-05 | 2025-04-12 15:15 | 2025-04-12 15:26 | تيرمين 2025-04-26 | SDK-شريك-01; كتب الطبخ التعليقات JS/Swift + التعقل يتحقق من جسر ISO. |
| شريك-w1-07 | `DOCS-SORA-Preview-REQ-P07` | موافق 2025-04-11 | 2025-04-12 15:18 | 2025-04-12 15:29 | تيرمين 2025-04-26 | SDK-شريك-02; صلاحية الامتثال 11-04-2025، قم بالتركيز على ملاحظات الاتصال/القياس عن بعد. |
| شريك-w1-08 | `DOCS-SORA-Preview-REQ-P08` | موافق 2025-04-11 | 2025-04-12 15:21 | 2025-04-12 15:33 | تيرمين 2025-04-26 | بوابة العمليات-01؛ تدقيق دليل du ops gate + Flux proxy جرب إخفاء الهوية. |

قم بإخطار **دعوة المبعوث** و **التأكيد** على أن البريد الإلكتروني يتم إرساله.
احصل على الساعات اللازمة للتخطيط بالتوقيت العالمي المنسق (UTC) في الخطة W1.

## القياس عن بعد لنقاط التفتيش

| هوروداتاج (UTC) | لوحات العدادات / المجسات | مسؤول | النتيجة | قطعة أثرية |
| --- | --- | --- | --- | --- |
| 2025-04-06 18:05 | `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals` | مستندات/DevRel + Ops | توت فير | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250406` |
| 2025-04-06 18:20 | النص `npm run manage:tryit-proxy -- --stage preview-w1` | العمليات | نظموا | `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log` |
| 2025-04-12 14:45 | لوحات المعلومات ci-dessus + `probe:portal` | مستندات/DevRel + Ops | لقطة ما قبل الدعوة، الانحدار aucune | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250412` |
| 2025-04-19 17:55 | لوحات المعلومات ci-dessus + diff de lantence proxy جربها | مستندات/DevRel الرصاص | بيئة نقطة التفتيش صالحة (0 تنبيهات؛ زمن الوصول جربها p95=410 مللي ثانية) | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250419` |
| 2025-04-26 16:25 | لوحات المعلومات ci-dessus + مسبار الطلعة | Docs/DevRel + اتصال الحوكمة | لقطة من الطلعة، صفر تنبيهات ثابتة | `artifacts/docs_preview/W1/preview-2025-04-12/grafana/20250426` |

يتم إعادة تجميع ساعات العمل اليومية (2025-04-13 -> 2025-04-25) وتصدير NDJSON + PNG sous
`artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/` بأسماء الملفات
`docs-preview-integrity-<date>.json` ويلتقط المراسلات.

## تسجيل الملاحظات والمشكلات

استخدم هذه اللوحة لاستئناف إحصائيات المراجعين. Liez chaque entry au Ticket GitHub/discuss
Ainsi qu'au صيغة التقاط الهيكل عبر
[`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md).

| مرجع | شديد | مسؤول | النظام الأساسي | ملاحظات |
| --- | --- | --- | --- | --- |
| `docs-preview/w1 #1` | منخفض | مستندات-core-02 | القرار 2025-04-18 | توضيح صياغة التنقل جربها + الشريط الجانبي الإضافي (`docs/source/sorafs/tryit.md` منذ يوم مع التسمية الجديدة). |
| `docs-preview/w1 #2` | منخفض | مستندات-core-03 | القرار 2025-04-19 | Capture Try it + legende rafraichies selon la request؛ قطعة أثرية `artifacts/docs_preview/W1/preview-2025-04-12/feedback/partner-w1-05/screenshot-diff.png`. |
| - | معلومات | مستندات/DevRel الرصاص | فيرمي | تحتوي التعليقات على أسئلة وأجوبة فريدة؛ يلتقط في كل صيغة Partenaire sous `artifacts/docs_preview/W1/preview-2025-04-12/feedback/`. |

## التحقق من المعرفة والدراسات الاستقصائية Suivi

1. قم بتسجيل درجات الاختبار (cible >=90%) لكل مراجع؛ انضم إلى تصدير ملف CSV إلى مجموعة عناصر الدعوة.
2. اجمع الردود النوعية للاستطلاع الملتقطة عبر قالب التعليقات ونسخها
   `artifacts/docs_preview/W1/preview-2025-04-12/surveys/`.
3. مخطط طلبات الإصلاح لجميع الأشخاص الذين يتابعونهم والمرسل إليهم.

سيحصل المراجعون على >=94% من فحص المعرفة (ملف CSV:
`artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`). هناك طلب علاج
لا داعي لذلك؛ صادرات المسح لكل شريك هي كذلك
`artifacts/docs_preview/W1/preview-2025-04-12/surveys/<partner-id>/summary.json`.

## اختراع التحف

- واصف/المجموع الاختباري لمعاينة الحزمة: `artifacts/docs_preview/W1/preview-2025-04-12/descriptor.json`
- استئناف التحقيق + فحص الارتباط: `artifacts/docs_preview/W1/preview-2025-04-12/preflight-summary.json`
- سجل تغيير الوكيل جربه : `artifacts/docs_preview/W1/preview-2025-04-12/tryit/OPS-TRYIT-147.log`
- قياس الصادرات عن بعد: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/<date>/`
- حزمة ساعات العمل اليومية للقياس عن بعد: `artifacts/docs_preview/W1/preview-2025-04-12/grafana/daily/`
- ردود الفعل على الصادرات + المسح: وضع الملفات حسب المراجعين
  `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`
- التحقق من المعرفة بملف CSV والسيرة الذاتية: `artifacts/docs_preview/W1/preview-2025-04-12/feedback/w1-quiz-scores.csv`

Garder l'inventaire syncyize avec l'issue Tracker. انضم إلى التجزئات عند نسخ القطع الأثرية
تتيح إدارة التذاكر أن يتمكن المدققون من التحقق من الملفات دون الوصول إليها.