---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-الخطة
العنوان: خطة الاختبار المبدئي لشريك W1
Sidebar_label: الخطة W1
الوصف: الأصدقاء والأحباء وقائمة الاختيار للمساهمة في مجموعة معاينة الشريك.
---

| نقطة | التفاصيل |
| --- | --- |
| فولنا | W1 - الشركاء والمتكاملون Torii |
| Целевое okno | الربع الثاني 2025 في 3 |
| قطعة أثرية (خطة) | `preview-2025-04-12` |
| تريكر | `DOCS-SORA-Preview-W1` |

## كيلي

1. الحصول على معاينة الشراكة القانونية والحوكمة.
2. قم بتثبيت جربه بالوكالة وصور القياس عن بعد لحزمة العرض.
3. قم بمعاينة المجموع الاختباري للتحقق من معاينة القطع الأثرية ومسبار النتائج.
4. الانتهاء من إعداد قائمة الشركاء والشركاء لتنفيذ الإجراءات.

## Разбивка задач

| معرف | زادا | فلاديليتس | كروك | الحالة | مساعدة |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | الحصول على الموافقة القانونية للمعاينة الإضافية | Docs/DevRel Lead -> قانوني | 2025-04-05 | ✅ ممتاز | تذكرة Юridichеский `DOCS-SORA-Preview-W1-Legal` معتمدة بتاريخ 05-04-2025; تطبيق PDF للتريكيرو. |
| W1-P2 | Зафиксировать staging-okно Try it proxy (2025-04-10) وتحقق من الوكيل التالي | مستندات/DevRel + Ops | 2025-04-06 | ✅ ممتاز | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` مفعل 2025-04-06; نسخ CLI وأرشفة `.env.tryit-proxy.bak`. |
| W1-P3 | قم بالتعرف على معاينة المنتج (`preview-2025-04-12`)، ثم قم بتثبيت `scripts/preview_verify.sh` + `npm run probe:portal`، وأرشفة الواصف/المجاميع الاختبارية | بوابة TL | 2025-04-08 | ✅ ممتاز | تحقق العناصر والشعارات المخزنة في `artifacts/docs_preview/W1/preview-2025-04-12/`؛ вывод пилоден к текеро. |
| W1-P4 | التحقق من نماذج القبول للشركاء (`DOCS-SORA-Preview-REQ-P01...P08`)، التحقق من جهات الاتصال واتفاقية عدم الإفشاء | الاتصال بالحوكمة | 2025-04-07 | ✅ ممتاز | جميع الحقوق محفوظة (تم الاسترجاع 2025-04-11); خدمات التوصيل في الرحلات. |
| W1-P5 | قم بقراءة النص (على أساس `docs/examples/docs_preview_invite_template.md`)، إلى `<preview_tag>` و`<request_ticket>` لكل شريك | مستندات/DevRel الرصاص | 2025-04-08 | ✅ ممتاز | تم إطلاق سراح تشيرنوفيك في 12-04-2025 الساعة 15:00 بالتوقيت العالمي المنسق مع إرشادات القطعة الأثرية. |

## قائمة الاختيار المبدئية

> اللغة: أغلق `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` لتتمكن من اختيار العناصر تلقائيًا من 1 إلى 5 (الإنشاء، والمجموع الاختباري للتحقق، ومسبار البوابة، ومدقق الارتباط، والتحقق جربه بالوكالة). يقوم البرنامج النصي بكتابة سجل JSON الذي يمكن تطبيقه على الإصدار التالي.

1. `npm run build` (مع `DOCS_RELEASE_TAG=preview-2025-04-12`) للتبديل `build/checksums.sha256` و`build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4.`DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` وأرشفة `build/link-report.json` باستخدام الواصف.
5.`npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (أو تحديد الهدف المطلوب من خلال `--tryit-target`); قم بإنهاء `.env.tryit-proxy` الأساسي وقم بإلحاق `.bak` للاستعادة.
6. قم بتحديث إصدار W1 من الشعار (واصف المجموع الاختباري، ومسبار الإدخال، وتغييرات Try it proxy ولقطات Grafana).

## قائمة التحقق من المصادقة

- [x] الموافقة الرسمية للنشرة (PDF أو تذكرة) مقدمة إلى `DOCS-SORA-Preview-W1`.
- [x] Grafana شاشات لـ `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals`.
- [x] الواصف والمجموع الاختباري `preview-2025-04-12` موجودان في `artifacts/docs_preview/W1/`.
- [x] يتم عرض قائمة اللوحة باستخدام `invite_sent_at` (سجل W1 في المسار).
- [x] قطع أثرية تم طرحها في [`preview-feedback/w1/log.md`](./log.md) مع شريك واحد (سيُنشر في 26/04/2025 في الموقع). القائمة/القياس عن بعد/القضايا).

قم بإدراج هذه الخطة لمجرد الإنتاج; يأتي trekker إلى ما هو ضروري لدعم خارطة طريق قابلية التدقيق.

## العملية معروفة

1. لكل مراجع يقوم بنشر شابلون
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)،
   قم بإلغاء النسخ وحفظ النسخة النهائية في
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. إطلاق النار ونقاط التفتيش عن بعد والقضايا المفتوحة في سجل الحياة
   [`preview-feedback/w1/log.md`](./log.md)، بحيث يمكن لمراجعي الإدارة أن يناقشوا الأمر بشكل كامل
   في الحقيقة لا تغلق المستودع.
3. عند الحصول على فحص المعرفة بالصادرات أو المشكلات، أو تفكيكها عبر قطعة أثرية، أو حفظها في السجل،
   والتحدث مع قضية trekera.