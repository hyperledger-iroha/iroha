---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-الخطة
العنوان: خطة التحضير للشركاء W1
Sidebar_label: خطة W1
الوصف: مهام، مالكون، وقائمة مزدوجة لمكالمات مكالماتهم.
---

| البند | التفاصيل |
| --- | --- |
| ب | W1 - البرنامج ومتكاملو Torii |
| نافذة الهدف | الربع الثاني 2025 الاسبوع 3 |
| اتصال الاثر (مخطط) | `preview-2025-04-12` |
| تذكرة التتبع | `DOCS-SORA-Preview-W1` |

## الاهداف

1. الحصول على موافقات وتشارك في شروط اشتراكك.
2. مختبر الوكيل جرب ولقطات القياس المستخدمة في حزمة الأحداث.
3. تحديث فعالية المعاينة المتحقق بالـ المجموع الاختباري ونتائج الـ تحقيقات.
4. إنها قائمة المرشحين وقوالب الطلبات قبل إرسال الدعوات.

## مجال العمل

| المعرف | | المالك | الاستحقاق | الحالة | تعليقات |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | الحصول على عضوية عضوية في شروط المعاينة | Docs/DevRel Lead -> قانوني | 2025-04-05 | ✅ مكتملة | الموافقة على التذكرة القانونية `DOCS-SORA-Preview-W1-Legal` في 2025-04-05؛ ملف PDF المرفق بالمتتبع. |
| W1-P2 | طلب نافذة التدريج لوكيل جربه (10-04-2025) والتحقق من صحة الوكيل | مستندات/DevRel + Ops | 2025-04-06 | ✅ مكتملة | تم تنفيذ `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` في 06-04-2025؛ تم ارشفة سجل CLI و `.env.tryit-proxy.bak`. |
| W1-P3 | تأثير بناء المعاينة (`preview-2025-04-12`)، تشغيل `scripts/preview_verify.sh` + `npm run probe:portal`، وارفة الواصف/المجاميع الاختبارية | بوابة TL | 2025-04-08 | ✅ مكتملة | تم حفظ الاثر وتم التحقق تحت `artifacts/docs_preview/W1/preview-2025-04-12/`؛ مخرجات مسبار المرفق بالمتتبع. |
| W1-P4 | مراجعة نماذج العينات للشركاء (`DOCS-SORA-Preview-REQ-P01...P08`)، وتاكيد جهات الاتصال و NDA | الاتصال بالحوكمة | 2025-04-07 | ✅ مكتملة | الموافقة على طلبات الموافقة (الطلبات الأخيرة في 11-04-2025)؛ الروابط موجودة في المتتبع. |
| W1-P5 | صياغة صياغة (مبنية على `docs/examples/docs_preview_invite_template.md`) وضبط `<preview_tag>` و `<request_ticket>` لكل شريك | مستندات/DevRel الرصاص | 2025-04-08 | ✅ مكتملة | ارسلت مسودة الأحداث في 12-04-2025 15:00 UTC مع روابط الاثر. |

## قائمة التحقق قبل الاطلاق

> تلميح: الوظيفة `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` وتتبع الخطوات 1-5 بشكل تلقائي (بناء، التحقق من المجموع الاختباري، مسبار للبوابة، مدقق الارتباط، وتحديث وكيل جربه). سجل السكربت سجل JSON الذي يمكنك اصطياده بتذكر التتبع.

1. `npm run build` (مع `DOCS_RELEASE_TAG=preview-2025-04-12`) لاعادة توليد `build/checksums.sha256` و `build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` وارشفة `build/link-report.json` Bebe الواصف.
5.`npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (او مرر الهدف المناسب عبر `--tryit-target`); ثبّت التحديث في `.env.tryit-proxy` واحتفظ بـ `.bak` للرجوع.
6. تحديث تذكرة W1 بمسارات تسجيل الدخول (المجموع الاختباري للواصف، مخرجات التحقيق، تغيير وكيل جربه، ولقطات Grafana).

## قائمة تعادل الاثبات

- [x] الموافقة على اتفاقية حظرة (PDF او رابط التذكرة) المرفق بـ `DOCS-SORA-Preview-W1`.
- [x] لقطات Grafana لـ `docs.preview.integrity`, `TryItProxyErrors`, `DocsPortal/GatewayRefusals`.
- [x] واصف المجموع الاختباري لـ `preview-2025-04-12` محفوظان تحت `artifacts/docs_preview/W1/`.
- [x] جدول للدعوات مع استهلاك `invite_sent_at` مفضل (راجع سجل W1 في التتبع).
- [x] اثار التغذية المراجعات منعكسة في [`preview-feedback/w1/log.md`](./log.md) مع صف لكل شريك (تم تحديثه 2025-04-26 ببيانات roster/telemetria/issues).

تحديث هذه البناء كلما تقدمت المهام؛ تشير إلى المتتبع للطقس على دقة خارطة الطريق.

## سير عمل التغذية المراجعة

1. لكل مرشح، انسخ موقعًا في
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)،
   املأ البيانات الوصفية، واحفظ النسخة الكاملة تحت
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. لخص الدعوات ونقاط القياس والمسائل المفتوحة داخل السجل الحي في
   [`preview-feedback/w1/log.md`](./log.md) حتى يرغب في إعادة تشغيل الالتماس من جديد
   دون فقدان المستودع.
3. عند وصول واردات المعرفة او الاستبيانات، ارفقها في مسار الاثر فقط في السجل
   وربط تذكرة التتبع.