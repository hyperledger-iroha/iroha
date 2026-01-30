---
lang: ja
direction: ltr
source: docs/portal/i18n/ar/docusaurus-plugin-content-docs/current/devportal/reviewer-onboarding.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 29c1889e3cee912666ae16a995b44b045ce363f4ad13ea98e812cba7ca98b445
source_last_modified: "2026-01-03T18:08:00+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/reviewer-onboarding.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# تأهيل مراجعي المعاينة

## نظرة عامة

DOCS-SORA يتابع اطلاقا مرحليا لبوابة المطورين. عمليات البناء المقيدة بالتحقق
(`npm run serve`) وتدفقات Try it المحصنة تفتح المرحلة التالية: تأهيل مراجعين مدققين قبل ان
تفتح المعاينة العامة على نطاق واسع. يشرح هذا الدليل كيفية جمع الطلبات، والتحقق من الاهلية،
وتوفير الوصول، وانهاء المشاركة بشكل امن. ارجع الى
[preview invite flow](./preview-invite-flow.md) لتخطيط الدفعات، وتيرة الدعوات، وصادرات القياس
عن بعد؛ الخطوات ادناه تركز على الاجراءات بعد اختيار المراجع.

- **النطاق:** المراجعين الذين يحتاجون وصولا الى معاينة المستندات (`docs-preview.sora`,
  عمليات بناء GitHub Pages او حزم SoraFS) قبل GA.
- **خارج النطاق:** مشغلو Torii او SoraFS (مغطون بكتيبات onboarding الخاصة بهم) وعمليات نشر
  البوابة الانتاجية (انظر
  [`devportal/deploy-guide`](./deploy-guide.md)).

## الادوار والمتطلبات المسبقة

| الدور | الاهداف المعتادة | الاثار المطلوبة | ملاحظات |
| --- | --- | --- | --- |
| Core maintainer | التحقق من الادلة الجديدة، تشغيل smoke tests. | مقبض GitHub، جهة اتصال Matrix، CLA موقعة في الملف. | غالبا موجود بالفعل في فريق GitHub `docs-preview`; مع ذلك قدم طلبا حتى يكون الوصول قابلا للتدقيق. |
| Partner reviewer | التحقق من مقتطفات SDK او محتوى الحوكمة قبل الاصدار العام. | بريد شركة، جهة اتصال قانونية، شروط preview موقعة. | يجب الاقرار بمتطلبات القياس عن بعد + التعامل مع البيانات. |
| Community volunteer | تقديم ملاحظات قابلية الاستخدام على الادلة. | مقبض GitHub، جهة اتصال مفضلة، منطقة زمنية، قبول CoC. | حافظ على صغر الدفعات؛ اعط الاولوية للمراجعين الذين وقعوا اتفاقية المساهم. |

يجب على جميع انواع المراجعين:

1. الاقرار بسياسة الاستخدام المقبول لاثار المعاينة.
2. قراءة ملاحق الامن/الرصد
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. الموافقة على تشغيل `docs/portal/scripts/preview_verify.sh` قبل تقديم اي
   snapshot محليا.

## سير عمل intake

1. اطلب من مقدم الطلب تعبئة نموذج
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   (او نسخه/لصقه في issue). التقط على الاقل: الهوية، وسيلة الاتصال، مقبض GitHub، تواريخ المراجعة
   المقصودة، وتاكيد ان مستندات الامن تمت قراءتها.
2. سجل الطلب في متتبع `docs-preview` (issue GitHub او تذكرة حوكمة) وعين معتمدا.
3. تحقق من المتطلبات المسبقة:
   - CLA / اتفاقية مساهم في الملف (او مرجع عقد شريك).
   - اقرار الاستخدام المقبول محفوظ داخل الطلب.
   - تقييم مخاطر مكتمل (مثال: مراجعي الشركاء تمت الموافقة عليهم من Legal).
4. يوقع المعتمد على الطلب ويربط issue المتابعة باي ادخال لادارة التغيير
   (مثال: `DOCS-SORA-Preview-####`).

## التزويد والادوات

