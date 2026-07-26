---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/public-preview-invite.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# دليل دعوات المعاينة العامة

## اهداف البرنامج

يوضح هذا الدليل كيفية اعلان وتشغيل المعاينة العامة بعد تفعيل سير عمل تاهيل المراجعين. يحافظ على
صدق خارطة الطريق DOCS-SORA عبر ضمان ان كل دعوة تتضمن اثارا قابلة للتحقق، ارشادات امنية، ومسارا
واضحا للتغذية الراجعة.

- **الجمهور:** قائمة منقحة من اعضاء المجتمع والشركاء والـ mainteneurs الذين وقعوا سياسة الاستخدام المقبول للمعاينة.
- **الحدود:** حجم الموجة الافتراضي <= 25 مراجع، نافذة وصول 14 يوما، استجابة للحوادث خلال 24h.

## قائمة فحص بوابة الاطلاق

اكمل هذه المهام قبل ارسال اي دعوة:

1. اخر اثار المعاينة مرفوعة في CI (`docs-portal-preview`,
   somme de contrôle du manifeste, descripteur, bundle SoraFS).
2. `npm run --prefix docs/portal serve` (avec somme de contrôle) est une balise.
3. تذاكر تاهيل المراجعين معتمدة ومربوطة بموجة الدعوة.
4. مستندات الامن والمراقبة والحوادث مؤكدة
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. نموذج feedback او قالب issue مجهز (يتضمن حقول الشدة، خطوات اعادة الانتاج، لقطات شاشة، ومعلومات البيئة).
6. نسخة الاعلان تمت مراجعتها من Docs/DevRel + Governance.

## حزمة الدعوة

يجب ان تتضمن كل دعوة:1. **اثار متحقق منها** — Il s'agit d'un manifeste/plan pour SoraFS et d'un artefact sur GitHub
   Il s'agit d'un descripteur de somme de contrôle manifeste. اذكر امر التحقق صراحة حتى يتمكن المراجعون
   من تشغيله قبل تشغيل الموقع.
2. **تعليمات التشغيل** — ادرج امر المعاينة المقيد بالchecksum :

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
يغطي هذه المتطلبات. حدّث العناصر النائبة (URL de téléchargement et URL)
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

   Broches pour `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   و`portal.dns-cutover.json` contre `artifacts/sorafs/`. ارفق هذه الملفات بموجة الدعوة
   حتى يتمكن كل مراجع من التحقق من نفس البتات.2. **نشر alias المعاينة:** اعد تشغيل الامر بدون `--skip-submit`
   (`TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, alias الصادر عن الحوكمة).
   سيقوم السكربت بربط manifeste بـ `docs-preview.sora` ويصدر
   `portal.manifest.submit.summary.json` et `portal.pin.report.json` pour la lecture.

3. **فحص النشر:** تاكد من ان alias يحل وانه checksum يطابق tag قبل ارسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   Utiliser `npm run serve` (`scripts/serve-verified-preview.mjs`) pour la solution de secours
   Il s'agit d'un aperçu du bord d'aperçu.

## جدول الاتصالات

| اليوم | الاجراء | Propriétaire |
| --- | --- | --- |
| J-3 | Fonctionnement à sec | Docs/DevRel |
| J-2 | موافقة الحوكمة + تذكرة تغيير | Docs/DevRel + Gouvernance |
| J-1 | ارسال الدعوات باستخدام القالب، تحديث tracker بقائمة المستلمين | Docs/DevRel |
| D | Coup d'envoi / heures de bureau, مراقبة لوحات القياس | Docs/DevRel + Sur appel |
| J+7 | digérer les commentaires sur le triage | Docs/DevRel |
| J+14 | اغلاق الموجة، الغاء الوصول المؤقت، نشر ملخص في `status.md` | Docs/DevRel |

## تتبع الوصول والقياس عن بعد

1. Comment utiliser l'enregistreur de commentaires d'aperçu
   (انظر [`preview-feedback-log`](./preview-feedback-log)) حتى تشترك كل موجة
   في نفس اثر الادلة:

   ```bash
   # اضافة حدث دعوة جديد الى artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```Liens vers `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, و`access-revoked`. يعيش السجل افتراضيا في
   `artifacts/docs_portal_preview/feedback_log.json` ; ارفقه بتذكرة موجة الدعوة
   مع نماذج الموافقة. استخدم مساعد résumé لانتاج roll-up قابل للتدقيق قبل
   ملاحظة الاغلاق:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   résumé JSON contient des commentaires et des commentaires
   الوقت لاخر حدث. المساعد يعتمد على
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Il s'agit d'un flux de travail similaire à CI. استخدم قالب digest في
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   Je vais récapituler ici.
2. Utilisez le `DOCS_RELEASE_TAG` pour la connexion avec la ligne de commande.
   الارتفاعات مع cohort الدعوات.
3. Utiliser `npm run probe:portal -- --expect-release=<tag>` pour la connexion
   Il s'agit de métadonnées de base.
4. Connectez-vous au runbook et au runbook.

## feedback

1. Commentaires sur les problèmes liés aux problèmes liés aux problèmes. ضع وسم `docs-preview/<wave>` حتى يتمكن
   اصحاب feuille de route من العثور عليها بسهولة.
2. Résumé des résumés de l'enregistreur d'aperçu pour les détails de l'analyseur
   `status.md` (المشاركون، ابرز الملاحظات، الاصلاحات المخطط لها) et `roadmap.md`
   Veuillez consulter DOCS-SORA.
3. Comment procéder à l'offboarding
   [`reviewer-onboarding`](./reviewer-onboarding.md): الغ الوصول، ارشف الطلبات، واشكر المشاركين.
4. Utilisez la fonction Gates pour effectuer une somme de contrôle.
   وتحديث قالب الدعوة بتواريخ جديدة.تطبيق هذا الدليل بشكل متسق يحافظ على قابلية تدقيق برنامج المعاينة ويمنح Docs/DevRel
طريقة قابلة للتكرار لتوسيع الدعوات مع اقتراب البوابة من GA.