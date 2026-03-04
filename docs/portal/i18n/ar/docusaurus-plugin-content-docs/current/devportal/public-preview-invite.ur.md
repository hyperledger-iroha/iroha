---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دعوة مسبقة الدفع

## پروگرام کے مقاصد

تم تفعيل هذه النسخة الجديدة من العمل الروائي في بورنغ بعد الإعلان عن الإصدار الأول من الفيلم وكلايا جاي.
يتيح لك DOCS-SORA دعوة إلى إنشاء عناصر فنية وأبحاث علمية قابلة للتتبع.
ردود الفعل لدينا واضحة تشمل هذه الفكرة.

- **الأحداث:** أعضاء بارزون وشركاء ومشرفون منسقون في الشهر الأول من العام لا يقومون بمعاينة الاستخدام المقبول في باليس ساينت.
- **المحتوى:** حجم الموجة الافتراضي <= 25 ريوورز، 14 دن نافذة وصول، واستجابة للحوادث على مدار 24 ساعة.

## لانچ گیٹ چیک لٹ

هناك أيضًا دعوة متجددة أو مكتملة:

1. أحدث معاينة لعناصر CI التي تم تنزيلها (`docs-portal-preview`,
   بيان المجموع الاختباري، الواصف، حزمة SoraFS)۔
2.`npm run --prefix docs/portal serve` (بوابة المجموع الاختباري) هي العلامة الموجودة على القائمة.
3. قم بالموافقة على الاشتراك ودعوة موجة من الإعجاب.
4. المراقبة والملاحظة والتحقق من صحة الحادث
   ([`security-hardening`](./security-hardening.md)،
   [`observability`](./observability.md)،
   [`incident-runbooks`](./incident-runbooks.md)).
5. قالب التعليقات أو المزرعة أو المشكلة (الخطورة، وخطوات إعادة الإنتاج، ولقطات الشاشة، ومعلومات البيئة تتضمن كل ذلك).
6. الإعلان عن Docs/DevRel + Governance جديد تمامًا.

## دعوةی پیکیج

دعوة تشمل ما يلي:

1. **العناصر التي تم التحقق منها** — بيان/خطة SoraFS أو قطعة أثرية لـ GitHub لن تكون كذلك،
   يوجد أيضًا بيان المجموع الاختباري والواصف. أصبح التحقق واضحًا لهذه الغاية
   موقع ريوورز لانچ کرنے سے پہلے هو چلا سکیں.
2. **تعليمات التقديم** — تتضمن ميزة المعاينة ذات المجموع الاختباري:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **تذكيرات الأمان** — تنتهي صلاحية الرموز المميزة الخاصة بك بشكل واضح، ولن تكرر أي مشكلة،
   والحوادث يتم الإبلاغ عنها على الفور.
4. **قناة التعليقات** — نموذج/نموذج المشكلة وتوقعات وقت الاستجابة بشكل واضح.
5. **تواريخ البرنامج** — تواريخ البدء/الانتهاء، وساعات العمل أو المزامنة، ونافذة التحديث الأخيرة.

نمو مليون
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
هناك الأجهزة والمتطلبات الخاصة بها. العناصر النائبة (التواريخ وعناوين URL وجهات الاتصال)
كل شيء على ما يرام.

## مضيف أولي يقوم بكشف الكريں

عندما لا يتم إجراء عملية الإعداد وتغيير التذكرة، لا يتم معاينة مضيف المعاينة الذي لا يقوم بالترويج.
إنشاء/نشر/التحقق من الخطوات الشاملة
[دليل تعرض المضيف للمعاينة](./preview-host-exposure.md) د.

1. **البناء والتصوير:** حرر ختم العلامة والتحف الحتمية.

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

   الدبوس النصي `portal.car`، `portal.manifest.*`، `portal.pin.proposal.json`،
   و `portal.dns-cutover.json` و `artifacts/sorafs/` لا يزالان كذلك. ان تنتهي موجة الدعوة
   قم بإرفاق علامة التبويب فقط وتحقق من البتات.

2. **معاينة الاسم المستعار للنشر:** سجل `--skip-submit` لإعادة النشر مرة أخرى
   (`TORII_URL`، `AUTHORITY`، `PRIVATE_KEY[_FILE]` وإثبات الاسم المستعار الصادر عن الحوكمة).
   السكربت `docs-preview.sora` ملف ربط البيان وحزمة الأدلة
   `portal.manifest.submit.summary.json` و `portal.pin.report.json` .

