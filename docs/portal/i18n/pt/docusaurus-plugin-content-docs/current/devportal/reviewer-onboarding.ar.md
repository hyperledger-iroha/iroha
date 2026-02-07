---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/reviewer-onboarding.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تأهيل مراجعي المعاينة

## نظرة عامة

DOCS-SORA não pode ser danificado. عمليات البناء المقيدة بالتحقق
(`npm run serve`) Tente experimentá-lo.
تفتح المعاينة العامة على نطاق واسع. يشرح هذا الدليل كيفية جمع الطلبات, والتحقق من الاهلية,
وتوفير الوصول, وانهاء المشاركة بشكل امن. ارجع الى
[fluxo de convite de visualização](./preview-invite-flow.md) لتخطيط الدفعات, وتيرة الدعوات, وصادرات القياس
عن بعد؛ Certifique-se de que o produto esteja funcionando corretamente.

- **النطاق:** المراجعين الذين يحتاجون وصولا الى معاينة المستندات (`docs-preview.sora`,
  Você pode usar GitHub Pages e SoraFS) no GA.
- **خارج النطاق:** مشغلو Torii e SoraFS (مغطون بكتيبات onboarding الخاصة بهم) وعمليات نشر
  البوابة الانتاجية (انظر
  [`devportal/deploy-guide`](./deploy-guide.md)).

## الادوار والمتطلبات المسبقة

| الدور | الاهداف المعتادة | الاثار المطلوبة | Produtos |
| --- | --- | --- | --- |
| Mantenedor principal | Faça testes de fumaça. | No GitHub, você pode usar o Matrix, CLA no site. | غالبا موجود بالفعل في ريق GitHub `docs-preview`; Isso significa que você não pode se preocupar com isso. |
| Revisor parceiro | O SDK está disponível e o SDK está disponível para download. | بريد شركة, جهة اتصال قانونية, شروط visualização موقعة. | يجب الاقرار بمتطلبات القياس عن بعد + التعامل مع البيانات. |
| Voluntário comunitário | Verifique se há algum problema com isso. | No GitHub, você pode usar o CoC para usar o CoC. | حافظ على صغر الدفعات؛ اعط الاولوية للمراجعين الذين وقعوا اتفاقية المساهم. |

يجب على جميع انواع المراجعين:

1. Verifique o valor da máquina.
2. قراءة ملاحق الامن/الرصد
   ([`security-hardening`](./security-hardening.md),
   [`observability`](./observability.md),
   [`incident-runbooks`](./incident-runbooks.md)).
3. O código de segurança `docs/portal/scripts/preview_verify.sh` é o mais importante.
   instantâneo.

## ingestão de alimentos

1. اطلب من مقدم الطلب تعبئة نموذج
   [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)
   (É um problema / لصقه في). Mais informações: الهوية, وسيلة الاتصال, مقبض GitHub, تواريخ المراجعة
   Não há nada que você possa fazer com isso.
2. Verifique o arquivo `docs-preview` (issue GitHub e تذكرة حوكمة) e também.
3. تحقق من المتطلبات المسبقة:
   - CLA / اتفاقية مساهم في الملف (او مرجع عقد شريك).
   - اقرار الاستخدام المقبول محفوظ داخل الطلب.
   - تقييم مخاطر مكتمل (مثال: مراجعي الشركاء تمت الموافقة عليهم من Legal).
4. يوقع المعتمد على الطلب ويربط issue المتابعة باي ادخال لادارة التغيير
   (exemplo: `DOCS-SORA-Preview-####`).

## التزويد والادوات

1. **مشاركة الاثار** — é o descritor + ارشيف معاينة do fluxo de trabalho CI e pin SoraFS
   (Erro `docs-portal-preview`). Como fazer isso:

   ```bash
   ./docs/portal/scripts/preview_verify.sh \
     --build-dir build \
     --descriptor artifacts/preview-descriptor.json \
     --archive artifacts/preview-site.tar.gz
   ```

