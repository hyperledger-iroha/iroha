---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل الدعوات للمعاينة العامة

## أهداف البرنامج

يتم شرح هذا قواعد اللعبة كإعلان وتنفيذ المعاينة العامة مرة واحدة
هذا هو سير عمل تأهيل المراجعين النشط. حافظ على خارطة الطريق الصادقة DOCS-SORA al
تأكد من أن كل دعوة سيتم إرسالها باستخدام عناصر يمكن التحقق منها، ودليل أمان وآخر
طريق كلارو دي ردود الفعل.

- **الجمهور:** قائمة بأعضاء المجتمع والشركاء والمشرفين الذين
  ثبت سياسة الاستخدام المقبول للمعاينة.
- **الحدود:** حجم العيب <= 25 مراجعة، نافذة الوصول 14 يومًا، الرد
  حوادث على مدار 24 ساعة.

## قائمة التحقق من بوابة الانزامينتو

أكمل هذه المهام قبل إرسال أي دعوة:

1. آخر معاينة الشحنات الفنية في CI (`docs-portal-preview`,
   بيان المجموع الاختباري، الواصف، الحزمة SoraFS).
2.`npm run --prefix docs/portal serve` (بوابة للمجموع الاختباري) تم اختباره بنفس العلامة.
3. تذاكر تأهيل المراجعات المناسبة والمُسرَّعة على أساس الدعوات.
4. مستندات الأمان والمراقبة والأحداث التي تم التحقق منها
   ([`security-hardening`](./security-hardening.md)،
   [`observability`](./observability.md)،
   [`incident-runbooks`](./incident-runbooks.md)).
5. صيغة التعليقات أو مجموعة المشكلات المعدة (بما في ذلك مجالات القوة،
   خطوات الإنتاج ولقطات الشاشة ومعلومات الإدخال).
6. نص الإعلان المنقح بواسطة Docs/DevRel + Governance.

##باكيت دي إنفيتاسيون

تتضمن كل دعوة ما يلي:

1. **المنتجات المصطنعة التي تم التحقق منها** - تتضمن المعلومات البيان/الخطة SoraFS o a los
   تمثل المصنوعات اليدوية لـ GitHub بيان المجموع الاختباري والواصف. مرجع القائد
   التحقق بشكل صريح حتى تتمكن المراجعات من تنفيذها قبل المشرق
   الموقع.
2. **تعليمات الخدمة** - تتضمن أمر المعاينة الموجه من خلال المجموع الاختباري:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **مسجلات الأمان** - تشير إلى أن الرموز تنتهي صلاحيتها تلقائيًا، والروابط
   لا ينبغي مشاركة الأحداث ويجب الإبلاغ عنها بشكل فوري.
4. **قناة ردود الفعل** - قم بإعداد النبات/الصيغة وإظهار التوقعات المتوقعة لوقت الاستجابة.
5. **مذكرات البرنامج** - تقديم مذكرات البداية/النهاية، وساعات العمل أو المزامنة، وبالقرب من ذلك
   نافذة التحديث.

El email de muestra en
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
مكعب هذه المتطلبات. تحديث العناصر النائبة (fechas، وعناوين URL، وجهات الاتصال)
قبل دي Enviar.

## كشف مضيف المعاينة

يتم الترويج لمضيف المعاينة فقط بمجرد اكتمال عملية الإعداد وتذكرة التغيير
هذا هو aprobado. راجع [دليل عرض مضيف المعاينة](./preview-host-exposure.md)
للخطوات الشاملة للإنشاء/النشر/التحقق من الاستخدام في هذا القسم.

1. ** البناء والتعبئة: ** قم بتمييز علامة الإصدار وإنتاج القطع الأثرية المحددة.

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

   البرنامج النصي للكتابة `portal.car`، `portal.manifest.*`، `portal.pin.proposal.json`،
   ذ `portal.dns-cutover.json` باجو `artifacts/sorafs/`. أضف هذه الملفات إلى ola de
   دعوات لكي يتمكن كل مراجع من التحقق من نفس البتات.

2. **نشر الاسم المستعار للمعاينة:** كرر الأمر بدون `--skip-submit`
   (نسبة `TORII_URL`، `AUTHORITY`، `PRIVATE_KEY[_FILE]`، واختبار الاسم المستعار الصادر
   من أجل الحاكم). يتم طباعة البرنامج النصي على البيان `docs-preview.sora` ويتم إصداره
   `portal.manifest.submit.summary.json` و`portal.pin.report.json` لحزمة الأدلة.

