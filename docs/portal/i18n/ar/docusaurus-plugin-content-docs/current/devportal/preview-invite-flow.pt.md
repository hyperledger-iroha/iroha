---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-invite-flow.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# يقوم Fluxo de convites بالمعاينة

## أوبجيتيفو

يقوم عنصر خريطة الطريق **DOCS-SORA** بإلغاء إعداد المراجعة وبرنامج الدعوة لمعاينة عامة مثل آخر أدوات الحظر قبل الإصدار التجريبي من البوابة. توصف هذه الصفحة بأنها تفتح كل مرة من المكالمات، حيث يجب أن يتم إرسالها قبل بدء المكالمات وإثبات التدفق والتدقيق. استخدم جونتو كوم:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) لإدارة المراجعة.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) لضمانات المجموع الاختباري.
- [`devportal/observability`](./observability.md) لتصدير أدوات القياس عن بعد وخطافات التنبيه.

## بلانو دي أونداس

| اوندا | الجمهور | معايير الدخول | معايير الصيدة | نوتاس |
| --- | --- | --- | --- | --- |
| **W0 - جوهر المشرفين** | المشرفون على Docs/SDK صالحون للمحتوى في هذا اليوم. | الوقت GitHub `docs-portal-preview` شعبي، بوابة المجموع الاختباري `npm run serve` أخضر، مدير التنبيهات صامت لمدة 7 أيام. | جميع مستندات P0 تمت مراجعتها، وتراكم الأعمال المتراكمة، دون أي حوادث. | تستخدم للتحقق من التدفق. بمجرد إرسال رسالة بريد إلكتروني، فقط قم بمشاركة عناصر المعاينة. |
| **W1 - الشركاء** | المشغلون SoraFS، المتكاملون Torii، مراجعو الحوكمة sob NDA. | W0 مكتمل، شروط قانونية معتمدة، وكيل جربه في التدريج. | تسجيل الخروج من الشركاء المجمعين (الإصدار أو الصيغة المعطلة)، عرض القياس عن بعد <= 10 مراجعة متزامنة، بدون تراجعات أمان لمدة 14 يومًا. | تطبيق قالب الدعوة + تذاكر الاستدعاء. |
| **W2 - الجماعة** | تم اختيار المساهمين من قائمة أمل المجتمع. | تم الانتهاء من W1، وتدريبات على الأحداث الرائعة، وتم تحديث الأسئلة الشائعة العامة. | تم إرسال التعليقات >=إصدارين من المستندات المرسلة عبر مسار المعاينة دون التراجع. | يتضمن الحد المتزامنات (<=25) ويتم تجميعها بشكل منفصل. |

قم بتوثيق ما هو موجود في `status.md` ولا يمكنك تتبع طلبات المعاينة لإدارة الحالة بسرعة.

## قائمة التحقق من الاختبار المبدئي

اختتام هذه الكلمات **المسبقة** لجدول الأعمال المدعو لشخص ما:

1. **Artefatos de CI disponiveis**
   - آخر `docs-portal-preview` + الواصف المرسل لـ `.github/workflows/docs-portal-preview.yml`.
   - دبوس SoraFS موضح في `docs/portal/docs/devportal/deploy-guide.md` (واصف القطع الحالي).
2. **تنفيذ المجموع الاختباري**
   - استدعاء `docs/portal/scripts/serve-verified-preview.mjs` عبر `npm run serve`.
   - تعليمات `scripts/preview_verify.sh` التي تم اختبارها على macOS + Linux.
3. ** خط الأساس للقياس عن بعد **
   - `dashboards/grafana/docs_portal.json` Mostra trafego جربه saudavel e o تنبيه `docs.preview.integrity` esta verde.
   - تم تحديث آخر ملحق لـ `docs/portal/docs/devportal/observability.md` مع روابط Grafana.
