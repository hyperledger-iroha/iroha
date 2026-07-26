---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/public-preview-invite.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Playbook de convites يقوم بمعاينة الجمهور

## أهداف البرنامج

يتم شرح هذا قواعد اللعبة كإعلان وتنفيذ أو معاينة عامة كما هو الحال
سير العمل في إعداد المراجعين النشطين. اتبع خريطة الطريق DOCS-SORA الصادقة
نضمن أن كل دعوة تأتي مع التحف التي تم التحقق منها، وتوجيه الأمان والحماية
كامينيو كلارو دي ردود الفعل.

- **الجمهور:** قائمة بأعضاء المجتمع والشركاء والمشرفين الذين ينضمون إليهم
  سياسة الاستخدام العادل للمعاينة.
- **الحدود:** عدد الزيارات <= 25 مراجعة، الوصول خلال 14 يومًا، الرد
  حوادث م 24 ساعة.

## قائمة التحقق من بوابة الإنطلاق

أكمل هذه المهام قبل إرسال أي دعوة:

1. آخر معاينة فنية مرسلة إلى CI (`docs-portal-preview`,
   بيان المجموع الاختباري، الواصف، الحزمة SoraFS).
2. `npm run --prefix docs/portal serve` (بوابة المجموع الاختباري) تم اختباره بدون علامة mesmo.
3. تذاكر تأهيل المراجعات المعتمدة والموافقة على المكالمات.
4. مستندات الأمان وإمكانية الملاحظة والأحداث التي تم التحقق منها
   ([`security-hardening`](./security-hardening.md)،
   [`observability`](./observability.md)،
   [`incident-runbooks`](./incident-runbooks.md)).
5. صيغة الملاحظات أو نموذج الإصدار المُعد (بما في ذلك مجالات الخصوصية،
   مقاطع إعادة الإنتاج ولقطات الشاشة والمعلومات المحيطة).
6. انسخ الإعلان المنقح بواسطة Docs/DevRel + Governance.

##باكوت دي كونفيت

تتضمن كل دعوة ما يلي:

1. **Artefatos verificados** - الروابط Forneca للبيان/الخطة SoraFS أو للArtefatos
   GitHub، بالإضافة إلى بيان المجموع الاختباري والواصف. المرجع بوضوح إلى الأمر
   للتحقق من إمكانية تنفيذ المراجعات قبل فتح الموقع.
2. **تعليمات الخدمة** - تتضمن أمر المعاينة الموجه من خلال المجموع الاختباري:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **رموز الأمان** - لإعلامك بانتهاء صلاحية الرموز المميزة تلقائيًا، لن يتم تفعيل الروابط
   يجب أن يتم الإبلاغ عن الأحداث والأحداث على الفور.
4. **قناة التعليقات** - قم بربط القالب/النموذج وتوضيح التوقعات في وقت الاستجابة.
5. **بيانات البرنامج** - إبلاغ بيانات البداية/الفيلم وساعات العمل والمزامنة والتقريبية
   janela defresh.

البريد الإلكتروني للمثال
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
متطلبات الكوبري. تحقيق العناصر النائبة لنظام التشغيل (البيانات وعناوين URL وجهات الاتصال)
قبل دي Enviar.

## تصدير مضيف المعاينة

لذلك قم بتعزيز مضيف المعاينة عند اكتمال عملية الإعداد وتذكرة التغيير
com.aprovado. شاهد [دليل العرض الخاص بمضيف المعاينة](./preview-host-exposure.md) لكلمات مرورك
من النهاية إلى النهاية للإنشاء/النشر/التحقق من المستخدمين هنا.

1. **البناء والتعبئة:** قم بتمييز علامة الإصدار وإنتاج العناصر المحددة.

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

   يا البرنامج النصي للدبوس Grava `portal.car`، `portal.manifest.*`، `portal.pin.proposal.json`،
   و`portal.dns-cutover.json` في `artifacts/sorafs/`. قم بإضافة هذه الملفات إلى قائمة المكالمات
   حتى يتمكن كل مراجع من التحقق من هذه البتات.

2. **نشر الاسم المستعار للمعاينة:** ركب الأمر Sem `--skip-submit`
   (forneca `TORII_URL`، `AUTHORITY`، `PRIVATE_KEY[_FILE]`، وإثبات الاسم المستعار المنبعث
   بيلا جوفرانكا). البرنامج النصي أو إنشاء ملف `docs-preview.sora` وإصداره
   `portal.manifest.submit.summary.json` المزيد من `portal.pin.report.json` لحزمة الأدلة.