3. **اختبار الإرسال:** تأكد من ظهور الاسم المستعار وأن المجموع الاختباري يتزامن مع العلامة
   قبل إرسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   صيانة `npm run serve` (`scripts/serve-verified-preview.mjs`) كأداة احتياطية
   يمكن للمراجعين توفير نسخة محلية في حالة فشل حافة المعاينة.

## الجدول الزمني للاتصالات

| ضياء | أكسيون | المالك |
| --- | --- | --- |
| د-3 | الانتهاء من نسخة الدعوة، تجديد المصنوعات اليدوية، التحقق الجاف | مستندات/ديفريل |
| د-2 | تسجيل الخروج من الحكومة + تذكرة التغيير | Docs/DevRel + الحوكمة |
| د-1 | إرسال الدعوات باستخدام النبات، وتحديث المتتبع بقائمة الوجهات | مستندات/ديفريل |
| د | مكالمة البداية / ساعات العمل، لوحات أجهزة القياس عن بعد | Docs/DevRel + عند الطلب |
| د+7 | ملخص التعليقات على آخر مرة، فرز المشكلات التراكمية | مستندات/ديفريل |
| د+14 | قم بالرجوع الآن، وإلغاء الوصول المؤقت، ونشر السيرة الذاتية في `status.md` | مستندات/ديفريل |

## متابعة الوصول والقياس عن بعد

1. قم بالتسجيل لكل وجهة، والطابع الزمني للدعوة، وإشعار الإلغاء مع
   معاينة مسجل ردود الفعل (الإصدار
   [`preview-feedback-log`](./preview-feedback-log)) لكل من يقوم بمقارنة نفس الشيء
   تحليل الأدلة:

   ```bash
   # Agrega un nuevo evento de invitacion a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   الأحداث المناسبة هي `invite-sent`، `acknowledged`،
   `feedback-submitted`، `issue-opened`، و`access-revoked`. إل سجل الحياة أون
   `artifacts/docs_portal_preview/feedback_log.json` بسبب الخلل؛ adjuntalo al Ticket de
   La ola de invitaciones junto con los formarios de convisimento. الولايات المتحدة الأمريكية المساعد
   ملخص لإنتاج سيرة ذاتية قابلة للتدقيق قبل ملاحظة السلك:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   ملخص JSON يسرد الدعوات من أجل ola، destinatarios aiertos، conteos de
   التعليقات والطابع الزمني للحدث الأحدث. El helper is respaldado por
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)،
   كما يمكن لسير العمل نفسه تصحيحه محليًا أو في CI. يستخدم نبات الهضم أون
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   لنشر خلاصة العلا.
2. يتم استخدام لوحة معلومات لوحات القياس عن بعد مع `DOCS_RELEASE_TAG` من أجلك.
   يمكن أن ترتبط الصور بمجموعات الدعوة.
3. قم بتشغيل `npm run probe:portal -- --expect-release=<tag>` بعد النشر للتأكيد
   تعلن عملية المعاينة عن إصدار البيانات الوصفية الصحيحة.
4. قم بتسجيل أي حادث في مجموعة دليل التشغيل وقم بتوزيعه على المجموعة.

## ردود الفعل والمتابعة

1. قم بجمع التعليقات في مستند مشترك أو جدول المشكلات. عناصر الاتيكيت مع
   `docs-preview/<wave>` حتى يتمكن أصحاب خريطة الطريق من استشارة بسهولة.
2. استخدم ملخص معاينة المسجل لعرض تقرير العلا واستئنافه
   la cohorte en `status.md` (المشاركين، النقاط الرئيسية، الإصلاحات المسطحة) و
   تم تحديث `roadmap.md` مع تغيير DOCS-SORA.
3. اتبع خطوات النقل
   [`reviewer-onboarding`](./reviewer-onboarding.md): إلغاء الوصول وطلبات الأرشيف و
   الحصول على المشاركين.
4. قم بإعداد العناصر التالية لتجديد العناصر، وإعادة تشغيل بوابات المجموع الاختباري،
   تحديث لوحة الدعوة برسومات جديدة.

قم بتطبيق هذا الدليل بشكل متسق للحفاظ على برنامج المعاينة القابل للتدقيق
قم باستخدام Docs/DevRel بطريقة متكررة لرفع الدعوات عبر البوابة
تابع إلى GA.