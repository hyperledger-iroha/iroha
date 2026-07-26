---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل دعوات المعاينة العامة

## اهداف البرنامج

يوضح هذا الدليل كيفية تشغيل المعاينة العامة بعد تفعيل سير عمل تاهيل المراجعين. تحافظ على
صدق خارطة الطريق DOCS-SORA عبر ضمان ان كل دعوة تتضمن اثارا فقط، اتجاهات أمنية، ومسارا
واضحا للتغذية المراجعة.

- **الجمهور:** قائمة منقحة من أعضاء المجتمع والشركاء والمشرفين الذين وقعوا على ضرورة البناء للمعاينة.
- **الحدود:** الاشتراك الصوتي الافتراضي <= 25 زائر، نافذة الوصول 14 يومًا، للملفات خلال 24 ساعة.

## قائمة فحص بوابة الاطلاق

اكمل هذه الأغنية السابقة اي ترجمة:

1. اخر اثار المعاينة مرفهة في CI (`docs-portal-preview`,
   المجموع الاختباري للبيان، الواصف، الحزمة SoraFS).
2. `npm run --prefix docs/portal serve` (مقيد بالمجموع الاختباري) تم اختباره على نفس العلامة.
3. تذكرة تاهيل المراجعين معتمدة ومربوطة بموجة الأحداث.
4. المستندات الأمنية والمراقبة والحوادث
   ([`security-hardening`](./security-hardening.md)،
   [`observability`](./observability.md)،
   [`incident-runbooks`](./incident-runbooks.md)).
5. نموذج التغذية الراجعة او قالب اصدار مخصص (يتضمن تغطية الكثافة، خطوات إعادة الإنتاج، لقطات الشاشة، ومعلومات البيئة).
6. نسخة اعلان تمت مراجعتها من Docs/DevRel + Governance.

##حزمة الأحداث

يجب ان تتضمن كل دعوة:

1. **اثار متحقق منها** — روابط الروابط Manifest/plan لـ SoraFS او artefact من GitHub
   إضافة الى المجموع الاختباري الواضح وdescriptor. اذكر التحقق من الصراحة حتى تقبل المراجعون
   من قبل تشغيل الموقع.
2. **تعليمات التشغيل** — ادرج أمر المعاينة المقيد بالمجموع الاختباري:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **تذكيرات أمنية** — توضيح ان الكتابة تنتهي حصريا، والروابط لا يجب عليها النص،
   ويجب الابلاغ عنه فورا.
4. **قناة التغذية الراجعة** — ربط نموذج/قالب مشكلة وتوضيح توقعات زمن الانتظار.
5. **تواريخ البرنامج** — نقطة البداية/النهاية، ساعات الويب او جلسات المزامنة،
   ونافذة التحديث التالي.

البريد النموذجي في
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
تغطي هذه المتطلبات. تحديد العنصر النائبة (التواريخ، عناوين URL، رؤية الاتصال)
قبل الارسال.

## اظهار ضيوف المعاينة

لا تروج لمضيف المعاينة الا القانوني بعد التاهيل و اعتماد تذكرة التغيير. إعادة النظر
[دليل اظهار مضيف المعاينة](./preview-host-exposure.md) لخطوات بناء/نشر/تحقق من البداية للنهاية
المستخدمة في هذا القسم.