2. **التقديم مع فرض checksum** — وجه المراجعين الى الامر المقيد بالتحقق:

   ```bash
   DOCS_RELEASE_TAG=preview-<stamp> npm run --prefix docs/portal serve
   ```

   A configuração `scripts/serve-verified-preview.mjs` permite que você construa algo sem problemas.

3. **منح وصول GitHub (اختياري)** — اذا احتاج المراجعون الى فروع غير منشورة, اضفهم الى فريق
   GitHub `docs-preview` é um arquivo de código aberto e um arquivo de código aberto no site.

4. **قنوات الدعم** — شارك جهة الاتصال المناوبة (Matrix/Slack) e واجراء الحوادث من
   [`incident-runbooks`](./incident-runbooks.md).

5. **القياس عن بعد + الملاحظات** — ذكر المراجعين بان التحليلات المجهولة يتم جمعها
   (`observability`](./observability.md)). وفر نموذج الملاحظات او قالب issue المذكور في الدعوة
   Você pode usar o recurso [`preview-feedback-log`](./preview-feedback-log) para fazer isso.

## قائمة المراجع

قبل الوصول الى المعاينة, يجب على المراجعين اكمال التالي:

1. Verifique o valor do arquivo (`preview_verify.sh`).
2. Use o `npm run serve` (e `serve:verified`) para obter a soma de verificação.
3. Coloque a chave de fenda e o cabo de alimentação.
4. Use o OAuth/Try it para obter o código do dispositivo (o código do dispositivo) e o código do dispositivo
   رموز الانتاج.
5. تسجيل الملاحظات في المتتبع المتفق عليه (issue او مستند مشترك او نموذج) ووضع وسم اصدار المعاينة.

## مسؤوليات المينتينرز وانهاء المشاركة| المرحلة | Produtos |
| --- | --- |
| Início | تاكيد ان قائمة ingestão مرفقة بالطلب, مشاركة الاثار + التعليمات, اضافة ادخال `invite-sent` عبر [`preview-feedback-log`](./preview-feedback-log), وجدولة مزامنة في منتصف الفترة اذا استمرت المراجعة لاكثر من اسبوع. |
| Monitoramento | مراقبة قياس المعاينة (حركة Try it غير معتادة, فشل probe) e runbook الحوادث اذا كان هناك شيء مريب. Verifique se o `feedback-submitted`/`issue-opened` está funcionando corretamente e sem problemas. |
| Desativação | Use o GitHub e o SoraFS, instale `access-revoked` e instale-o (por exemplo, + الاجراءات المعلقة), وتحديث سجل المراجعين. Verifique o valor do arquivo e o valor do arquivo em [`docs/examples/docs_preview_feedback_digest.md`](../../../examples/docs_preview_feedback_digest.md). |

Certifique-se de que não há nenhum problema com isso. الحفاظ على الاثر داخل المستودع (edição + قوالب)
DOCS-SORA é um documento de identificação completo e de alta qualidade. الضوابط الموثقة.

## قوالب الدعوة والتتبع

- ابدء كل تواصل بملف
  [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md).
  يلتقط الحد الادنى من اللغة القانونية e checksum للمعاينة وتوقع ان يقر المراجعون بسياسة
  الاستخدام المقبول.
- Verifique o valor do código `<preview_tag>` e `<request_ticket>` e `<request_ticket>`.
  احفظ نسخة من الرسالة النهائية داخل تذكرة حتى يتمكن المراجعون والمعتمدون والمدققون من
  الرجوع الى النص الدقيق الذي تم ارساله.
- بعد ارسال الدعوة, حدث جدول التتبع او issue بطابع `invite_sent_at` وتاريخ الانتهاء المتوقع حتى يتمكن
  تقرير [fluxo de convite de visualização](./preview-invite-flow.md) من التقاط الدفعة تلقائيا.