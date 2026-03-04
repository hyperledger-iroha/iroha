---
lang: es
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تأهيل مراجعي المعاينة

## نظرة عامة

DOCS-SORA يتابع اطلاقا مرحليا لبوابة المطورين. عمليات البناء المقيدة بالتحقق
(`npm run serve`) وتدفقات Pruébelo المحصنة تفتح المرحلة التالية: تأهيل مراجعين مدققين قبل ان
تفتح المعاينة العامة على نطاق واسع. يشرح هذا الدليل كيفية جمع الطلبات، والتحقق من الاهلية،
وتوفير الوصول، وانهاء المشاركة بشكل امن. ارجع الى
[vista previa del flujo de invitación](./preview-invite-flow.md) لتخطيط الدفعات، وتيرة الدعوات، وصادرات القياس
عن بعد؛ الخطوات ادناه تركز على الاجراءات بعد اختيار المراجع.

- **النطاق:** المراجعين الذين يحتاجون وصولا الى معاينة المستندات (`docs-preview.sora`,
  Utilice GitHub Pages y seleccione SoraFS) en GA.
- **خارج النطاق:** مشغلو Torii او SoraFS (مغطون بكتيبات onboarding الخاصة بهم) y عمليات نشر
  البوابة الانتاجية (انظر
  [`devportal/deploy-guide`](./deploy-guide.md)).

## الادوار والمتطلبات المسبقة| الدور | الاهداف المعتادة | الاثار المطلوبة | ملاحظات |
| --- | --- | --- | --- |
| Mantenedor principal | التحقق من الادلة الجديدة، تشغيل pruebas de humo. | En GitHub, utilice Matrix y CLA para acceder a él. | غالبا موجود بالفعل في فريق GitHub `docs-preview`; مع ذلك قدم طلبا حتى يكون الوصول قابلا للتدقيق. |
| Revisor socio | التحقق من مقتطفات SDK او محتوى الحوكمة قبل الاصدار العام. | بريد شركة، جهة اتصال قانونية، شروط vista previa موقعة. | يجب الاقرار بمتطلبات القياس عن بعد + التعامل مع البيانات. |
| Voluntario comunitario | تقديم ملاحظات قابلية الاستخدام على الادلة. | Para obtener GitHub, utilice el software CoC. | حافظ على صغر الدفعات؛ اعط الاولوية للمراجعين الذين وقعوا اتفاقية المساهم. |

يجب على جميع انواع المراجعين:

1. الاقرار بسياسة الاستخدام المقبول لاثار المعاينة.
2. قراءة ملاحق الامن/الرصد
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. الموافقة على تشغيل `docs/portal/scripts/preview_verify.sh` قبل تقديم اي
   instantánea محليا.

## ingesta de سير عمل1. اطلب من مقدم الطلب تعبئة نموذج
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   (او نسخه/لصقه في problema). التقط على الاقل: الهوية، وسيلة الاتصال، مقبض GitHub, تواريخ المراجعة
   المقصودة، وتاكيد ان مستندات الامن تمت قراءتها.
2. سجل الطلب في متتبع `docs-preview` (problema GitHub او تذكرة حوكمة) y عين معتمدا.
3. تحقق من المتطلبات المسبقة:
   - CLA / اتفاقية مساهم في الملف (او مرجع عقد شريك).
   - اقرار الاستخدام المقبول محفوظ داخل الطلب.
   - تقييم مخاطر مكتمل (مثال: مراجعي الشركاء تمت الموافقة عليهم من Legal).
4. يوقع المعتمد على الطلب ويربط problema المتابعة باي ادخال لادارة التغيير
   (مثال: `DOCS-SORA-Preview-####`).

## التزويد والادوات

1. **مشاركة الاثار** — Descriptor del flujo de trabajo + CI del flujo de trabajo del pin SoraFS
   (`docs-portal-preview`). ذكر المراجعين بتشغيل:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **التقديم مع فرض suma de comprobación** — وجه المراجعين الى الامر المقيد بالتحقق:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   هذا يعيد استخدام `scripts/serve-verified-preview.mjs` حتى لا يتم تشغيل build غير محقق بالخطأ.

3. **منح وصول GitHub (اختياري)** — اذا احتاج المراجعون الى فروع غير منشورة, اضفهم الى فريق
   GitHub `docs-preview` طوال فترة المراجعة وسجل تغيير العضوية في الطلب.

4. **قنوات الدعم** — شارك جهة الاتصال المناوبة (Matrix/Slack) y اجراء الحوادث من
   [`incident-runbooks`](./incident-runbooks.md).5. **القياس عن بعد + الملاحظات** — ذكر المراجعين بان التحليلات المجهولة يتم جمعها
   (`observability`](./observability.md)). وفر نموذج الملاحظات او قالب problema المذكور في الدعوة
   وسجل الحدث عبر المساعد [`preview-feedback-log`](./preview-feedback-log) كي يبقى ملخص الموجة محدثا.

## قائمة المراجع

قبل الوصول الى المعاينة, يجب على المراجعين اكمال التالي:

1. التحقق من الاثار التي تم تنزيلها (`preview_verify.sh`).
2. Haga clic en `npm run serve` (en `serve:verified`) para realizar la suma de comprobación.
3. قراءة ملاحظات الامن والرصد المرتبطة اعلاه.
4. اختبار وحدة OAuth/Pruébelo باستخدام تسجيل الدخول código de dispositivo (ان كان ذلك ينطبق) وتجنب اعادة استخدام
   رموز الانتاج.
5. تسجيل الملاحظات في المتتبع المتفق عليه (problema او مستند مشترك او نموذج) ووضع وسم اصدار المعاينة.

## مسؤوليات المينتينرز وانهاء المشاركة| المرحلة | الاجراءات |
| --- | --- |
| Inicio | تاكيد ان قائمة مرفقة بالطلب، مشاركة الاثار + التعليمات، اضافة ادخال `invite-sent` عبر [`preview-feedback-log`](./preview-feedback-log) ، وجدولة مزامنة في منتصف الفترة اذا استمرت المراجعة لاكثر من اسبوع. |
| Monitoreo | مراقبة قياس المعاينة (حركة Pruébelo غير معتادة، فشل sonda) y runbook الحوادث اذا كان هناك شيء مريب. Asegúrese de que `feedback-submitted`/`issue-opened` esté conectado a un dispositivo electrónico. |
| Baja de embarque | Haga clic en GitHub y SoraFS, descargue `access-revoked`, conecte el enlace (esta opción está disponible en + المعلقة) ، وتحديث سجل المراجعين. Haga clic en el enlace de abajo para obtener información sobre [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

استخدم نفس العملية عند تدوير المراجعين بين الموجات. الحفاظ على الاثر داخل المستودع (edición + قوالب)
Asegúrese de que DOCS-SORA esté disponible para su uso en el hogar y el hogar.

## قوالب الدعوة والتتبع- ابدء كل تواصل بملف
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  يلتقط الحد الادنى من اللغة القانونية وتعليمات checksum للمعاينة وتوقع ان يقر المراجعون بسياسة
  الاستخدام المقبول.
- Utilice el software de configuración `<preview_tag>` e `<request_ticket>` y sus dispositivos.
  احفظ نسخة من الرسالة النهائية داخل تذكرة ingesta حتى يتمكن المراجعون والمعتمدون والمدققون من
  الرجوع الى النص الدقيق الذي تم ارساله.
- Se ha producido un problema al volcar la traducción.
  تقرير [vista previa del flujo de invitación](./preview-invite-flow.md) من التقاط الدفعة تلقائيا.