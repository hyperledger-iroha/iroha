---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-الخطة
العنوان: W1 شراكة داروں کے لئے پری فلائٹ پلان
Sidebar_label: خطة W1
الوصف: معاينة الشريك لكل من ٹاسكس، مالكان، وثبوت چیك لٹ.
---

| ئٹم | تفاصيل |
| --- | --- |
| لہر | W1 - الشركاء ومتكاملو Torii |
| ہدف ونڈو | الربع الثاني 2025 ہفتہ 3 |
| آرٹیفیکٹ ٹیگ (منصوبہ) | `preview-2025-04-12` |
| ٹریکر ایشو | `DOCS-SORA-Preview-W1` |

##مقاصد

1. يمكن الحصول على شريط معاينة الشريك المرخص قانونًا وهدفه الوصول إليه.
2. قم بتجريب استخدام الوكيل ولقطات القياس عن بعد.
3. يقوم المجموع الاختباري بالتحقق من معاينة القطعة الأثرية والتحقيق في النتائج الحالية.
4. دعوة إلى قائمة الشركاء وقوالب الطلب.

## حجر الطوب

| معرف | ٹاسك | مالك | مقررہ تاريخ | ٹیٹس | أخبار |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | معاينة الشروط الملحق کے لئےی قانونی حاصلی کرنا | Docs/DevRel Lead -> قانوني | 2025-04-05 | ✅ مكمل | قانون ٹکٹ `DOCS-SORA-Preview-W1-Legal` 2025-04-05 هو المقصود؛ PDF مُسجل بواسطة منسلك. |
| W1-P2 | جربه الوكيل کا التدريج ونڈو (2025-04-10) محفوظ کرنا وصحة الوكيل کی تصدیق | مستندات/DevRel + Ops | 2025-04-06 | ✅ مكمل | `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` 2025-04-06 كليا جايا؛ نسخة CLI و`.env.tryit-proxy.bak` محفوظه کر دیے گئے. |
| W1-P3 | معاينة القطعة الأثرية (`preview-2025-04-12`) بنانا، `scripts/preview_verify.sh` + `npm run probe:portal` چلانا، واصف/مجموعات اختبارية محفوظ کرنا | بوابة TL | 2025-04-08 | ✅ مكمل | المصنوعات اليدوية وسجلات التحقق `artifacts/docs_preview/W1/preview-2025-04-12/` محفوظ ہیں؛ إخراج المسبار ٹریکر کے ساتھ منسلک ہے. |
| W1-P4 | نماذج قبول المشاركين (`DOCS-SORA-Preview-REQ-P01...P08`) الجوائز وجهات الاتصال واتفاقيات عدم الإفشاء | الاتصال بالحوكمة | 2025-04-07 | ✅ مكمل | كل هذا هو الهدف النهائي (آخر يوم 2025-04-11 هو الغد)؛ الموافقات ٹریکر لنك ہیں. |
| W1-P5 | دعوة لدعوة جديدة (`docs/examples/docs_preview_invite_template.md` للأبناء)، وشريك لـ `<preview_tag>` و`<request_ticket>` سيت كرنا | مستندات/DevRel الرصاص | 2025-04-08 | ✅ مكمل | دعوة للسوداء 12-04-2025 15:00 بالتوقيت العالمي المنسق (UTC) قطعة أثرية لنعلن عنها اليوم. |

## الاختبار المبدئي چیک لٹ

> الجزء: `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` يتم تنفيذ المراحل من 1 إلى 5 للشبكة (الإنشاء، والتحقق من المجموع الاختباري، ومسبار البوابة، ومدقق الارتباط، وجرب تحديث الوكيل). إنه سكربت JSON لاغ بنتا وهو يسے ٹریکر یشو وهو یا سلک یا کتا ہے.

1. `npm run build` (`DOCS_RELEASE_TAG=preview-2025-04-12` متصل) حتى `build/checksums.sha256` و`build/release.json` مكرر.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4. `DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` و`build/link-report.json` هو الواصف الذي يعمل على أرشفة الملفات.
5.`npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (أو الهدف المناسب `--tryit-target`)؛ تم إجراء هذا على `.env.tryit-proxy` وهو الالتزام بالتمرير والتراجع عن `.bak`.
6. يقوم W1 بتسجيل المسارات باستخدام المجموع الاختباري للواصف، وإخراج المسبار، وتبديل الوكيل، ولقطات Grafana).

## ثبوت چیک لٹ

- [x] تم إصدار نص قانوني للطعن (PDF أو ٹکٹ لنك) `DOCS-SORA-Preview-W1` وهو ما تم التوصل إليه من قبل.
- [x] `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals` کے Grafana لقطات الشاشة.
- [x] واصف `preview-2025-04-12` وسجل المجموع الاختباري `artifacts/docs_preview/W1/` تحت المحفوظات.
- [x] جدول قائمة الدعوات موجود في `invite_sent_at` الطوابع الزمنية مكتمل (سجل W1 للمسجل).
- [x] عناصر التعليقات [`preview-feedback/w1/log.md`](./log.md) نظرة عامة، شريك في صف واحد (26/04/2025 القائمة/القياس عن بعد/القضايا، إلخ. ڈي)).

استمتع باللعبة أو خطة كل لعبة؛ ٹریکر هي خارطة طريق لقابلية التدقيق من أجل الحصول على حوالة ديتا.

## فيڈبیک وک فلو

1. ہر المراجع کے لئے
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md) قالب بطاقة الائتمان،
   كل شيء آخر ومكتمل
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/` متجدد.
2. الدعوات ونقاط التفتيش والقياس عن بعد والقضايا المفتوحة کو
   [`preview-feedback/w1/log.md`](./log.md) سجل مباشر يوفر خلاصة لمراجعي الحوكمة
   مستودع الاندرويد إعادة التشغيل.
3. فيما يتعلق بالتحقق من المعرفة أو صادرات الاستطلاع، يمكنك إنشاء سجل لمسار المصنوعات اليدوية ثم إرفاقه
   ومشكلة التعقب هي الرابط.