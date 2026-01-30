---
lang: he
direction: rtl
source: docs/portal/docs/devportal/preview-invite-tracker.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# متعقب دعوات المعاينة

يسجل هذا المتعقب كل موجة معاينة لبوابة المستندات حتى يتمكن مالكو DOCS-SORA ومراجعو الحوكمة من رؤية اي مجموعة نشطة، من وافق على الدعوات، واي اثار ما زالت بحاجة الى متابعة. حدّثه كلما تم ارسال الدعوات او سحبها او تأجيلها حتى يبقى سجل التدقيق داخل المستودع.

## حالة الموجات

| الموجة | المجموعة | تذكرة المتابعة | الموافقون | الحالة | النافذة المستهدفة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| **W0 - Core maintainers** | Maintainers من Docs + SDK يتحققون من تدفق checksum | `DOCS-SORA-Preview-W0` (متتبع GitHub/ops) | Lead Docs/DevRel + Portal TL | مكتمل | Q2 2025 الاسابيع 1-2 | الدعوات ارسلت 2025-03-25، القياس عن بعد بقي اخضر، ملخص الخروج نشر 2025-04-08. |
| **W1 - Partners** | مشغلو SoraFS ومتكاملو Torii تحت NDA | `DOCS-SORA-Preview-W1` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | Q2 2025 الاسبوع 3 | الدعوات 2025-04-12 -> 2025-04-26 مع تأكيد جميع الشركاء الثمانية؛ تم حفظ الادلة في [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) وملخص الخروج في [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md). |
| **W2 - Community** | قائمة انتظار مجتمعية منتقاة (<=25 في كل مرة) | `DOCS-SORA-Preview-W2` | Lead Docs/DevRel + community manager | مكتمل | Q3 2025 الاسبوع 1 (مبدئي) | الدعوات 2025-06-15 -> 2025-06-29 مع قياس عن بعد اخضر طوال الفترة؛ الادلة والنتائج في [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md). |
| **W3 - Cohortes beta** | Beta التمويل/الملاحظة + شريك SDK + داعم للنظام البيئي | `DOCS-SORA-Preview-W3` | Lead Docs/DevRel + رابط الحوكمة | مكتمل | Q1 2026 الاسبوع 8 | الدعوات 2026-02-18 -> 2026-02-28؛ تم توليد الملخص وبيانات البوابة عبر موجة `preview-20260218` (انظر [`preview-feedback/w3/summary.md`](./preview-feedback/w3/summary.md)). |

> ملاحظة: اربط كل تذكرة في المتعقب بطلبات المعاينة وارشفها تحت مشروع `docs-portal-preview` حتى تبقى الموافقات قابلة للاكتشاف.

## المهام النشطة (W0)

- تحديث اثار preflight (تشغيل GitHub Actions `docs-portal-preview` بتاريخ 2025-03-24، وتحقق descriptor عبر `scripts/preview_verify.sh` باستخدام وسم `preview-2025-03-24`).
- توثيق خطوط اساس القياس عن بعد (`docs.preview.integrity`، وحفظ لقطة `TryItProxyErrors` في تذكرة W0).
- تثبيت نص التواصل باستخدام [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) مع وسم preview `preview-2025-03-24`.
- تسجيل طلبات الدخول لاوائل خمسة maintainers (تذاكر `DOCS-SORA-Preview-REQ-01` ... `-05`).
- ارسال اول خمس دعوات 2025-03-25 10:00-10:20 UTC بعد سبعة ايام متتالية من القياس الاخضر؛ تم حفظ الايصالات في `DOCS-SORA-Preview-W0`.
- متابعة القياس عن بعد + office hours للمضيف (فحوصات يومية حتى 2025-03-31؛ سجل checkpoints بالاسفل).
- جمع ملاحظات منتصف الموجة / القضايا ووضع الوسم `docs-preview/w0` (انظر [W0 digest](./preview-feedback/w0/summary.md)).
- نشر ملخص الموجة + تأكيدات الخروج (حزمة خروج بتاريخ 2025-04-08؛ انظر [W0 digest](./preview-feedback/w0/summary.md)).
- تتبع موجة W3 beta؛ جدولة موجات لاحقة حسب مراجعة الحوكمة.