3. **اختبار النشر:** تأكد من حل الاسم المستعار وأن المجموع الاختباري يتوافق مع العلامة
   أنتيس دي Enviar convites.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Mantenha `npm run serve` (`scripts/serve-verified-preview.mjs`) وماو كبديل احتياطي
   يمكن للمراجعين إرسال نسخة محلية عند حافة المعاينة.

## الجدول الزمني للاتصالات

| ضياء | أكاو | المالك |
| --- | --- | --- |
| د-3 | النسخة النهائية للدعوة، وتحديث القطع الأثرية، والتشغيل الجاف للتحقق | مستندات/ديفريل |
| د-2 | تسجيل الخروج من الحكم + تذكرة مودانكا | Docs/DevRel + الحوكمة |
| د-1 | أرسل الدعوة إلى استخدام القالب، وتحديث المتتبع مع قائمة الوجهات | مستندات/ديفريل |
| د | مكالمة البداية / ساعات العمل، لوحات مراقبة القياس عن بعد | Docs/DevRel + عند الطلب |
| د+7 | ملخص التعليقات ليس أفضل من أي وقت مضى، فرز المشكلات التراكمية | مستندات/ديفريل |
| د+14 | قم بالبدء مرة أخرى، واستعادة الوصول المؤقت، ونشر السيرة الذاتية في `status.md` | مستندات/ديفريل |

## تتبع الوصول والقياس عن بعد

1. قم بتسجيل كل وجهة، والطابع الزمني للدعوة، وبيانات التجديد مع o
   معاينة مسجل الملاحظات (veja
   [`preview-feedback-log`](./preview-feedback-log)) حتى تتمكن من المشاركة في نفس الوقت
   معلومات عن الأدلة:

   ```bash
   # Adicione um novo evento de convite a artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```

   الأحداث المدعومة sao `invite-sent`، `acknowledged`،
   `feedback-submitted`، `issue-opened`، و`access-revoked`. يا سجل فيكا م
   `artifacts/docs_portal_preview/feedback_log.json` بواسطة بادراف؛ ملحق ao تذكرة da
   Onda de convites juto com os صيغ الموافقة. استخدام يا مساعد دي
   ملخص لإنتاج مراجعة إجمالية قبل ملاحظة التحسين:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   ملخص JSON enumera convites por onda، destinatarios abertos، contagens de
   ردود الفعل والطابع الزمني للحدث الأحدث. يا مساعد ه apoiado بور
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs)،
   يمكن نقل أو سير العمل نفسه إلى مكانه أو في CI. استخدم قالب الهضم
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   أو نشر أو تلخيص أوندا.
2. قم بوضع لوحات معلومات القياس عن بعد باستخدام `DOCS_RELEASE_TAG` لتتمكن من ذلك
   يمكن أن تكون الصور مرتبطة بحفلات الدعوة.
3. ركب `npm run probe:portal -- --expect-release=<tag>` للنشر لتأكيد ذلك
   تعلن بيئة المعاينة عن بيانات التعريف الصحيحة للإصدار.
4. قم بالتسجيل في أي حادثة بدون قالب Runbook أو قم بالتسجيل في Corte.

## ردود الفعل والملاحظات

1. قم بجمع التعليقات في مستند لمشاركته أو في قائمة المشكلات. ماركو أو إس إيتنس كوم
   `docs-preview/<wave>` لكي يتمكن المالكون من الوصول إلى خريطة الطريق بسهولة.
2. استخدم ملخصًا لمعاينة المسجل لمعاينة أو الارتباط من هناك، بعد ذلك
   استئناف الحضور في `status.md` (المشاركين، المبتدئين، إصلاحات التخطيط) e
   قم بتفعيل `roadmap.md` كما هو موضح في DOCS-SORA.
3. احصل على تصاريح الخروج من الطائرة
   [`reviewer-onboarding`](./reviewer-onboarding.md): تجديد الوصول وطلبات التقديم
   agradeca os المشاركين.
4. قم بإعداد تقريبي لتحديث العناصر وإعادة تنفيذ بوابات المجموع الاختباري
   تحديث قالب الدعوة مع بيانات جديدة.

تطبيق قواعد اللعبة هذه بشكل متسق مع الحفاظ على برنامج المعاينة القابل للتدقيق
نقدم إلى Docs/DevRel طريقًا متكررًا لتصعيد الدعوة عبر البوابة
يقع بالقرب من GA.