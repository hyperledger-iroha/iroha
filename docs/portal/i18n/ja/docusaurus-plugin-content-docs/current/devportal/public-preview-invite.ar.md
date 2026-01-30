---
lang: ja
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# دليل دعوات المعاينة العامة

## اهداف البرنامج

يوضح هذا الدليل كيفية اعلان وتشغيل المعاينة العامة بعد تفعيل سير عمل تاهيل المراجعين. يحافظ على
صدق خارطة الطريق DOCS-SORA عبر ضمان ان كل دعوة تتضمن اثارا قابلة للتحقق، ارشادات امنية، ومسارا
واضحا للتغذية الراجعة.

- **الجمهور:** قائمة منقحة من اعضاء المجتمع والشركاء والـ maintainers الذين وقعوا سياسة الاستخدام المقبول للمعاينة.
- **الحدود:** حجم الموجة الافتراضي <= 25 مراجع، نافذة وصول 14 يوما، استجابة للحوادث خلال 24h.

## قائمة فحص بوابة الاطلاق

اكمل هذه المهام قبل ارسال اي دعوة:

1. اخر اثار المعاينة مرفوعة في CI (`docs-portal-preview`,
   manifest checksum, descriptor, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (مقيد بالchecksum) تم اختباره على نفس tag.
3. تذاكر تاهيل المراجعين معتمدة ومربوطة بموجة الدعوة.
4. مستندات الامن والمراقبة والحوادث مؤكدة
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. نموذج feedback او قالب issue مجهز (يتضمن حقول الشدة، خطوات اعادة الانتاج، لقطات شاشة، ومعلومات البيئة).
6. نسخة الاعلان تمت مراجعتها من Docs/DevRel + Governance.

## حزمة الدعوة

يجب ان تتضمن كل دعوة:

1. **اثار متحقق منها** — قدم روابط manifest/plan لـ SoraFS او artefact من GitHub
   اضافة الى manifest checksum وdescriptor. اذكر امر التحقق صراحة حتى يتمكن المراجعون
   من تشغيله قبل تشغيل الموقع.
2. **تعليمات التشغيل** — ادرج امر المعاينة المقيد بالchecksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **تذكيرات امنية** — اوضح ان الرموز تنتهي تلقائيا، والروابط لا يجب مشاركتها،
   ويجب الابلاغ عن الحوادث فورا.
4. **قناة feedback** — اربط نموذج/قالب issue ووضح توقعات زمن الاستجابة.
5. **تواريخ البرنامج** — قدم تواريخ البداية/النهاية، ساعات المكتب او جلسات sync،
   ونافذة التحديث التالية.

البريد النموذجي في
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
يغطي هذه المتطلبات. حدّث العناصر النائبة (التواريخ، URLs، وجهات الاتصال)
قبل الارسال.

## اظهار مضيف المعاينة

لا تروج لمضيف المعاينة الا بعد اكتمال التاهيل واعتماد تذكرة التغيير. راجع
[دليل اظهار مضيف المعاينة](./preview-host-exposure.md) لخطوات build/publish/verify من البداية للنهاية
المستخدمة في هذا القسم.

1. **البناء والتعبئة:** ضع release tag وانتج اثارا حتمية.

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

   سكربت pin يكتب `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   و`portal.dns-cutover.json` تحت `artifacts/sorafs/`. ارفق هذه الملفات بموجة الدعوة
   حتى يتمكن كل مراجع من التحقق من نفس البتات.

2. **نشر alias المعاينة:** اعد تشغيل الامر بدون `--skip-submit`
   (قدم `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`، واثبات alias الصادر عن الحوكمة).
   سيقوم السكربت بربط manifest بـ `docs-preview.sora` ويصدر
   `portal.manifest.submit.summary.json` و`portal.pin.report.json` لحزمة الادلة.

3. **فحص النشر:** تاكد من ان alias يحل وانه checksum يطابق tag قبل ارسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   احتفظ بـ `npm run serve` (`scripts/serve-verified-preview.mjs`) كخيار fallback ليتمكن
   المراجعون من تشغيل نسخة محلية اذا حدث خلل في preview edge.

## جدول الاتصالات

| اليوم | الاجراء | Owner |
| --- | --- | --- |
| D-3 | انهاء نص الدعوة، تحديث الاثار، تنفيذ dry-run للتحقق | Docs/DevRel |
| D-2 | موافقة الحوكمة + تذكرة تغيير | Docs/DevRel + Governance |
| D-1 | ارسال الدعوات باستخدام القالب، تحديث tracker بقائمة المستلمين | Docs/DevRel |
| D | مكالمة kickoff / office hours، مراقبة لوحات القياس | Docs/DevRel + On-call |
| D+7 | digest للfeedback في منتصف الموجة، triage للقضايا المانعة | Docs/DevRel |
| D+14 | اغلاق الموجة، الغاء الوصول المؤقت، نشر ملخص في `status.md` | Docs/DevRel |

## تتبع الوصول والقياس عن بعد

1. سجل كل مستلم وطابع وقت الدعوة وتاريخ الالغاء باستخدام preview feedback logger
   (انظر [`preview-feedback-log`](./preview-feedback-log)) حتى تشترك كل موجة
   في نفس اثر الادلة:

   ```bash
   # اضافة حدث دعوة جديد الى artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   الاحداث المدعومة هي `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, و`access-revoked`. يعيش السجل افتراضيا في
   `artifacts/docs_portal_preview/feedback_log.json`; ارفقه بتذكرة موجة الدعوة
   مع نماذج الموافقة. استخدم مساعد summary لانتاج roll-up قابل للتدقيق قبل
   ملاحظة الاغلاق:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   summary JSON يعد الدعوات لكل موجة، المستلمين المفتوحين، تعداد feedback، وطابع
   الوقت لاخر حدث. المساعد يعتمد على
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   لذا يمكن تشغيل نفس workflow محليا او في CI. استخدم قالب digest في
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   عند نشر recap للموجة.
2. ضع وسم `DOCS_RELEASE_TAG` المستخدم للموجة على لوحات القياس حتى يمكن ربط
   الارتفاعات مع cohort الدعوات.
3. شغل `npm run probe:portal -- --expect-release=<tag>` بعد النشر لتاكيد
   ان بيئة المعاينة تعلن عن metadata الاصدار الصحيحة.
4. سجل اي حادثة في قالب runbook واربطها بالمجموعة.

## feedback والاغلاق

1. اجمع feedback في مستند مشترك او لوحة issues. ضع وسم `docs-preview/<wave>` حتى يتمكن
   اصحاب roadmap من العثور عليها بسهولة.
2. استخدم مخرجات summary من preview logger لملء تقرير الموجة، ثم لخص المجموعة في
   `status.md` (المشاركون، ابرز الملاحظات، الاصلاحات المخطط لها) وحدّث `roadmap.md`
   اذا تغير معلم DOCS-SORA.
3. اتبع خطوات offboarding من
   [`reviewer-onboarding`](./reviewer-onboarding.md): الغ الوصول، ارشف الطلبات، واشكر المشاركين.
4. جهز الموجة التالية عبر تحديث الاثار، اعادة تشغيل gates الخاصة بالchecksum،
   وتحديث قالب الدعوة بتواريخ جديدة.

تطبيق هذا الدليل بشكل متسق يحافظ على قابلية تدقيق برنامج المعاينة ويمنح Docs/DevRel
طريقة قابلة للتكرار لتوسيع الدعوات مع اقتراب البوابة من GA.
