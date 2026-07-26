---
lang: es
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
El cable de alimentación DOCS-SORA está diseñado para funcionar con dispositivos de seguridad y dispositivos electrónicos.
واضحا للتغذية الراجعة.

- **الجمهور:** قائمة منقحة من اعضاء المجتمع والشركاء والـ mantenedores الذين وقعوا سياسة الاستخدام المقبول للمعاينة.
- **الحدود:** حجم الموجة الافتراضي <= 25 مراجع، نافذة وصول 14 يوما، استجابة للحوادث خلال 24h.

## قائمة فحص بوابة الاطلاق

اكمل هذه المهام قبل ارسال اي دعوة:

1. اخر اثار المعاينة مرفوعة في CI (`docs-portal-preview`,
   suma de comprobación del manifiesto, descriptor, paquete SoraFS).
2. `npm run --prefix docs/portal serve` (suma de comprobación) para la etiqueta.
3. تذاكر تاهيل المراجعين معتمدة ومربوطة بموجة الدعوة.
4. مستندات الامن والمراقبة والحوادث مؤكدة
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
5. Comentarios y preguntas sobre el problema (يتضمن حقول الشدة، خطوات اعادة الانتاج، لقطات شاشة، ومعلومات البيئة).
6. Utilice Docs/DevRel + Governance para acceder a ellos.

## حزمة الدعوة

Aquí están las siguientes cosas:1. **اثار متحقق منها** — قدم روابط manifest/plan لـ SoraFS او artefacto de GitHub
   Esta es la suma de comprobación y el descriptor del manifiesto. اذكر امر التحقق صراحة حتى يتمكن المراجعون
   من تشغيله قبل تشغيل الموقع.
2. **تعليمات التشغيل** — ادرج امر المعاينة المقيد بالchecksum:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

3. **تذكيرات امنية** — اوضح ان الرموز تنتهي تلقائيا، والروابط لا يجب مشاركتها،
   ويجب الابلاغ عن الحوادث فورا.
4. **قناة feedback** — Problema de اربط نموذج/قالب y ووضح توقعات زمن الاستجابة.
5. **تواريخ البرنامج** — قدم تواريخ البداية/النهاية, ساعات المكتب او جلسات sync,
   ونافذة التحديث التالية.

