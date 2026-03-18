---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# عرض بلاي بوك للمعاينة العامة

## برامج سيلي

هذا هو المقصود من النشرة العامة عندما تقوم بالموافقة على المعاينة العامة وتوفيرها بعد ذلك، كما هو الحال
تم بدء تشغيل سير العمل. من خلال دعم البطاقات التالية DOCS-SORA,
ضمان أن يتم التحقق من القطع الأثرية والتعليمات
مع السلامة وقناة اتصال جيدة.

- **تدقيق الحسابات:** قائمة تضم كل من الشركاء والشركاء والشركاء الذين
  подписали poliтику مقبول الاستخدام للمعاينة.
- **المزايا:** حجم السعة <= 25 مراجعة، التسليم في غضون 14 يومًا، الرد على
  الأحداث في التدريب 24 ساعة.

## بوابة الاختيار قبل المغادرة

هذه هي الفوائد التي سبقت النقل:

1. المعاينة اللاحقة للقطع الأثرية المخزنة في CI (`docs-portal-preview`,
   بيان المجموع الاختباري، الواصف، حزمة SoraFS).
2. `npm run --prefix docs/portal serve` (بوابة المجموع الاختباري) تم اختبارها على هذه العلامة.
3. تذاكر الصعود معتمدة ومرتبطة بالدفع الكامل.
4. وثائق الأمن وقابلية المراقبة والتحقق من الحوادث
   ([`security-hardening`](./security-hardening.md)،
   [`observability`](./observability.md)،
   [`incident-runbooks`](./incident-runbooks.md)).
5. نموذج التعليقات المكتملة أو قالب المشكلة (درجة الخطورة، بعض ردود الفعل،
   اللقطات والمعلومات المتعلقة بالنشر).
6. تم التحقق من النص Docs/DevRel + Governance.

## حزمة الحماية

عند الحاجة إلى التضمين:

1. **العناصر التي تم التحقق منها** — قم بالاتصال ببيان/خطة SoraFS أو عناصر GitHub،
   بالإضافة إلى بيان المجموع الاختباري والواصف. من المؤكد أن تقوم بأمر التحقق من ذلك
   يمكن للمراجعين إغلاق موقعهم الإلكتروني قبل إغلاقه.
2. **خدمات التعليمات** — قم بإدراج أمر المعاينة باستخدام بوابة المجموع الاختباري:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **توصية بالسلامة** — تأكد من أن الرموز موجودة تلقائيًا، بدون انقطاع
   من المهم أن تكون الأحداث غير منتظمة.
4. **قناة الاتصال الخاصة** — قم بالاتصال بنموذج/نموذج المشكلة وقم بتوضيح الخدمة خلال الوقت المحدد.
5. **برامج البيانات** — تمكين البيانات الأولية/الموافقة وساعات العمل أو المزامنة والتحديث التالي.

الرسالة الأولية في
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
قم بإظهار هذا الطلب. قم بتعيين العناصر النائبة (البيانات وعناوين URL وجهات الاتصال)
قبل الانتهاء.

## مضيف معاينة النشر

قم بترقية مضيف المعاينة فقط بعد إنهاء عملية النقل وإلغاء تذكرة التغيير.
سم. [Руководство по Exposure Preview Host](./preview-host-exposure.md) للعرض الشامل
إنشاء/نشر/تحقق، يتم استخدامه في هذا المكان.

1. **الإنشاء والتعبئة:** قم بتثبيت علامة الإصدار وإنشاء العناصر المحددة.

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

   تم كتابة دبوس البرنامج النصي `portal.car`، `portal.manifest.*`، `portal.pin.proposal.json`،
   و `portal.dns-cutover.json` في `artifacts/sorafs/`. استخدم هذه الملفات بالحجم الكامل
   يرجى ملاحظة أن كل جهاز كمبيوتر يمكنه التحقق من ذلك.

2. ** الاسم المستعار لمعاينة النشر:** أرسل الأمر بدون `--skip-submit`
   (تظهر `TORII_URL`، `AUTHORITY`، `PRIVATE_KEY[_FILE]`، والاسم المستعار للإدارة).
   تمكين البرنامج النصي من البيان إلى `docs-preview.sora` والتحقق منه
   `portal.manifest.submit.summary.json` بالإضافة إلى `portal.pin.report.json` لحزمة الأدلة.

