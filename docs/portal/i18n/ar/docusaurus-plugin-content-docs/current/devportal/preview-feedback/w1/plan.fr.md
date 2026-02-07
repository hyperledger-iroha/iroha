---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-الخطة
العنوان: Plan de preflight Partenaires W1
Sidebar_label: الخطة W1
الوصف: العلامات والمسؤولون وقائمة المراجعة لمجموعة المعاينة المشتركة.
---

| العنصر | التفاصيل |
| --- | --- |
| غامضة | W1 - شركاء ومتكاملون Torii |
| نافذة زجاجية | الربع الثاني من عام 2025، الفصل الثالث |
| علامة القطع الأثرية (planifie) | `preview-2025-04-12` |
| تعقب القضية | `DOCS-SORA-Preview-W1` |

## الأهداف

1. احصل على الموافقات القانونية والحوكمة لشروط المعاينة المشتركة.
2. قم بإعداد الوكيل جربه واستخدم لقطات القياس عن بعد في حزمة الدعوة.
3. قم بتحرير معاينة المعاينة من خلال المجموع الاختباري ونتائج التحقيقات.
4. قم بإنهاء قائمة الشركاء ونماذج الطلب قبل إرسال الدعوات.

## Decoupage des taches

| معرف | تاش | مسؤول | الصدى | النظام الأساسي | ملاحظات |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | الحصول على الموافقة القانونية لإضافة شروط المعاينة | Docs/DevRel Lead -> قانوني | 2025-04-05 | ترمين | التذكرة قانونية `DOCS-SORA-Preview-W1-Legal` صالحة لو 2025-04-05؛ إرفاق ملف PDF للتعقب. |
| W1-P2 | التقط نافذة التدريج للوكيل جربها (10-04-2025) وتحقق من صحة الوكيل | مستندات/DevRel + Ops | 2025-04-06 | ترمين | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` تنفيذ لو 2025-04-06؛ أرشيفات النسخ CLI + `.env.tryit-proxy.bak`. |
| W1-P3 | إنشاء المعاينة الفنية (`preview-2025-04-12`)، المنفذ `scripts/preview_verify.sh` + `npm run probe:portal`، واصف الأرشيف/المجاميع الاختبارية | بوابة TL | 2025-04-08 | ترمين | قطعة أثرية + سجلات التحقق من المخزون Sous `artifacts/docs_preview/W1/preview-2025-04-12/`؛ طلعة التحقيق المرفقة إلى جهاز التعقب. |
| W1-P4 | Revoir les formaires d'intake Partenaires (`DOCS-SORA-Preview-REQ-P01...P08`)، جهات الاتصال المؤكدة + NDAs | الاتصال بالحوكمة | 2025-04-07 | ترمين | الموافقة على طلب الرطوبة (les deux dernieres le 2025-04-11); approbations liees dans le Tracker. |
| W1-P5 | أعد كتابة نص الدعوة (قاعدة على `docs/examples/docs_preview_invite_template.md`)، وحدد `<preview_tag>` و`<request_ticket>` لكل مشاركة | مستندات/DevRel الرصاص | 2025-04-08 | ترمين | إرسال دعوة إلى 12-04-2025 15:00 بالتوقيت العالمي المنسق مع الامتيازات الفنية. |

## قائمة المراجعة المبدئية

> السبب: lancez `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` لتنفيذ الخطوات 1-5 تلقائيًا (الإنشاء، والمجموع الاختباري للتحقق، واختبار البوابة، ومدقق الارتباط، وإعداد يوم الوكيل جربه). يقوم البرنامج النصي بتسجيل سجل JSON وضمه إلى المتتبع.

1. `npm run build` (مع `DOCS_RELEASE_TAG=preview-2025-04-12`) لإعادة إنشاء `build/checksums.sha256` و`build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` والأرشيف `build/link-report.json` عبارة عن جزء من الواصف.
5.`npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (أو قم بتوفير الكابل المناسب عبر `--tryit-target`)؛ التزم بـ `.env.tryit-proxy` في اليوم واحتفظ بـ `.bak` للتراجع.
6. قم بإجراء الإصدار W1 يوميًا باستخدام سلاسل السجلات (المجموع الاختباري للواصف، وفرز المسبار، وتغيير الوكيل، وجرب اللقطات واللقطات Grafana).

## قائمة المراجعة المسبقة

- [x] التوقيع القانوني للموافقة (PDF أو حجز التذكرة) الملحق بـ `DOCS-SORA-Preview-W1`.
- [x] لقطات الشاشة Grafana لـ `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals`.
- [x] الواصف وسجل المجموع الاختباري `preview-2025-04-12` الأسهم الموجودة في `artifacts/docs_preview/W1/`.
- [x] لوحة قائمة الدعوات مع `invite_sent_at` reenseignes (voir le log W1 du Tracker).
- [x] ردود الفعل المرتدة في [`preview-feedback/w1/log.md`](./log.md) بخط مشترك (في 26/04/2025 مع القائمة/القياس عن بعد/القضايا).

Mettre a jour ce Plan a mesure de l'avancement; يشير المتتبع إلى أن خريطة الطريق قابلة للتدقيق.

## تدفق ردود الفعل

1. قم بنسخ القالب في كل مرة
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)،
   قم باستعادة البيانات واحتفظ بالنسخة كاملة
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. استئناف الدعوات ونقاط التفتيش للقياس عن بعد والإصدارات الخارجية في سجل الحياة
   [`preview-feedback/w1/log.md`](./log.md) لكي يتمكن المراجعون من تجديد الغموض
   بلا إنهاء لو مستودع.
3. عند وصول صادرات المعرفة أو المسبار، يتم وصلها في شريط المادة الأثرية في السجل
   ثم قم بإصدار جهاز التعقب.