البريد النموذجي في
[`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md)
يغطي هذه المتطلبات. حدّث العناصر النائبة (التواريخ، URL y وجهات الاتصال)
قبل الارسال.

## اظهار مضيف المعاينة

لا تروج لمضيف المعاينة الا بعد اكتمال التاهيل واعتماد تذكرة التغيير. راجع
[دليل اظهار مضيف المعاينة](./preview-host-exposure.md) Para construir/publicar/verificar desde el exterior
المستخدمة في هذا القسم.

1. **البناء والتعبئة:** ضع etiqueta de lanzamiento y اثارا حتمية.

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

   Pasador de conexión `portal.car`, `portal.manifest.*`, `portal.pin.proposal.json`,
   Y `portal.dns-cutover.json` en `artifacts/sorafs/`. ارفق هذه الملفات بموجة الدعوة
   حتى يتمكن كل مراجع من التحقق من نفس البتات.2. **نشر alias المعاينة:** اعد تشغيل الامر بدون `--skip-submit`
   (como `TORII_URL`, `AUTHORITY`, `PRIVATE_KEY[_FILE]`, y alias الصادر عن الحوكمة).
   سيقوم السكربت بربط manifiesto بـ `docs-preview.sora` ويصدر
   `portal.manifest.submit.summary.json` y `portal.pin.report.json` están conectados.

3. **فحص النشر:** تاكد من ان alias يحل وانه checksum يطابق etiqueta قبل ارسال الدعوات.

   ```bash
   npm run probe:portal -- \
     --base-url=https://docs-preview.sora.link \
     --expect-release="$DOCS_RELEASE_TAG"
   ```

   احتفظ بـ `npm run serve` (`scripts/serve-verified-preview.mjs`) para respaldo de respaldo
   المراجعون من تشغيل نسخة محلية اذا حدث خلل في vista previa del borde.

## جدول الاتصالات

| اليوم | الاجراء | Propietario |
| --- | --- | --- |
| D-3 | انهاء نص الدعوة، تحديث الاثار، تنفيذ funcionamiento en seco للتحقق | Documentos/DevRel |
| D-2 | موافقة الحوكمة + تذكرة تغيير | Documentos/DevRel + Gobernanza |
| D-1 | Dispositivos de seguimiento de dispositivos | Rastreadores de dispositivos | Documentos/DevRel |
| D | مكالمة inicio / horario de oficina، مراقبة لوحات القياس | Documentos/DevRel + De guardia |
| D+7 | resumen de comentarios في منتصف الموجة، triaje للقضايا المانعة | Documentos/DevRel |
| D+14 | Este producto está equipado con un conector `status.md` | Documentos/DevRel |

## تتبع الوصول والقياس عن بعد

1. سجل كل مستلم وطابع وقت الدعوة وتاريخ الالغاء باستخدام vista previa del registrador de comentarios
   (انظر [`preview-feedback-log`](./preview-feedback-log)) حتى تشترك كل موجة
   في نفس اثر الادلة:

   ```bash
   # اضافة حدث دعوة جديد الى artifacts/docs_portal_preview/feedback_log.json
   npm run --prefix docs/portal preview:log -- \
     --wave preview-20250303 \
     --recipient alice@example.com \
     --event invite-sent \
     --notes "wave-01 seed"
   ```Nombre del producto: `invite-sent`, `acknowledged`, `feedback-submitted`,
   `issue-opened`, y `access-revoked`. يعيش السجل افتراضيا في
   `artifacts/docs_portal_preview/feedback_log.json`; ارفقه بتذكرة موجة الدعوة
   مع نماذج الموافقة. استخدم مساعد resumen لانتاج resumen قابل للتدقيق قبل
   ملاحظة الاغلاق:

   ```bash
   npm run --prefix docs/portal preview:summary -- --summary-json \
     > artifacts/docs_portal_preview/preview-20250303-summary.json
   ```

   resumen JSON لكل موجة، المستلمين المفتوحين، تعداد feedback، وطابع
   الوقت لاخر حدث. المساعد يعتمد على
   [`scripts/preview-feedback-log.mjs`](../../scripts/preview-feedback-log.mjs),
   Aquí hay un flujo de trabajo completo y CI. استخدم قالب resumen في
   [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md)
   عند نشر resumen de للموجة.
2. ضع وسم `DOCS_RELEASE_TAG` المستخدم للموجة على لوحات القياس حتى يمكن ربط
   الارتفاعات مع cohorte الدعوات.
3. شغل `npm run probe:portal -- --expect-release=<tag>` بعد النشر لتاكيد
   ان بيئة المعاينة تعلن عن metadatos الصحيحة.
4. Haga clic en el runbook y en el archivo runbook.

## comentarios y comentarios

1. Comentarios sobre problemas relacionados con problemas y problemas. ضع وسم `docs-preview/<wave>` حتى يتمكن
   Hoja de ruta de اصحاب من العثور عليها بسهولة.
2. Resumen del resumen del registrador de vista previa del registro
   `status.md` (المشاركون، ابرز الملاحظات، الاصلاحات المخطط لها) y `roadmap.md`
   Utilice el documento DOCS-SORA.
3. اتبع خطوات offboarding من
   [`reviewer-onboarding`](./reviewer-onboarding.md): الغ الوصول، ارشف الطلبات، واشكر المشاركين.
4. جهز الموجة التالية عبر تحديث الاثار، اعادة تشغيل gates الخاصة بالchecksum،
   وتحديث قالب الدعوة بتواريخ جديدة.Para obtener más información, consulte Docs/DevRel.
طريقة قابلة للتكرار لتوسيع الدعوات مع اقتراب البوابة من GA.