## ملخص موجة الشركاء W1

- الموافقات القانونية والحوكمة. تم توقيع ملحق الشركاء 2025-04-05؛ تمت رفع الموافقات الى `DOCS-SORA-Preview-W1`.
- القياس عن بعد + Try it staging. تم تنفيذ تذكرة التغيير `OPS-TRYIT-147` في 2025-04-06 مع ارشفة لقطات Grafana لـ `docs.preview.integrity` و `TryItProxyErrors` و `DocsPortal/GatewayRefusals`.
- تجهيز artefact + checksum. تم التحقق من حزمة `preview-2025-04-12`؛ تم حفظ سجلات descriptor/checksum/probe تحت `artifacts/docs_preview/W1/preview-2025-04-12/`.
- قائمة الدعوات + الارسال. تمت الموافقة على ثماني طلبات شركاء (`DOCS-SORA-Preview-REQ-P01...P08`)؛ ارسلت الدعوات 2025-04-12 15:00-15:21 UTC مع تسجيل الايصالات لكل مراجع.
- ادوات جمع الملاحظات. تم تسجيل office hours اليومية + checkpoints القياس؛ راجع [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md) للملخص.
- القائمة النهائية / سجل الخروج. يسجل [`preview-feedback/w1/log.md`](./preview-feedback/w1/log.md) الان توقيتات الدعوات/الايصالات، ادلة القياس، exports الاختبارات، ومؤشرات artefacts حتى 2025-04-26 ليتمكن فريق الحوكمة من اعادة بناء الموجة.

## سجل الدعوات - W0 core maintainers

| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| docs-core-01 | Portal maintainer | `DOCS-SORA-Preview-REQ-01` | 2025-03-25 10:05 | 2025-04-08 10:00 | نشط | اكد تحقق checksum؛ يركز على مراجعة nav/sidebar. |
| sdk-rust-01 | Rust SDK lead | `DOCS-SORA-Preview-REQ-02` | 2025-03-25 10:08 | 2025-04-08 10:00 | نشط | يختبر وصفات SDK + quickstarts Norito. |
| sdk-js-01 | JS SDK maintainer | `DOCS-SORA-Preview-REQ-03` | 2025-03-25 10:12 | 2025-04-08 10:00 | نشط | يراجع وحدة Try it + تدفقات ISO. |
| sorafs-ops-01 | SoraFS operator liaison | `DOCS-SORA-Preview-REQ-04` | 2025-03-25 10:15 | 2025-04-08 10:00 | نشط | يدقق runbooks SoraFS + وثائق orchestration. |
| observability-01 | Observability TL | `DOCS-SORA-Preview-REQ-05` | 2025-03-25 10:18 | 2025-04-08 10:00 | نشط | يراجع ملاحق القياس/الحوادث؛ مسؤول عن تغطية Alertmanager. |

كل الدعوات تشير الى نفس artefact `docs-portal-preview` (تشغيل 2025-03-24، وسم `preview-2025-03-24`) وسجل التحقق المحفوظ في `DOCS-SORA-Preview-W0`. يجب تسجيل اي اضافة/توقف في الجدول اعلاه وفي تذكرة المتعقب قبل الانتقال للموجة التالية.