3. **تجربة تجريبية:** تابع ما هو الاسم المستعار الذي تم تغييره والمجموع الاختباري لعلامة الموافقة
   قبل التسليم النهائي.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   اتبع `npm run serve` (`scripts/serve-verified-preview.mjs`) كخيار احتياطي،
   لكي يتمكن المراجعون من عمل نسخة محلية، إذا قمت بمعاينة الحافة حتى الآن.

## التواصل الزمني

| اليوم | الحقيقة | المالك |
| --- | --- | --- |
| د-3 | الانتهاء من إرسال النص واكتشاف العناصر والتحقق من التشغيل الجاف | مستندات/ديفريل |
| د-2 | توقيع الحوكمة + تذكرة التغيير | Docs/DevRel + الحوكمة |
| د-1 | قم بالتحكم في التتبع من خلال شابلونو، قم بتكوين جهاز تعقب مع المستفيدين الموضحين | مستندات/ديفريل |
| د | مكالمة البداية / ساعات العمل، مراقبة القياس عن بعد | Docs/DevRel + عند الطلب |
| د+7 | ملخص التغذية الراجعة الداعمة، فرز القضايا المحظورة | مستندات/ديفريل |
| د+14 | قم بإغلاق المجلد، قم بالتوصيل الفوري، قم بنشر الملخص في `status.md` | مستندات/ديفريل |

## توصيل الرحلات والقياس عن بعد

1. قم بتسجيل كل ما تم استلامه، وإخطار الطابع الزمني، ومراجعة التاريخ في مسجل ملاحظات المعاينة
   (اسم. [`preview-feedback-log`](./preview-feedback-log)), لكي يتم حل المشكلة بالكامل
   واحد وهذا هو دليل الأدلة:

   ```bash
   # Добавить новое событие приглашения в artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   ملحقات إضافية: `invite-sent`، `acknowledged`، `feedback-submitted`،
   `issue-opened`، و`access-revoked`. سجل التشجيع موجود في
   `artifacts/docs_portal_preview/feedback_log.json`; استخدم نفسك من خلال تذكرة السفر
   استمتع بالتنسيق مع الأشكال. استخدم الملخص المساعد لتمكينك من الاستماع إلى الصوت
   نشمر قبل النهاية النهائية:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   ملخص JSON праечисляет праглазения по волнам, отклыты плучателей, четчики ردود الفعل
   والطابع الزمني بعد الزواج. مساعد رئيسي
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)،
   يمكن تشغيل هذا وسير العمل محليًا أو في CI. استخدم شابلون دايجست في
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   قبل نشر خلاصة volny.
2. تشغيل لوحات المعلومات عن بعد `DOCS_RELEASE_TAG`، المستخدمة للالتقاط
   يمكن أن يكون متاحًا مع مجموعة من المشاركين.
3. قم بتثبيت `npm run probe:portal -- --expect-release=<tag>` بعد النشر للتأكيد،
   من أجل معاينة الشاشة، يتم عرض البيانات الوصفية الصحيحة للإصدار.
4. تثبت الأحداث المفضلة في دليل التشغيل وتتواصل مع المجموعة.

## ردود الفعل والخلاص

1. قم بإرسال التعليقات إلى المستند الرئيسي أو إلى لوحة الإصدار. تحديد العناصر `docs-preview/<wave>`,
   يمكن أن نصل إلى أصحاب خريطة الطريق.
2. استخدم مسجل معاينة ملخص للمغادرة، ثم قم بإدراج المجموعة في
   `status.md` (الأزرار، المفاتيح الرئيسية، الإصلاحات المخططة) واكتشف `roadmap.md`،
   إذا تم تغيير المعلم الرئيسي DOCS-SORA.
3. اتبع الخطوات اللازمة للخروج من الطائرة
   [`reviewer-onboarding`](./reviewer-onboarding.md): قم بالموافقة على التوصيل وأرشفة الملفات،
   شكرا للأصدقاء.
4. قم بالمتابعة التالية، والقطع الأثرية، والبوابات الاختبارية القابلة للتحويل، وما إلى ذلك
   قم بملاحظة شيبلون بالبيانات الجديدة.

التحسين التالي لهذا البرنامج هو برنامج معاينة قابل للتدقيق وهذا
Docs/DevRel هو الاحتمال الأفضل للاشتراك في بوابة المشاركة فقط في GA.