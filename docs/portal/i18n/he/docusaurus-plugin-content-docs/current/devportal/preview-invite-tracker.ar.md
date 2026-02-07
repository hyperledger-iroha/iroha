---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# متعقب دعوات المعاينة

يسجل هذا المتعقب كل موجة معاينة لبوابة المستندات حتى يتمكن مالكو DOCS-SORA ومراجعو الحوكمة من رؤية اي مجموعة نشطة، من وافق على الدعوات، واي اثار ما زالت بحاجة الى متابعة. حدّثه كلما تم ارسال الدعوات او سحبها او تأجيلها حتى يبقى سجل التدقيق داخل المستودع.

## حالة الموجات

| الموجة | المجموعة | تذكرة المتابعة | الموافقون | الحالة | النافذة المستهدفة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - מתחזקי ליבה** | Maintainers من Docs + SDK يتحققون من تدفق checksum | `DOCS-SORA-Preview-W0` (متتبع GitHub/ops) | Lead Docs/DevRel + Portal TL | مكتمل | Q2 2025 الاسابيع 1-2 | תאריך תאריך 2025-03-25, תאריך תאריך 08-04-2025. |
| **W1 - שותפים** | مشغلو SoraFS ومتكاملو Torii تحت NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | Q2 2025 الاسبوع 3 | الدعوات 2025-04-12 -> 2025-04-26 مع تأكيد جميع الشركاء الثمانية؛ تم حفظ الادلة في [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) وملخص الخروج في [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - קהילה** | قائمة انتظار مجتمعية منتقاة (<=25 في كل مرة) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + מנהל קהילה | مكتمل | Q3 2025 الاسبوع 1 (مبدئي) | الدعوات 2025-06-15 -> 2025-06-29 مع قياس عن بعد اخضر طوال الفترة؛ الادلة والنتائج في [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes בטא** | Beta التمويل/الملاحظة + شريك SDK + داعم للنظام البيئي | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | Q1 2026 الاسبوع 8 | الدعوات 2026-02-18 -> 2026-02-28؛ تم توليد الملخص وبيانات البوابة عبر موجة `preview-20260218` (انظر [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> ملاحظة: اربط كل تذكرة في المتعقب بطلبات المعاينة وارشفها تحت مشروع `docs-portal-preview` حتى تبقى الموافقات قابلة للاكتشاف.

## المهام النشطة (W0)

- تحديث اثار preflight (تشغيل GitHub Actions `docs-portal-preview` بتاريخ 2025-03-24، وتحقق descriptor عبر `scripts/preview_verify.sh` باستخدام ועם `preview-2025-03-24`).
- توثيق خطوط اساس القياس عن بعد (`docs.preview.integrity`، وحفظ لقطة `TryItProxyErrors` في تذكرة W0).
- تثبيت نص التواصل باستخدام [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) مع وسم preview `preview-2025-03-24`.
- تسجيل طلبات الدخول لاوائل خمسة maintainers (تذاكر `DOCS-SORA-Preview-REQ-01` ... `-05`).
- ارسال اول خمس دعوات 2025-03-25 10:00-10:20 UTC بعد سبعة ايام متتالية من القياس الاخضر؛ تم حفظ الايصالات في `DOCS-SORA-Preview-W0`.
- متابعة القياس عن بعد + office hours للمضيف (فحوصات يومية حتى 2025-03-31؛ سجل checkpoints بالاسفل).
- جمع ملاحظات منتصف الموجة / القضايا ووضع الوسم `docs-preview/w0` (انظر [W0 digest](./preview-feedback/w0/summary.md)).
- نشر ملخص الموجة + تأكيدات الخروج (حزمة خروج بتاريخ 2025-04-08؛ انظر [W0 digest](./preview-feedback/w0/summary.md)).
- تتبع موجة W3 beta؛ جدولة موجات لاحقة حسب مراجعة الحوكمة.

## ملخص موجة الشركاء W1- الموافقات القانونية والحوكمة. تم توقيع ملحق الشركاء 2025-04-05؛ تمت رفع الموافقات الى `DOCS-SORA-Preview-W1`.
- القياس عن بعد + Try it staging. تم تنفيذ تذكرة التغيير `OPS-TRYIT-147` في 2025-04-06 مع ارشفة لقطات Grafana لـ `docs.preview.integrity` و `TryItProxyErrors` ו-`DocsPortal/GatewayRefusals`.
- تجهيز artefact + checksum. تم التحقق من حزمة `preview-2025-04-12`؛ تم حفظ سجلات descriptor/checksum/probe تحت `artifacts/docs_preview/W1/preview-2025-04-12/`.
- قائمة الدعوات + الارسال. تمت الموافقة على ثماني طلبات شركاء (`DOCS-SORA-Preview-REQ-P01...P08`)؛ ارسلت الدعوات 2025-04-12 15:00-15:21 UTC مع تسجيل الايصالات لكل مراجع.
- ادوات جمع الملاحظات. تم تسجيل office hours اليومية + checkpoints القياس؛ راجع [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) للملخص.
- القائمة النهائية / سجل الخروج. يسجل [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) الان توقيتات الدعوات/الايصالات، ادلة القياس، exports الاختبارات، ومؤشرات artefacts حتى 2025-04-26 ليتمكن فريق الحوكمة من اعادة بناء الموجة.

## سجل الدعوات - W0 core maintainers

| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | מתחזק פורטל | `DOCS-SORA-Preview-REQ-01` | 25-03-2025 10:05 | 2025-04-08 10:00 | نشط | اكد تحقق checksum؛ يركز على مراجعة nav/sidebar. |
| sdk-rust-01 | עופרת SDK חלודה | `DOCS-SORA-Preview-REQ-02` | 25-03-2025 10:08 | 2025-04-08 10:00 | نشط | يختبر وصفات SDK + quickstarts Norito. |
| sdk-js-01 | מתחזק JS SDK | `DOCS-SORA-Preview-REQ-03` | 25-03-2025 10:12 | 2025-04-08 10:00 | نشط | يراجع وحدة Try it + تدفقات ISO. |
| sorafs-ops-01 | SoraFS קשר מפעיל | `DOCS-SORA-Preview-REQ-04` | 25-03-2025 10:15 | 2025-04-08 10:00 | نشط | يدقق runbooks SoraFS + وثائق orchestration. |
| observability-01 | צפיות TL | `DOCS-SORA-Preview-REQ-05` | 25-03-2025 10:18 | 2025-04-08 10:00 | نشط | يراجع ملاحق القياس/الحوادث؛ مسؤول عن تغطية Alertmanager. |

كل الدعوات تشير الى نفس artefact `docs-portal-preview` (تشغيل 2025-03-24، وسم `preview-2025-03-24`) وسجل التحقق المحفوظ في `DOCS-SORA-Preview-W0`. يجب تسجيل اي اضافة/توقف في الجدول اعلاه وفي تذكرة المتعقب قبل الانتقال للموجة التالية.

## سجل checkpoints - W0

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 26-03-2025 | مراجعة baseline القياس + office hours | بقيت `docs.preview.integrity` و `TryItProxyErrors` باللون الاخضر؛ اكدت office hours اكتمال تحقق checksum. |
| 27-03-2025 | نشر ملخص feedback الوسطي | تم حفظ الملخص في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md)؛ تم تسجيل مشكلتين بسيطتين في nav تحت `docs-preview/w0` دون حوادث. |
| 31-03-2025 | فحص قياس الاسبوع الاخير | اخر office hours قبل الخروج؛ اكد المراجعون تقدم المهام، دون تنبيهات. |
| 2025-04-08 | ملخص الخروج + اغلاق الدعوات | تاكيد اكتمال المراجعات، سحب الوصول المؤقت، ارشفة النتائج في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08)؛ تحديث المتعقب قبل تجهيز W1. |

## سجل الدعوات - W1 partners| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| soraps-op-01 | מפעיל SoraFS (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 26/04/2025 15:00 | مكتمل | تم تسليم ملاحظات ops للاوركستريتور 2025-04-20؛ ack יום ראשון 15:05 UTC. |
| soraps-op-02 | מפעיל SoraFS (JP) | `DOCS-SORA-Preview-REQ-P02` | 12-04-2025 15:03 | 26/04/2025 15:00 | مكتمل | سجل تعليقات rollout في `docs-preview/w1`; ack 15:10 UTC. |
| soraps-op-03 | מפעיל SoraFS (ארה"ב) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 26/04/2025 15:00 | مكتمل | تم تسجيل تعديلات dispute/blacklist؛ ack 15:12 UTC. |
| torii-int-01 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P04` | 12-04-2025 15:09 | 26/04/2025 15:00 | مكتمل | מדריך הדרכה נסה זאת אימות; ack 15:14 UTC. |
| torii-int-02 | אינטגרטור Torii | `DOCS-SORA-Preview-REQ-P05` | 12-04-2025 15:12 | 26/04/2025 15:00 | مكتمل | تسجيل تعليقات RPC/OAuth؛ ack 15:16 UTC. |
| sdk-partner-01 | שותף SDK (Swift) | `DOCS-SORA-Preview-REQ-P06` | 12-04-2025 15:15 | 26/04/2025 15:00 | مكتمل | دمج ملاحظات سلامة المعاينة؛ ack 15:18 UTC. |
| sdk-partner-02 | שותף SDK (אנדרואיד) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 26/04/2025 15:00 | مكتمل | مراجعة telemetria/redaction اكتملت؛ ack 15:22 UTC. |
| gateway-ops-01 | מפעיל שער | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 26/04/2025 15:00 | مكتمل | تسجيل تعليقات runbook DNS gateway؛ ack 15:24 UTC. |

## سجل checkpoints - W1

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-04-12 | ارسال الدعوات + تحقق artefacts | تم ارسال البريد لكل الشركاء مع descriptor/archive `preview-2025-04-12`؛ الايصالات محفوظة في المتعقب. |
| 2025-04-13 | مراجعة baseline القياس | `docs.preview.integrity` و `TryItProxyErrors` و `DocsPortal/GatewayRefusals` باللون الاخضر؛ office hours اكدت اكتمال تحقق checksum. |
| 2025-04-18 | office hours منتصف الموجة | بقي `docs.preview.integrity` اخضر؛ تم تسجيل ملاحظتين docs تحت `docs-preview/w1` (صياغة nav + لقطة Try it). |
| 22-04-2025 | فحص قياس نهائي | الوكيل ولوحات القياس سليمة؛ لا قضايا جديدة، تم تدوينها في المتعقب قبل الخروج. |
| 26-04-2025 | ملخص الخروج + اغلاق الدعوات | اكد جميع الشركاء اكتمال المراجعات، تم سحب الدعوات، الادلة ارشفت في [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## ملخص cohorte beta W3

- ارسلت الدعوات 2026-02-18 مع تحقق checksum وتسجيل الايصالات في نفس اليوم.
- تم جمع feedback تحت `docs-preview/20260218` مع تذكرة الحوكمة `DOCS-SORA-Preview-20260218`; تم توليد digest + ملخص عبر `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- تم سحب الوصول 2026-02-28 بعد فحص القياس النهائي؛ تحديث المتعقب وجداول البوابة لاظهار اكتمال W3.

## سجل الدعوات - W2 community| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | מבקר קהילה (SDK) | `DOCS-SORA-Preview-REQ-C01` | 15-06-2025 16:00 | 29/06/2025 16:00 | مكتمل | ack 16:06 UTC; يركز على quickstarts SDK؛ تم تأكيد الخروج 2025-06-29. |
| comm-vol-02 | מבקר קהילה (ממשל) | `REQ-C02` | 15/06/2025 16:03 | 29/06/2025 16:00 | مكتمل | مراجعة الحوكمة/SNS اكتملت؛ تم تأكيد الخروج 2025-06-29. |
| comm-vol-03 | מבקר קהילה (Norito) | `REQ-C03` | 15/06/2025 16:06 | 29/06/2025 16:00 | مكتمل | تسجيل ملاحظات walkthrough Norito؛ ack 2025-06-29. |
| comm-vol-04 | מבקר קהילה (SoraFS) | `REQ-C04` | 15/06/2025 16:09 | 29/06/2025 16:00 | مكتمل | مراجعة runbooks SoraFS اكتملت؛ ack 2025-06-29. |
| comm-vol-05 | מבקר קהילה (נגישות) | `REQ-C05` | 15/06/2025 16:12 | 29/06/2025 16:00 | مكتمل | مشاركة ملاحظات accessibility/UX؛ ack 2025-06-29. |
| comm-vol-06 | מבקר קהילה (לוקליזציה) | `REQ-C06` | 15/06/2025 16:15 | 29/06/2025 16:00 | مكتمل | تسجيل ملاحظات localization؛ ack 2025-06-29. |
| comm-vol-07 | מבקר קהילה (נייד) | `REQ-C07` | 15/06/2025 16:18 | 29/06/2025 16:00 | مكتمل | تم تسليم مراجعات docs SDK mobile؛ ack 2025-06-29. |
| comm-vol-08 | מבקר קהילה (צפיות) | `REQ-C08` | 15/06/2025 16:21 | 29/06/2025 16:00 | مكتمل | مراجعة ملحق observability اكتملت؛ ack 2025-06-29. |

## سجل checkpoints - W2

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-06-15 | ارسال الدعوات + تحقق artefacts | تم مشاركة descriptor/archive `preview-2025-06-15` مع 8 مراجعين؛ الايصالات محفوظة في المتعقب. |
| 2025-06-16 | مراجعة baseline القياس | `docs.preview.integrity` و `TryItProxyErrors` و `DocsPortal/GatewayRefusals` خضراء؛ سجلات الوكيل Try it تظهر رموز المجتمع نشطة. |
| 2025-06-18 | office hours وفرز القضايا | اقتراحان (`docs-preview/w2 #1` صياغة tooltip، `#2` شريط localization) - كلاهما اسند الى Docs. |
| 21-06-2025 | فحص قياس + اصلاحات docs | تم حل `docs-preview/w2 #1/#2`; لوحات القياس خضراء، دون حوادث. |
| 24-06-2025 | office hours للاسبوع الاخير | اكد المراجعون ما تبقى من الملاحظات؛ لا تنبيهات. |
| 29-06-2025 | ملخص الخروج + اغلاق الدعوات | تم تسجيل الايصالات، سحب وصول المعاينة، ارشفة snapshots + artefacts (انظر [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | office hours وفرز القضايا | تم تسجيل اقتراحين توثيقيين تحت `docs-preview/w1`; لا حوادث ولا تنبيهات. |

## روابط التقارير

- كل يوم اربعاء، حدّث الجدول اعلاه وتذكرة الدعوات النشطة بملاحظة قصيرة (الدعوات المرسلة، المراجعين النشطين، الحوادث).
- عند اغلاق موجة، اضف ​​مسار ملخص الملاحظات (مثلا `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) واربطه من `status.md`.
- اذا تم تفعيل اي معيار توقف في [preview invite flow](./preview-invite-flow.md)، اضف ​​خطوات المعالجة هنا قبل استئناف الدعوات.