1. **البناء والتعبئة:** ضع Release tag واثارا حتمية.

   ```bash
   cd docs/portal
   export DOCS_RELEASE_TAG="preview-$(date -u +%Y%m%dT%H%M%SZ)"
   npm ci
   npm run build
   ./scripts/sorafs-pin-release.sh \
     --alias docs-preview.sora \
     --alias-namespace docs \
     --alias-name preview \
     --pin-label docs-preview \
     --skip-submit
   node scripts/generate-preview-descriptor.mjs \
     --manifest artifacts/checksums.sha256 \
     --archive artifacts/sorafs/portal.tar.gz \
     --out artifacts/sorafs/preview-descriptor.json
   ```

   سكربت دبوس كاتب `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   و`portal.dns-cutover.json` تحت `artifacts/sorafs/`. ارفق هذه الملفات بموجة الأحداث
   حتى يتم قبول كل طلب من التحقق من نفس البتات.

2. **نشر الاسم المستعار المعاينة:** اعد الأمر تشغيل بدون `--skip-submit`
   (قدم `TORII_URL`، `AUTHORITY`، `PRIVATE_KEY[_FILE]`، واثبات الاسم المستعار ومؤشر التزايد).
   السكربت بربط المانيفست بـ `docs-preview.sora` ويصدر
   `portal.manifest.submit.summary.json` و`portal.pin.report.json` لأزمة المعادلة.

3. **فحص النشر:** متأكد من ان الاسم المستعار يحل وانه المجموع الاختباري يطابق العلامة قبل ارسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   احتفظ بـ `npm run serve` (`scripts/serve-verified-preview.mjs`) كخيار احتياطي ليتمكن
   المراجعون من تشغيل نسخة اذا حدث خلل في Preview edge.

## جدول الاتصالات

| اليوم | الاجراء | المالك |
| --- | --- | --- |
| د-3 | انهاء نص الحادث، تحديث الاثار، تنفيذ تجربة جافة | مستندات/ديفريل |
| د-2 | موافقة الإدماج + تذكرة التغيير | Docs/DevRel + الحوكمة |
| د-1 | إرسال الدعوات باستخدام موقع، تحديث Tracker بقائمة المرسلين | مستندات/ديفريل |
| د | مكالمة البداية / ساعات العمل، مراقبة لوحات القياس | Docs/DevRel + عند الطلب |
| د+7 | ملخص للملاحظات في منتصف الاستماع، فرز للقضايا المانعة | مستندات/ديفريل |
| د+14 | الاغلاق، الوصول الغاء مؤقت، نشر ملخص في `status.md` | مستندات/ديفريل |

## تتبع الوصول والقياس عن بعد

1. سجل كل ما تم استلامه و طابعات وقت الحادث الالغاء باستخدام معاينة مسجل الملاحظات
   (انظر [`preview-feedback-log`](./preview-feedback-log)) حتى تشترك في كل موجة
   في نفس التأثير المعادل:

   ```bash
   # اضافة حدث دعوة جديد الى artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   الاحداث الجديدة هي `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`، و`access-revoked`. يعيش افتراضيا في
   `artifacts/docs_portal_preview/feedback_log.json`; ارفقه بتذكر موجة الأحداث
   مع الموافقات. استخدم مساعد ملخص لانتاج رول اب قابل للدقيق من قبل
   ملاحظة الاغلاق:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   ملخص JSON يعد الدعوات لكل موجة، المستلمين الجددين، تعداد ردود الفعل، وطابعة
   الوقت لاخر حدث. المساعد يعتمد على
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)،
   يمكن لذلك تشغيل نفس سير العمل محليا او في CI. استخدم قالب الملخص في
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   عند نشر خلاصة للموجة.
2. ضع إشارة `DOCS_RELEASE_TAG` المستخدم للموجة على لوحات القياس حتى يمكن ربطها
   مشاهدات مع فوج الدعوات.
3. الوظيفة `npm run probe:portal -- --expect-release=<tag>` بعد النشر لتاكيد
   ان بيئة المعاينة تعلن عن الاصدار الصحيح للبيانات الوصفية.
4. سجل اي حادثة في قالب runbook واربطها بالمجموعة.

## ردود الفعل والاغلاق

1. اجمع ردود الفعل في القضايا المشتركة او اللوحة الأصلية. ضع إشارة `docs-preview/<wave>` حتى متقدمة
   اصحاب خريطة الطريق من العثور عليها بسهولة.
2. استخدم مخرجات ملخص من مسجل المعاينة لملء إقرار، ثم لخص المجموعة في
   `status.md` (المشاركون، باحثون، الاصلاحات الخاصة بها) وتحديث `roadmap.md`
   اذا تغير معلم DOCS-SORA.
3. اتبع خطوات offboarding من
   [`reviewer-onboarding`](./reviewer-onboarding.md): الغاء الوصول، ارشفطلب، واشكر المشاركين.
4. جهز التسجيل التالي عبر تحديث الاثار، إعادة البوابات الخاصة بالمجموع الاختباري،
   وتحديث قالب الأحداث بمواعيد جديدة.

تطبيق هذا الدليل بشكل متسق للتحكم بقابلية تدقيق برنامج المعاينة ويمنح Docs/DevRel
طريقة قابلة للتكرار لتوسيع الدعوات مع البوابة من GA.