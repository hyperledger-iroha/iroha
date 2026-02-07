---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-الخطة
العنوان: خطة الاختبار المبدئي للشركاء W1
Sidebar_label: الخطة W1
الوصف: المهام والمسؤولون وقائمة الأدلة المرجعية لمجموعة معاينة الشركاء.
---

| العنصر | تفاصيل |
| --- | --- |
| علا | W1 - الشركاء والمتكاملون في Torii |
| فنتانا الهدف | الربع الثاني 2025 سيمانا 3 |
| بطاقة قطعة أثرية (planeado) | `preview-2025-04-12` |
| إصدار المتتبع | `DOCS-SORA-Preview-W1` |

##الأهداف

1. ضمان الموافقات القانونية وإدارة محطات معاينة الشركاء.
2. قم بإعداد الوكيل، جربه ولقطات القياس عن بعد المستخدمة في حزمة الدعوة.
3. قم بتحديث عنصر المعاينة الذي تم التحقق منه من خلال المجموع الاختباري ونتائج التحقيقات.
4. الانتهاء من قائمة الشركاء ومجموعات الطلبات قبل إرسال الدعوات.

## قم بإلغاء تحديد الأشياء

| معرف | تاريا | مسؤول | فيشا ليميت | حالة | نوتاس |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | الحصول على الموافقة القانونية لملحق شروط المعاينة | Docs/DevRel Lead -> قانوني | 2025-04-05 | اكتمل | التذكرة قانونية `DOCS-SORA-Preview-W1-Legal` بتاريخ 05-04-2025; PDF مساعد آل تعقب. |
| W1-P2 | التقاط نافذة التدريج للوكيل جربها (10/04/2025) وتحقق من صحة الوكيل | مستندات/DevRel + Ops | 2025-04-06 | اكتمل | تم تنفيذه `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` في 06-04-2025; نسخ CLI و `.env.tryit-proxy.bak` أرشيفات. |
| W1-P3 | إنشاء معاينة أصلية (`preview-2025-04-12`)، التصحيح `scripts/preview_verify.sh` + `npm run probe:portal`، واصف الأرشيف/المجاميع الاختبارية | بوابة TL | 2025-04-08 | اكتمل | القطعة الأثرية وسجلات التحقق المحمية في `artifacts/docs_preview/W1/preview-2025-04-12/`؛ يخرج المسبار من جهاز التعقب. |
| W1-P4 | مراجعة صيغ قبول الشركاء (`DOCS-SORA-Preview-REQ-P01...P08`)، وتأكيد جهات الاتصال واتفاقيات عدم الإفشاء | الاتصال بالحوكمة | 2025-04-07 | اكتمل | Las ocho solicitudes aprobadas (las ultimas dos el 2025-04-11); Approbaciones enlazadas en el Tracker. |
| W1-P5 | قم بتحرير نسخة الدعوة (استنادًا إلى `docs/examples/docs_preview_invite_template.md`)، وتسجيل `<preview_tag>` و`<request_ticket>` لكل شريك | مستندات/DevRel الرصاص | 2025-04-08 | اكتمل | تم إرسال دعوة الدعوة في 12-04-2025 15:00 بالتوقيت العالمي المنسق جنبًا إلى جنب مع أحزمة مصنوعة يدويًا. |

## قائمة التحقق من الاختبار المبدئي

> النصيحة: تنفيذ `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` لتنفيذ الخطوات 1-5 تلقائيًا (الإنشاء والتحقق من المجموع الاختباري واختبار البوابة ومدقق الارتباط وتحديث الوكيل جربه). يقوم البرنامج النصي بتسجيل سجل JSON يمكن إضافته إلى إصدار المتتبع.

1. `npm run build` (مع `DOCS_RELEASE_TAG=preview-2025-04-12`) لإعادة إنشاء `build/checksums.sha256` و`build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` وأرشفة `build/link-report.json` جنبًا إلى جنب مع الواصف.
5.`npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (لتتبع الهدف المناسب عبر `--tryit-target`)؛ قم بتنفيذ `.env.tryit-proxy` الذي تم تحديثه واحتفظ بـ `.bak` للتراجع.
6. تحديث إصدار W1 بمسارات السجلات (المجموع الاختباري للواصف، وإخراج التحقيق، وتغيير الوكيل، جربه واللقطات Grafana).

## قائمة التحقق من الأدلة

- [x] الموافقة القانونية (PDF أو إرفاق التذكرة) ملحقة بـ `DOCS-SORA-Preview-W1`.
- [x] لقطات شاشة من Grafana لـ `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals`.
- [x] الواصف وسجل المجموع الاختباري لـ `preview-2025-04-12` المحميان أسفل `artifacts/docs_preview/W1/`.
- [x] جدول قائمة الدعوات مع الطوابع الزمنية `invite_sent_at` مكتمل (إصدار سجل W1 من المتتبع).
- [x] ردود الفعل المصطنعة في [`preview-feedback/w1/log.md`](./log.md) مع ملف لشريك (تم التحديث في 26-04-2025 مع بيانات القائمة/القياس عن بعد/المشكلات).

تفعيل هذه الخطة من خلال تعزيز المهام; المتتبع هو المرجع للحفاظ على خريطة الطريق القابلة للتدقيق.

## تدفق ردود الفعل

1. لكل مراجع، قم بتكرار النبات في
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)،
   أكمل البيانات الوصفية واحفظ النسخة المنتهية مؤقتًا
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. استئناف الدعوات ونقاط التفتيش للقياس عن بعد وإصدار فتح داخل سجل الحياة en
   [`preview-feedback/w1/log.md`](./log.md) حتى يتمكن مراجعو الإدارة من مراجعة كل شيء
   بدون مغادرة المستودع.
3. عندما ترغب في تصدير المعرفة أو التحقق من المعلومات أو الملحقات على طريق المصنوعات اليدوية المشار إليها في السجل
   وقم بتشغيل إصدار المتعقب.