## سجل checkpoints - W0

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-03-26 | مراجعة baseline القياس + office hours | بقيت `docs.preview.integrity` و `TryItProxyErrors` باللون الاخضر؛ اكدت office hours اكتمال تحقق checksum. |
| 2025-03-27 | نشر ملخص feedback الوسطي | تم حفظ الملخص في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md)؛ تم تسجيل مشكلتين بسيطتين في nav تحت `docs-preview/w0` دون حوادث. |
| 2025-03-31 | فحص قياس الاسبوع الاخير | اخر office hours قبل الخروج؛ اكد المراجعون تقدم المهام، دون تنبيهات. |
| 2025-04-08 | ملخص الخروج + اغلاق الدعوات | تاكيد اكتمال المراجعات، سحب الوصول المؤقت، ارشفة النتائج في [`preview-feedback/w0/summary.md`](./preview-feedback/w0/summary.md#exit-summary-2025-04-08)؛ تحديث المتعقب قبل تجهيز W1. |

## سجل الدعوات - W1 partners

| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| sorafs-op-01 | SoraFS operator (EU) | `DOCS-SORA-Preview-REQ-P01` | 2025-04-12 15:00 | 2025-04-26 15:00 | مكتمل | تم تسليم ملاحظات ops للاوركستريتور 2025-04-20؛ ack خروج 15:05 UTC. |
| sorafs-op-02 | SoraFS operator (JP) | `DOCS-SORA-Preview-REQ-P02` | 2025-04-12 15:03 | 2025-04-26 15:00 | مكتمل | سجل تعليقات rollout في `docs-preview/w1`; ack 15:10 UTC. |
| sorafs-op-03 | SoraFS operator (US) | `DOCS-SORA-Preview-REQ-P03` | 2025-04-12 15:06 | 2025-04-26 15:00 | مكتمل | تم تسجيل تعديلات dispute/blacklist؛ ack 15:12 UTC. |
| torii-int-01 | Torii integrator | `DOCS-SORA-Preview-REQ-P04` | 2025-04-12 15:09 | 2025-04-26 15:00 | مكتمل | قبول walkthrough Try it auth؛ ack 15:14 UTC. |
| torii-int-02 | Torii integrator | `DOCS-SORA-Preview-REQ-P05` | 2025-04-12 15:12 | 2025-04-26 15:00 | مكتمل | تسجيل تعليقات RPC/OAuth؛ ack 15:16 UTC. |
| sdk-partner-01 | SDK partner (Swift) | `DOCS-SORA-Preview-REQ-P06` | 2025-04-12 15:15 | 2025-04-26 15:00 | مكتمل | دمج ملاحظات سلامة المعاينة؛ ack 15:18 UTC. |
| sdk-partner-02 | SDK partner (Android) | `DOCS-SORA-Preview-REQ-P07` | 2025-04-12 15:18 | 2025-04-26 15:00 | مكتمل | مراجعة telemetria/redaction اكتملت؛ ack 15:22 UTC. |
| gateway-ops-01 | Gateway operator | `DOCS-SORA-Preview-REQ-P08` | 2025-04-12 15:21 | 2025-04-26 15:00 | مكتمل | تسجيل تعليقات runbook DNS gateway؛ ack 15:24 UTC. |

## سجل checkpoints - W1

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-04-12 | ارسال الدعوات + تحقق artefacts | تم ارسال البريد لكل الشركاء مع descriptor/archive `preview-2025-04-12`؛ الايصالات محفوظة في المتعقب. |
| 2025-04-13 | مراجعة baseline القياس | `docs.preview.integrity` و `TryItProxyErrors` و `DocsPortal/GatewayRefusals` باللون الاخضر؛ office hours اكدت اكتمال تحقق checksum. |
| 2025-04-18 | office hours منتصف الموجة | بقي `docs.preview.integrity` اخضر؛ تم تسجيل ملاحظتين docs تحت `docs-preview/w1` (صياغة nav + لقطة Try it). |
| 2025-04-22 | فحص قياس نهائي | الوكيل ولوحات القياس سليمة؛ لا قضايا جديدة، تم تدوينها في المتعقب قبل الخروج. |
| 2025-04-26 | ملخص الخروج + اغلاق الدعوات | اكد جميع الشركاء اكتمال المراجعات، تم سحب الدعوات، الادلة ارشفت في [`preview-feedback/w1/summary.md`](./preview-feedback/w1/summary.md#exit-summary-2025-04-26). |

## ملخص cohorte beta W3

- ارسلت الدعوات 2026-02-18 مع تحقق checksum وتسجيل الايصالات في نفس اليوم.
- تم جمع feedback تحت `docs-preview/20260218` مع تذكرة الحوكمة `DOCS-SORA-Preview-20260218`; تم توليد digest + ملخص عبر `npm run --prefix docs/portal preview:wave -- --wave preview-20260218`.
- تم سحب الوصول 2026-02-28 بعد فحص القياس النهائي؛ تحديث المتعقب وجداول البوابة لاظهار اكتمال W3.

## سجل الدعوات - W2 community

| معرف المراجع | الدور | تذكرة الطلب | ارسال الدعوة (UTC) | الخروج المتوقع (UTC) | الحالة | ملاحظات |
| --- | --- | --- | --- | --- | --- | --- |
| comm-vol-01 | Community reviewer (SDK) | `DOCS-SORA-Preview-REQ-C01` | 2025-06-15 16:00 | 2025-06-29 16:00 | مكتمل | ack 16:06 UTC؛ يركز على quickstarts SDK؛ تم تأكيد الخروج 2025-06-29. |
| comm-vol-02 | Community reviewer (Governance) | `REQ-C02` | 2025-06-15 16:03 | 2025-06-29 16:00 | مكتمل | مراجعة الحوكمة/SNS اكتملت؛ تم تأكيد الخروج 2025-06-29. |
| comm-vol-03 | Community reviewer (Norito) | `REQ-C03` | 2025-06-15 16:06 | 2025-06-29 16:00 | مكتمل | تسجيل ملاحظات walkthrough Norito؛ ack 2025-06-29. |
| comm-vol-04 | Community reviewer (SoraFS) | `REQ-C04` | 2025-06-15 16:09 | 2025-06-29 16:00 | مكتمل | مراجعة runbooks SoraFS اكتملت؛ ack 2025-06-29. |
| comm-vol-05 | Community reviewer (Accessibility) | `REQ-C05` | 2025-06-15 16:12 | 2025-06-29 16:00 | مكتمل | مشاركة ملاحظات accessibility/UX؛ ack 2025-06-29. |
| comm-vol-06 | Community reviewer (Localization) | `REQ-C06` | 2025-06-15 16:15 | 2025-06-29 16:00 | مكتمل | تسجيل ملاحظات localization؛ ack 2025-06-29. |
| comm-vol-07 | Community reviewer (Mobile) | `REQ-C07` | 2025-06-15 16:18 | 2025-06-29 16:00 | مكتمل | تم تسليم مراجعات docs SDK mobile؛ ack 2025-06-29. |
| comm-vol-08 | Community reviewer (Observability) | `REQ-C08` | 2025-06-15 16:21 | 2025-06-29 16:00 | مكتمل | مراجعة ملحق observability اكتملت؛ ack 2025-06-29. |

## سجل checkpoints - W2

| التاريخ (UTC) | النشاط | ملاحظات |
| --- | --- | --- |
| 2025-06-15 | ارسال الدعوات + تحقق artefacts | تم مشاركة descriptor/archive `preview-2025-06-15` مع 8 مراجعين؛ الايصالات محفوظة في المتعقب. |
| 2025-06-16 | مراجعة baseline القياس | `docs.preview.integrity` و `TryItProxyErrors` و `DocsPortal/GatewayRefusals` خضراء؛ سجلات الوكيل Try it تظهر رموز المجتمع نشطة. |
| 2025-06-18 | office hours وفرز القضايا | اقتراحان (`docs-preview/w2 #1` صياغة tooltip، `#2` شريط localization) - كلاهما اسند الى Docs. |
| 2025-06-21 | فحص قياس + اصلاحات docs | تم حل `docs-preview/w2 #1/#2`; لوحات القياس خضراء، دون حوادث. |
| 2025-06-24 | office hours للاسبوع الاخير | اكد المراجعون ما تبقى من الملاحظات؛ لا تنبيهات. |
| 2025-06-29 | ملخص الخروج + اغلاق الدعوات | تم تسجيل الايصالات، سحب وصول المعاينة، ارشفة snapshots + artefacts (انظر [`preview-feedback/w2/summary.md`](./preview-feedback/w2/summary.md#exit-summary-2025-06-29)). |
| 2025-04-15 | office hours وفرز القضايا | تم تسجيل اقتراحين توثيقيين تحت `docs-preview/w1`; لا حوادث ولا تنبيهات. |

## روابط التقارير

- كل يوم اربعاء، حدّث الجدول اعلاه وتذكرة الدعوات النشطة بملاحظة قصيرة (الدعوات المرسلة، المراجعين النشطين، الحوادث).
- عند اغلاق موجة، اضف مسار ملخص الملاحظات (مثلا `docs/portal/docs/devportal/preview-feedback/w0/summary.md`) واربطه من `status.md`.
- اذا تم تفعيل اي معيار توقف في [preview invite flow](./preview-invite-flow.md)، اضف خطوات المعالجة هنا قبل استئناف الدعوات.