4. **مصنوعات الحكم**
   - إصدار دعوة تعقب برونتا (uma Issue por onda).
   - نموذج سجل المراجعات المنسوخ (الإصدار [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - إجراءات قانونية وSRE تتطلب إضافة مشكلة.

سجل نتيجة الاختبار المبدئي دون دعوة للتعقب قبل إرسال أي بريد إلكتروني.

## إيتاباس دو فلوكسو

1. **المرشحون المختارون**
   - Puxar da Planilha de Espera أو عائلة الشركاء.
   - ضمان أن كل مرشح لديه نموذج طلب كامل.
2. **أبروفار أسيسو**
   - Atribuir um aprovador aمسألة القيام بدعوة تعقب.
   - التحقق من المتطلبات الأساسية (CLA/contrato، uso aceitavel، Summary de seguranca).
3. ** إنفيار يدعو **
   - قم بإظهار العناصر النائبة لـ [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`، `<request_ticket>`، جهات الاتصال).
   - إضافة واصف + أرشيف تجزئة، وعنوان URL للتدريج، وقناة الدعم.
   - حماية البريد الإلكتروني النهائي (أو نسخة Matrix/Slack) من المشكلة.
4. **التأهيل المرافق**
   - تحديث أو دعوة تعقب com `invite_sent_at`, `expected_exit_at`, الحالة الإلكترونية (`pending`, `active`, `complete`, `revoked`).
   - رابط لطلب إدخال المراجعة للتدقيق.
5. ** مراقبة القياس عن بعد **
   - مراقبة `docs.preview.session_active` والتنبيهات `TryItProxyErrors`.
   - فتح حادثة في حالة انتهاء القياس عن بعد من خط الأساس وتسجيل النتيجة بعد دخول المكالمة.
6. **التعليقات والملاحظات الجماعية**
   - قم بالاستدعاء عند بدء تشغيل الملاحظات أو انتهاء صلاحية `expected_exit_at`.
   - تحديث المشكلة الموجودة مع ملخص مختصر (الأحداث، الأحداث، النقاط القريبة) قبل المرور إلى المنطقة القريبة.

## التقارير الإلكترونية الأدلة

| ارتيفاتو | أوندي أرمازينار | إيقاع التحديث |
| --- | --- | --- |
| قضية القيام بدعوة تعقب | بروجيتو جيثب `docs-portal-preview` | Atualizar apos cada convite. |
| تصدير قائمة المراجعة | تم التسجيل في `docs/portal/docs/devportal/reviewer-onboarding.md` | سيمانال. |
| لقطات القياس عن بعد | `docs/source/sdk/android/readiness/dashboards/<date>/` (إعادة استخدام حزمة القياس عن بعد) | بسبب حوادث + apos. |
| ملخص التعليقات | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (تحضير المعكرونة بوروندا) | في غضون 5 أيام بعد أن وصلت إلى هناك. |
| مذكرة إعادة توحيد الحكم | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | قم بالمزامنة المسبقة لكل DOCS-SORA. |

نفذ `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
كل الكثير من أجل إنتاج ملخص قانوني للآلة. تم إصدار ملف JSON المرفق لتأكيد مراجعة الإدارة كرسائل دعوة دون إعادة إنتاج كل شيء أو تسجيل الدخول.

قم بإرفاق قائمة الأدلة إلى `status.md` حتى تنتهي دائمًا حتى يتم تحديث مدخل خريطة الطريق بسرعة.

## معايير التراجع والإيقاف المؤقت

إيقاف تدفق المكالمات مؤقتًا (إشعار الإدارة) عند حدوث أي من العناصر التالية:

- حادثة الوكيل حاول تجربة التراجع (`npm run manage:tryit-proxy`).
- مجموعة التنبيهات: >3 صفحات تنبيه لنقاط النهاية قبل المعاينة في 7 أيام.
- فجوة الامتثال: قم بإرسال دعوة دون شروط ملغاة أو بدون مسجل أو نموذج طلب.
- خطر التكامل: عدم تطابق المجموع الاختباري المكتشف بواسطة `scripts/preview_verify.sh`.

قم بعد ذلك بتوثيق عملية الإصلاح بدون دعوة متتبع وتأكيد أن لوحة القياس عن بعد قد تم وضعها لمدة لا تقل عن 48 ساعة.