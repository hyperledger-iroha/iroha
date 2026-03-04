---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# معاينة كتاب اللعب للدعوة عامة

## كائنات البرنامج

يشرح كتاب قواعد اللعبة هذا التعليق المعلن ويعرض جولة المعاينة العامة مرة واحدة
سير العمل في إعداد المراجعين نشط. حارس خريطة الطريق DOCS-SORA جيد جدًا
أؤكد أن كل دعوة جزء من القطع الأثرية التي يمكن التحقق منها، والشحنات الآمنة
et un chemin clair pour le ردود الفعل.

- **الجمهور:** قائمة بأعضاء المجتمع والشركاء والمشرفين الذين يعملون
  علامة على سياسة الاستخدام المقبولة للمعاينة.
- **Plafonds:** حجم الغموض الافتراضي <= 25 مراجعًا، نافذة الوصول لمدة 14 يومًا،
  الاستجابة للحوادث على مدار 24 ساعة.

## قائمة التحقق من بوابة الرمح

إنهاء هذه العلامات قبل إرسال دعوة:

1. رسوم المعاينة النهائية لرسوم المعاينة في CI (`docs-portal-preview`،
   بيان المجموع الاختباري، الواصف، الحزمة SoraFS).
2.`npm run --prefix docs/portal serve` (المجموع الاختباري للبوابة) اختبر علامة meme.
3. توافق تذاكر تأهيل المراجعين على الدعوة المبهمة.
4. المستندات آمنة وقابلة للملاحظة وصالحة للحوادث
   ([`security-hardening`](./security-hardening.md)،
   [`observability`](./observability.md)،
   [`incident-runbooks`](./incident-runbooks.md)).
5. نموذج التعليقات أو نموذج إعداد الإصدار (بما في ذلك الخطوط العريضة،
   أشرطة النسخ ولقطات الشاشة ومعلومات البيئة).
6. قم بنسخ الإعلان بمراجعة Docs/DevRel + Governance.

## باقة دعوة

تتضمن دعوة Chaque ما يلي:

1. **التحقق من المصنوعات اليدوية** - تقديم الامتيازات مقابل البيان/الخطة SoraFS أو المصنوعات اليدوية
   GitHub، بالإضافة إلى بيان المجموع الاختباري والواصف. المرجع صريح للأمر
   للتحقق من قدرة المراجعين على التنفيذ قبل بدء الموقع.
2. **تعليمات الخدمة** - قم بتضمين أمر معاينة البوابة حسب المجموع الاختباري:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **Rappels Securite** - تشير إلى أن الرموز المميزة تنتهي تلقائيًا، وأن الامتيازات
   ne doivent pas etrepartages، وما هي الأحداث التي تحدث على الفور.
4. **قناة التعليقات** - قم بوضع القالب/الصيغة وتوضيح أوقات الاستجابة.
5. **تواريخ البرنامج** - قم بتوفير تواريخ الظهور/الانتهاء، وساعات العمل أو المزامنة، وما إلى ذلك
   نافذة التحديث.

البريد الإلكتروني على سبيل المثال
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
تغطية هذه المتطلبات. Mettre a jour les العناصر النائبة (التواريخ وعناوين URL وجهات الاتصال)
قبل المبعوث.

## عرض المعاينة الساخنة

لا تقم بالترويج للمعاينة السريعة عند الموافقة على محطة الصعود وتذكرة التغيير.
Voir le [guide d'exposition de l'hote Preview](./preview-host-exposure.md) للمقاطع من النهاية إلى النهاية
قم بإنشاء/نشر/التحقق من المستخدمين في هذا القسم.

1. **الإنشاء والتعبئة:** قم بتحديد علامة الإصدار وإنتاج القطع الأثرية المحددة.

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

   النص البرمجي للدبوس المكتوب `portal.car`، `portal.manifest.*`، `portal.pin.proposal.json`،
   وآخرون `portal.dns-cutover.json` سو `artifacts/sorafs/`. انضم إلى هذه الملفات بطريقة غامضة
   تتيح لك الدعوة أن كل مراجع يمكنه التحقق من أجزاء الميمات.

2. **نشر معاينة الاسم المستعار:** إعادة توجيه الأوامر بدون `--skip-submit`
   (أربعة `TORII_URL`، `AUTHORITY`، `PRIVATE_KEY[_FILE]`، والإصدار المسبق من الاسم المستعار
   على قدم المساواة مع الحوكمة). البرنامج النصي يكذب `docs-preview.sora` ويصدر
   `portal.manifest.submit.summary.json` بالإضافة إلى `portal.pin.report.json` لحزمة Preuves.

