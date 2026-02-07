---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-feedback/w1/plan.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
المعرف: معاينة ردود الفعل W1-الخطة
العنوان: Plano de preflight de parceiros W1
Sidebar_label: بلانو W1
الوصف: المهام والردود وقائمة الأدلة المرجعية لمجموعة معاينة المشاركات.
---

| العنصر | ديتالز |
| --- | --- |
| اوندا | W1 - التكامل والتكامل Torii |
| جانيلا ألفو | الربع الثاني 2025 سيمانا 3 |
| Tag de artefato (planejado) | `preview-2025-04-12` |
| قضية تفعل تعقب | `DOCS-SORA-Preview-W1` |

##الأهداف

1. ضمان الموافقة القانونية والحوكمة على شروط معاينة المشاركة.
2. قم بإعداد الوكيل، جربه وقم بتجميع لقطات القياس عن بعد المستخدمة بدون حزمة دعوة.
3. قم بتحديث أداة المعاينة التي تم التحقق منها من خلال المجموع الاختباري ونتائج التحقيقات.
4. قم بإكمال قائمة المشاركات ونماذج الطلبات قبل إرسال دعوتك.

## إلغاء الطلب

| معرف | طريفة | الرد | برازو | الحالة | نوتاس |
| --- | --- | --- | --- | --- | --- |
| W1-P1 | الحصول على موافقة قانونية لإضافة شروط المعاينة | Docs/DevRel Lead -> قانوني | 2025-04-05 | الخلاصة | التذكرة قانونية `DOCS-SORA-Preview-W1-Legal` تمت الموافقة عليها في 2025-04-05؛ PDF anexado ao Tracker. |
| W1-P2 | التقط صورة التدريج للوكيل جربها (10/04/2025) وتحقق من صحة عمل الوكيل | مستندات/DevRel + Ops | 2025-04-06 | الخلاصة | تم تنفيذ `npm run manage:tryit-proxy -- --stage preview-w1 --expires-in=21d --target https://tryit-preprod.sora` في 06-04-2025; نسخ CLI ومحفوظات `.env.tryit-proxy.bak`. |
| W1-P3 | إنشاء معاينة فنية (`preview-2025-04-12`)، قضيب `scripts/preview_verify.sh` + `npm run probe:portal`، واصف/مجموعات اختبارية | بوابة TL | 2025-04-08 | الخلاصة | تم إنشاء سجلات التحقق والمخزنة في `artifacts/docs_preview/W1/preview-2025-04-12/`؛ صيدا دي التحقيق anexada ao تعقب. |
| W1-P4 | مراجعة صيغ كمية الأجزاء (`DOCS-SORA-Preview-REQ-P01...P08`)، وتأكيد الاتصالات واتفاقيات عدم الإفشاء | الاتصال بالحوكمة | 2025-04-07 | الخلاصة | كطلب مسبق (كما هو الحال في آخر دعويين في 11/04/2025)؛ aprovacoes linkadas لا يوجد تعقب. |
| W1-P5 | قم بإعادة الاتصال (استنادًا إلى `docs/examples/docs_preview_invite_template.md`)، وتحديد `<preview_tag>` و`<request_ticket>` لكل قطعة | مستندات/DevRel الرصاص | 2025-04-08 | الخلاصة | أرسل Rascunho إرسال الدعوة في 12-04-2025 الساعة 15:00 بالتوقيت العالمي المنسق مع روابط com artefato. |

## قائمة التحقق من الاختبار المبدئي

> Dica: ركب `scripts/preview_wave_preflight.sh --tag preview-2025-04-12 --base-url https://preview.staging.sora --descriptor artifacts/preview-2025-04-12/descriptor.json --archive artifacts/preview-2025-04-12/docs-portal-preview.tar.zst --tryit-target https://tryit-proxy.staging.sora --output-json artifacts/preview-2025-04-12/preflight-summary.json` لتنفيذ المرور تلقائيًا 1-5 (الإنشاء والتحقق من المجموع الاختباري والتحقيق في البوابة ومدقق الارتباط وتحديث الوكيل جربه). يتم تسجيل البرنامج النصي في سجل JSON الذي يمكنك من خلاله إضافة ملف التتبع إلى المشكلة.

1. `npm run build` (com `DOCS_RELEASE_TAG=preview-2025-04-12`) لإعادة إنشاء `build/checksums.sha256` و`build/release.json`.
2.`docs/portal/scripts/preview_verify.sh --build-dir docs/portal/build --descriptor artifacts/<tag>/descriptor.json --archive artifacts/<tag>/docs-portal-preview.tar.zst`.
3.`PORTAL_BASE_URL=https://preview.staging.sora DOCS_RELEASE_TAG=preview-2025-04-12 npm run probe:portal -- --expect-release=preview-2025-04-12`.
4.`DOCS_RELEASE_TAG=preview-2025-04-12 npm run check:links` واسترجاع `build/link-report.json` من خلال الواصف.
5.`npm run manage:tryit-proxy -- update --target https://tryit-proxy.staging.sora` (أو تجاوز الهدف المناسب عبر `--tryit-target`)؛ قم بتنفيذ `.env.tryit-proxy` للتحديث والحماية إلى `.bak` للتراجع.
6. قم بتحديث إصدار W1 مع سجلات السجلات (المجموع الاختباري للواصف، مسبار التحقيق، غير معتمد، جربه ولقطات Grafana).

## قائمة التحقق من الأدلة

- [x] الموافقة القانونية (PDF ou link do Ticket) anexada ao `DOCS-SORA-Preview-W1`.
- [x] لقطات شاشة من Grafana لـ `docs.preview.integrity`، `TryItProxyErrors`، `DocsPortal/GatewayRefusals`.
- [x] الواصف وسجل المجموع الاختباري `preview-2025-04-12` المُخزن في `artifacts/docs_preview/W1/`.
- [x] جدول قائمة المكالمات مع `invite_sent_at` المسبق (إصدار سجل W1 بدون متعقب).
- [x] ردود الفعل المرتدة من [`preview-feedback/w1/log.md`](./log.md) مع خط مبسط (تم التحديث في 26-04-2025 مع بيانات القائمة/القياس عن بعد/المشاكل).

Atualize esteplano conforme as tarefas avancarem; o متعقب أو مرجعية لمراجعة خارطة الطريق.

## تدفق ردود الفعل

1. لكل مراجع، قم بتكرار القالب
   [`docs/examples/docs_preview_feedback_form.md`](../../../../../examples/docs_preview_feedback_form.md)،
   قم بحفظ التعريفات وخزن نسخة كاملة منها
   `artifacts/docs_preview/W1/preview-2025-04-12/feedback/<partner-id>/`.
2. استئناف المكالمات ونقاط التفتيش للقياس عن بعد وإصدار الفتحات داخل سجل الحياة
   [`preview-feedback/w1/log.md`](./log.md) حتى يتمكن مراجعو الحوكمة من الرجوع إلى Onda
   سيم ساير دو مستودع.
3. عند تصدير المعرفة أو التحقق من الدراسات الاستقصائية، قم بإضافة أي مسار من الأعمال الفنية المشار إليها بدون سجل
   رابط أو مشكلة تفعل تعقب.