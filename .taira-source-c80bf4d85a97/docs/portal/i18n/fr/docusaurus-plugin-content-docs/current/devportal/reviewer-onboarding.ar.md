---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تأهيل مراجعي المعاينة

## نظرة عامة

DOCS-SORA est disponible pour la recherche en ligne. عمليات البناء المقيدة بالتحقق
(`npm run serve`) وتدفقات Essayez-le.
تفتح المعاينة العامة على نطاق واسع. يشرح هذا الدليل كيفية جمع الطلبات، والتحقق من الاهلية،
وتوفير الوصول، وانهاء المشاركة بشكل امن. ارجع الى
[prévisualiser le flux d'invitation](./preview-invite-flow.md) لتخطيط الدفعات، وتيرة الدعوات، وصادرات القياس
عن بعد؛ الخطوات ادناه تركز على الاجراءات بعد اختيار المراجع.

- **النطاق :** المراجعين الذين يحتاجون وصولا الى معاينة المستندات (`docs-preview.sora`,
  Consultez les pages GitHub et SoraFS) de GA.
- ** Paramètres d'intégration : ** Torii et SoraFS (pour l'intégration à bord) et
  البوابة الانتاجية (انظر
  [`devportal/deploy-guide`](./deploy-guide.md)).

## الادوار والمتطلبات المسبقة| الدور | الاهداف المعتادة | الاثار المطلوبة | ملاحظات |
| --- | --- | --- | --- |
| Responsable du noyau | التحقق من الادلة الجديدة، تشغيل tests de fumée. | Utilisez GitHub pour Matrix et CLA pour vous. | غالبا موجود بالفعل في فريق GitHub `docs-preview` ; مع ذلك قدم طلبا حتى يكون الوصول قابلا للتدقيق. |
| Réviseur partenaire | Il s'agit du SDK et du SDK. | بريد شركة، جهة اتصال قانونية، شروط aperçu موقعة. | يجب الاقرار بمتطلبات القياس عن بعد + التعامل مع البيانات. |
| Bénévole communautaire | تقديم ملاحظات قابلية الاستخدام على الادلة. | Utilisez GitHub pour créer un lien vers CoC. | حافظ على صغر الدفعات؛ اعط الاولوية للمراجعين الذين وقعوا اتفاقية المساهم. |

يجب على جميع انواع المراجعين:

1. الاقرار بسياسة الاستخدام المقبول لاثار المعاينة.
2. قراءة ملاحق الامن/الرصد
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. Utilisez le code `docs/portal/scripts/preview_verify.sh` pour le télécharger.
   instantané محليا.

## سير عمل apport1. اطلب من مقدم الطلب تعبئة نموذج
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   (او نسخه/لصقه في issue). Lien vers la page : Lien vers la page d'accueil via GitHub et vers la page d'accueil
   المقصودة، وتاكيد ان مستندات الامن تمت قراءتها.
2. Téléchargez le code `docs-preview` (issue GitHub et version ultérieure) et téléchargez-le.
3. تحقق من المتطلبات المسبقة:
   - CLA / اتفاقية مساهم في الملف (او مرجع عقد شريك).
   - اقرار الاستخدام المقبول محفوظ داخل الطلب.
   - تقييم مخاطر مكتمل (مثال: مراجعي الشركاء تمت الموافقة عليهم من Legal).
4. يوقع المعتمد على الطلب ويربط issue المتابعة باي ادخال لادارة التغيير
   (Portrait : `DOCS-SORA-Preview-####`).

## التزويد والادوات

1. **مشاركة الاثار** — قدم اخر descriptor + ارشيف معاينة من workflow CI et pin SoraFS
   (اثر `docs-portal-preview`). ذكر المراجعين بتشغيل:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **La somme de contrôle est la somme de contrôle** — et la somme de contrôle est la suivante :

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   Vous devez utiliser `scripts/serve-verified-preview.mjs` pour construire votre build.

3. **منح وصول GitHub (اختياري)** — اذا احتاج المراجعون الى فروع غير منشورة، اضفهم الى فريق
   GitHub `docs-preview` est une source d'information et une version ultérieure.

4. **قنوات الدعم** — شارك جهة الاتصال المناوبة (Matrix/Slack) et واجراء الحوادث من
   [`incident-runbooks`](./incident-runbooks.md).5. **القياس عن بعد + الملاحظات** — ذكر المراجعين بان التحليلات المجهولة يتم جمعها
   (`observability`](./observability.md)). وفر نموذج الملاحظات او قالب numéro المذكور في الدعوة
   وسجل الحدث عبر المساعد [`preview-feedback-log`](./preview-feedback-log) كي يبقى ملخص الموجة محدثا.

## قائمة المراجع

قبل الوصول الى المعاينة، يجب على المراجعين اكمال التالي:

1. التحقق من الاثار التي تم تنزيلها (`preview_verify.sh`).
2. Utilisez la somme de contrôle `npm run serve` (`serve:verified`) pour utiliser la somme de contrôle.
3. قراءة ملاحظات الامن والرصد المرتبطة اعلاه.
4. Essayez OAuth/Try it en utilisant le code de l'appareil (ان كان ذلك ينطبق) et en utilisant le code de l'appareil.
   رموز الانتاج.
5. تسجيل الملاحظات في المتتبع المتفق عليه (numéro او مستند مشترك او نموذج) et وضع وسم اصدار المعاينة.

## مسؤوليات المينتينرز وانهاء المشاركة| المرحلة | الاجراءات |
| --- | --- |
| Coup d'envoi | تاكيد ان قائمة apport مرفقة بالطلب، مشاركة الاثار + التعليمات، اضافة ادخال `invite-sent` عبر [`preview-feedback-log`](./preview-feedback-log) ، وجدولة مزامنة في منتصف الفترة اذا استمرت المراجعة لاكثر من اسبوع. |
| Surveillance | مراقبة قياس المعاينة (حركة Essayez-le غير معتادة، فشل sonde) et runbook الحوادث اذا كان هناك شيء مريب. سجل احداث `feedback-submitted`/`issue-opened` عند وصول الملاحظات حتى تبقى مقاييس الموجة دقيقة. |
| Débarquement | Le lien vers GitHub et SoraFS, puis `access-revoked`, est disponible (plus de détails + détails). المعلقة) et وتحديث سجل المراجعين. اطلب من المراجع حذف البنيات المحلية وارفاق الملخص الناتج من [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

استخدم نفس العملية عند تدوير المراجعين بين الموجات. الحفاظ على الاثر داخل المستودع (numéro + قوالب)
يساعد DOCS-SORA على البقاء قابلا للتدقيق ويسمح للحوكمة بتاكيد ان وصول المعاينة اتبع الضوابط الموثقة.

## قوالب الدعوة والتتبع- ابدء كل تواصل بملف
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  La somme de contrôle est la somme de contrôle la plus importante pour la somme de contrôle
  الاستخدام المقبول.
- عند تحرير القالب، استبدل العناصر النائبة لـ `<preview_tag>` و`<request_ticket>` وقنوات الاتصال.
  احفظ نسخة من الرسالة النهائية داخل تذكرة apport حتى يتمكن المراجعون والمعتمدون والمدققون من
  الرجوع الى النص الدقيق الذي تم ارساله.
- بعد ارسال الدعوة، حدث جدول التتبع او issue بطابع `invite_sent_at` وتاريخ الانتهاء المتوقع حتى يتمكن
  تقرير [prévisualiser le flux d'invitation](./preview-invite-flow.md) من التقاط الدفعة تلقائيا.