1. **مشاركة الاثار** — قدم اخر descriptor + ارشيف معاينة من workflow CI او pin SoraFS
   (اثر `docs-portal-preview`). ذكر المراجعين بتشغيل:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **التقديم مع فرض checksum** — وجه المراجعين الى الامر المقيد بالتحقق:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   هذا يعيد استخدام `scripts/serve-verified-preview.mjs` حتى لا يتم تشغيل build غير محقق بالخطأ.

3. **منح وصول GitHub (اختياري)** — اذا احتاج المراجعون الى فروع غير منشورة، اضفهم الى فريق
   GitHub `docs-preview` طوال فترة المراجعة وسجل تغيير العضوية في الطلب.

4. **قنوات الدعم** — شارك جهة الاتصال المناوبة (Matrix/Slack) واجراء الحوادث من
   [`incident-runbooks`](./incident-runbooks.md).

5. **القياس عن بعد + الملاحظات** — ذكر المراجعين بان التحليلات المجهولة يتم جمعها
   (انظر [`observability`](./observability.md)). وفر نموذج الملاحظات او قالب issue المذكور في الدعوة
   وسجل الحدث عبر المساعد [`preview-feedback-log`](./preview-feedback-log) كي يبقى ملخص الموجة محدثا.

## قائمة المراجع

قبل الوصول الى المعاينة، يجب على المراجعين اكمال التالي:

1. التحقق من الاثار التي تم تنزيلها (`preview_verify.sh`).
2. تشغيل البوابة عبر `npm run serve` (او `serve:verified`) لضمان تفعيل حارس checksum.
3. قراءة ملاحظات الامن والرصد المرتبطة اعلاه.
4. اختبار وحدة OAuth/Try it باستخدام تسجيل الدخول device-code (ان كان ذلك ينطبق) وتجنب اعادة استخدام
   رموز الانتاج.
5. تسجيل الملاحظات في المتتبع المتفق عليه (issue او مستند مشترك او نموذج) ووضع وسم اصدار المعاينة.

## مسؤوليات المينتينرز وانهاء المشاركة

| المرحلة | الاجراءات |
| --- | --- |
| Kickoff | تاكيد ان قائمة intake مرفقة بالطلب، مشاركة الاثار + التعليمات، اضافة ادخال `invite-sent` عبر [`preview-feedback-log`](./preview-feedback-log)، وجدولة مزامنة في منتصف الفترة اذا استمرت المراجعة لاكثر من اسبوع. |
| Monitoring | مراقبة قياس المعاينة (حركة Try it غير معتادة، فشل probe) واتباع runbook الحوادث اذا كان هناك شيء مريب. سجل احداث `feedback-submitted`/`issue-opened` عند وصول الملاحظات حتى تبقى مقاييس الموجة دقيقة. |
| Offboarding | الغاء الوصول المؤقت الى GitHub او SoraFS، تسجيل `access-revoked`, ارشفة الطلب (يتضمن ملخص الملاحظات + الاجراءات المعلقة)، وتحديث سجل المراجعين. اطلب من المراجع حذف البنيات المحلية وارفاق الملخص الناتج من [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

استخدم نفس العملية عند تدوير المراجعين بين الموجات. الحفاظ على الاثر داخل المستودع (issue + قوالب)
يساعد DOCS-SORA على البقاء قابلا للتدقيق ويسمح للحوكمة بتاكيد ان وصول المعاينة اتبع الضوابط الموثقة.

## قوالب الدعوة والتتبع

- ابدء كل تواصل بملف
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  يلتقط الحد الادنى من اللغة القانونية وتعليمات checksum للمعاينة وتوقع ان يقر المراجعون بسياسة
  الاستخدام المقبول.
- عند تحرير القالب، استبدل العناصر النائبة لـ `<preview_tag>` و`<request_ticket>` وقنوات الاتصال.
  احفظ نسخة من الرسالة النهائية داخل تذكرة intake حتى يتمكن المراجعون والمعتمدون والمدققون من
  الرجوع الى النص الدقيق الذي تم ارساله.
- بعد ارسال الدعوة، حدث جدول التتبع او issue بطابع `invite_sent_at` وتاريخ الانتهاء المتوقع حتى يتمكن
  تقرير [preview invite flow](./preview-invite-flow.md) من التقاط الدفعة تلقائيا.