3. **تحقق من النشر:** تأكد من ظهور الاسم المستعار وأن المجموع الاختباري يتوافق مع العلامة
   قبل إرسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Garder `npm run serve` (`scripts/serve-verified-preview.mjs`) هو الجزء الرئيسي كبديل احتياطي
   يمكن للمراجعين إطلاق نسخة محلية إذا كانت معاينة الحافة مفتوحة.

## الجدول الزمني للاتصالات

| جور | العمل | المالك |
| --- | --- | --- |
| د-3 | الانتهاء من نسخة الدعوة، وتنقية المصنوعات اليدوية، والتشغيل الجاف للتحقق | مستندات/ديفريل |
| د-2 | حوكمة التوقيع + تذكرة التغيير | Docs/DevRel + الحوكمة |
| د-1 | أرسل الدعوات عبر القالب، وقم بمتابعتها يوميًا مع قائمة الوجهات | مستندات/ديفريل |
| د | مكالمة البداية / ساعات العمل، مراقبة لوحات القياس عن بعد | Docs/DevRel + عند الطلب |
| د+7 | ملخص التعليقات والتعليقات الغامضة، فرز المشكلات المتراكمة | مستندات/ديفريل |
| د+14 | إزالة الغموض، واستعادة الوصول المؤقت، ونشر السيرة الذاتية في `status.md` | مستندات/ديفريل |

## متابعة الوصول والقياس عن بعد

1. قم بتسجيل كل الوجهة والطابع الزمني للدعوة وتاريخ الإلغاء مع
   معاينة مسجل الملاحظات (voir
   [`preview-feedback-log`](./preview-feedback-log)) للتمكن من مشاركة الميم بشكل غامض
   أثر preuves:

   ```bash
   # Ajouter un nouvel evenement d'invitation a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   الأحداث السعرية المدفوعة هي `invite-sent`، `acknowledged`،
   `feedback-submitted`، `issue-opened`، و`access-revoked`. تم العثور على السجل أ
   `artifacts/docs_portal_preview/feedback_log.json` الاسمية الافتراضية؛ joignez-le au Ticket de
   دعوة غامضة مع صيغ الموافقة. استخدم مساعد الملخص
   من أجل إنتاج مجموعة قابلة للتدقيق قبل ملاحظة الملابس:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   يقوم ملخص JSON بتعداد الدعوات المبهمة، والوجهات المفتوحة، والدعوات
   حاسبات التعليقات والطابع الزمني للحدث الأخير. L'helper repose sur
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)،
   يمكن أن يؤدي إنشاء سير العمل إلى تغيير موضعه أو في CI. استخدم قالب الهضم
   في [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   أثناء نشر خلاصة الغموض.
2. قم بوضع علامات على لوحات القياس عن بعد باستخدام `DOCS_RELEASE_TAG` للتوضيح الغامض
   الصور مؤثرة ومترابطة مع مجموعات الدعوة.
3. قم بتنفيذ `npm run probe:portal -- --expect-release=<tag>` بعد النشر لتأكيد ذلك
   تعلن معاينة البيئة عن إصدار البيانات الوصفية الجيدة.
4. قم بإرسال كل الحادث في قالب دليل التشغيل وإضافته إلى المجموعة.

## ردود الفعل والتخثر

1. قم بجمع التعليقات في مشاركة مستند أو في لوحة المشكلات. قم بتمييز العناصر مع
   `docs-preview/<wave>` حتى يتمكن مالكو خريطة الطريق من استرجاعها بسهولة.
2. استخدم ملخص تسجيل المعاينة لعرض التقرير الغامض، ثم
   استئناف المجموعة في `status.md` (المشاركين، الإحصاءات الأساسية، الإصلاحات السابقة) وما إلى ذلك
   Mettre a jour `roadmap.md` si le jalon DOCS-SORA تغيير.
3. متابعة خطوات الصعود مرة أخرى
   [`reviewer-onboarding`](./reviewer-onboarding.md): إلغاء الوصول وأرشفة الطلبات وغيرها
   إعادة شراء المشاركين.
4. قم بإعداد السلسلة الغامضة التالية لتنقيح القطع الأثرية، وربط بوابات المجموع الاختباري
   وانتظر طوال اليوم قالب الدعوة مع التواريخ الجديدة.

قم بتطبيق هذا الدليل الإرشادي بطريقة متسقة مع معاينة البرنامج القابلة للتدقيق وما إلى ذلك
Docs/DevRel طريقة متكررة لجعل الدعوات أكبر حتى يقترب الباب من GA.