3. **مسبار النشر:** يدعو جميع الأسماء المستعارة لحل هاونا والمجموع الاختباري لعلامة تطابق هاونا
   هذا يعني.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   `npm run serve` (`scripts/serve-verified-preview.mjs`) وهو خيار احتياطي سهل الاستخدام
   إذا كانت معاينة الحافة ستكون مهمة، فستحتاج إلى تحديثات أقل من أي شيء آخر.

## كوميونيكيشن ٹائم عبر الإنترنت

| دن | ایکشن | المالك |
| --- | --- | --- |
| د-3 | دعوةی کاپی إنهاء کرنا، تحديث القطع الأثرية کرنا، التحقق من التشغيل الجاف | مستندات/ديفريل |
| د-2 | توقيع الحوكمة + تذكرة التغيير | Docs/DevRel + الحوكمة |
| د-1 | قالب إعلان دعوة سريع، متعقب لقائمة المستلمين في كل مرة | مستندات/ديفريل |
| د | مكالمة البداية / ساعات العمل، لوحات معلومات القياس عن بعد | Docs/DevRel + عند الطلب |
| د+7 | ملخص ردود الفعل عند نقطة المنتصف، حظر المشكلات، الفرز | مستندات/ديفريل |
| د+14 | wave Bend كریں، عارض رسائی إبطال كریں، `status.md` خلاصہ شائعة کریں | مستندات/ديفريل |

## تتبع الوصول والقياس عن بعد

1. المستلم، الطابع الزمني للدعوة، وتاريخ الإلغاء لمعاينة مسجل الملاحظات وتسجيل الملاحظات
   (ديوم [`preview-feedback-log`](./preview-feedback-log)) هذه الموجة هي دليل أفضل:

   ```bash
   # artifacts/docs_portal_preview/feedback_log.json میں نیا invite event شامل کریں
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   الأحداث المدعومة: `invite-sent`، `acknowledged`، `feedback-submitted`،
   `issue-opened`، و`access-revoked`. سجل ڈیفالٹ پر
   `artifacts/docs_portal_preview/feedback_log.json` موجود؛ إنها موجة دعوة ٹکٹ ے ساتھ
   نماذج الموافقة سميت إرفاق. ملاحظة إغلاق ملخص استخدام مساعد الاستخدام
   قائمة تجميعية قابلة للتدقيق:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   ملخص JSON عبارة عن موجة من الدعوات، وعدد من المستلمين، وعدد التعليقات، والحدث الثالث
   الطابع الزمني کو تعداد کرتا ہے۔ مساعد
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)
   في هذا الصدد، يتم توفير كل ما تحتاجه لسير العمل أو CI. خلاصة شاع کرتے الوقت
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   واستخدام قالب الملخص کریں۔
2. لوحات معلومات القياس عن بعد التي تستخدم الموجة `DOCS_RELEASE_TAG` تستخدم علامة التسجيل
   المسامير ودعوة الأفواج ترتبط ببعضها البعض.
3. نشر بيئة المعاينة بعد `npm run probe:portal -- --expect-release=<tag>`
   حق إطلاق البيانات الوصفية الإعلان عن کرے۔
4. يمكن لأي حادث في قالب دليل التشغيل التقاط الحركات وربط المجموعة النموذجية به.

## ردود الفعل والإغلاق

1. يتم جمع التعليقات على المستند المشترك أو لوحة المشكلات. العناصر التي `docs-preview/<wave>` لها علامة تجارية
   يمكن لأصحاب خارطة الطريق الاستعلام بسهولة عنهم.
2. قم بمعاينة المسجل لمخرجات ملخص تقرير الموجة الثانية، ثم قم بتلخيص المجموعة النموذجية `status.md`
   (المشاركين، النتائج، الإصلاحات المخططة) وإذا تم إنشاء DOCS-SORA بدلاً من `roadmap.md`.
3. [`reviewer-onboarding`](./reviewer-onboarding.md) اتبع خطوات إلغاء التشغيل: إلغاء الوصول،
   طلبات أرشفة الملفات والمشاركين
4. قم بتحديث اللعبة بعد موجة من القطع الأثرية، وبوابات المجموع الاختباري مرة أخرى، وقم بدعوة قالب للتواريخ الجديدة في كل مرة.

إنه دليل التشغيل الخاص بمسلسل لاغو وهو عبارة عن دعوة لمعاينة البرامج القابلة للتدقيق وDocs/DevRel
مهارة مهارة قابلة للتكرار عبر الإنترنت وإصدار من GA Portal هو قريب آتا.