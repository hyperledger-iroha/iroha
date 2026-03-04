---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مسار دعوات المعاينة

## الغرض

Verifique se o documento **DOCS-SORA** está disponível para download e instalação باعتبارهما اخر العوائق قبل خروج البوابة em beta. تصف هذه الصفحة كيفية فتح كل موجة دعوات, وما هي الاثار التي يجب شحنها قبل ارسال Você pode fazer isso com antecedência. O que fazer:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) não funciona.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) soma de verificação.
- [`devportal/observability`](./observability.md) لصادرات القياس عن بعد وروابط التنبيه.

## خطة الموجات

| الموجة | الجمهور | معاير الدخول | معايير الخروج | Produtos |
| --- | --- | --- | --- | --- |
| **W0 - Mantenedores الاساسيون** | Mantenedores do Docs/SDK estão disponíveis para download. | No GitHub `docs-portal-preview` você pode usar a soma de verificação no `npm run serve`, o Alertmanager está em 7 dias. | Para que o P0 seja um backlog de pendências, você pode fazer o mesmo. | تستخدم للتحقق من المسار؛ Não há problema em fazer isso. |
| **W1 - Parceiros** | O documento SoraFS, o Torii e o contrato de NDA. | Use o W0, o programa de teste e o Try-it na preparação. | جمع موافقة الشركاء (issue او نموذج موقع), القياس عن بعد يظهر <=10 مراجعين متزامنين, لا تراجعات 14 de junho. | فرض قالب الدعوة + تذاكر الطلب. |
| **W2 - Nome** | Você pode fazer isso com o telefone. | خروج W1, تدريبات الحوادث مجربة, FAQ do site. | استيعاب الملاحظات, >=2 اصدارات توثيق تم شحنها عبر خط المعاينة دون rollback. | تحديد الدعوات المتزامنة (<=25) وجدولة اسبوعية. |

Você pode usar o `status.md` para obter mais informações sobre o seu produto. Então.

## قائمة تحقق comprovação

اتم هذه الاجراءات **قبل** جدولة الدعوات لموجة:

1. **اثار CI متاحة**
   - اخر `docs-portal-preview` + descritor تم رفعه بواسطة `.github/workflows/docs-portal-preview.yml`.
   - Pin لـ SoraFS موثق في `docs/portal/docs/devportal/deploy-guide.md` (descritor التحويل موجود).
2. **Soma de verificação**
   - `docs/portal/scripts/serve-verified-preview.mjs` é compatível com `npm run serve`.
   - Use o `scripts/preview_verify.sh` para macOS + Linux.
3. **خط اساس للقياس عن بعد**
   - `dashboards/grafana/docs_portal.json` يظهر حركة Try it بصحة, وتنبيه `docs.preview.integrity` باللون الاخضر.
   - Use o `docs/portal/docs/devportal/observability.md` para substituir o Grafana.
4. **اثار الحوكمة**
   - Emitir لمتعقب الدعوات جاهزة (emitir واحدة لكل موجة).
   - Faça o download do arquivo (`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - الموافقات القانونية وSRE المطلوبة مرفقة بالـ questão.

Faça o pré-voo em qualquer lugar do mundo.

## خطوات المسار

1. **اختيار المرشحين**
   - السحب من قائمة الانتظار او طابور الشركاء.
   - Não há nada de errado com você.
2. **الموافقة على الوصول**
   - تعيين موافق على issue متعقب الدعوات.
   - التحقق من المتطلبات (CLA/عقد, استخدام مقبول, موجز امني).
3. **ارسال الدعوات**
   - Use o código [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, جهات الاتصال).
   - ارفاق descritor + hash الارشيف, عنوان staging لواجهة Try it, وقنوات الدعم.
   - حفظ البريد النهائي (ou seja, Matrix/Slack) neste problema.
4. **تتبع التأهيل**
   - Você pode usar o código `invite_sent_at` e `expected_exit_at` e (`pending`, `active`, `complete`, `revoked`).
   - ربط طلب دخول المراجع لضمان التدقيق.
5. **مراقبة القياس عن بعد**
   - Número `docs.preview.session_active` e `TryItProxyErrors`.
   - فتح حادثة اذا انحرفت القياس عن الخط الاساس وتسجيل النتيجة بجانب مدخل الدعوة.
6. **جمع الملاحظات والخروج**
   - Verifique o número de telefone e o código `expected_exit_at`.
   - تحديث issue الموجة بملخص قصير (نتائج, حوادث, خطوات تالية) قبل الانتقال للمجموعة التالية.

## الادلة والتقارير

| الاثر | مكان التخزين | وتيرة التحديث |
| --- | --- | --- |
| Emitir متعقب الدعوات | Baixar GitHub `docs-portal-preview` | تحديث بعد كل دعوة. |
| تصدير قائمة المراجعين | Solução de problemas em `docs/portal/docs/devportal/reviewer-onboarding.md` | Bem. |
| لقطات القياس عن بعد | `docs/source/sdk/android/readiness/dashboards/<date>/` (Integração de código de barras) | لكل موجة + بعد الحوادث. |
| Máquinas de lavar | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (alterar o valor da moeda) | خلال 5 ايام من خروج الموجة. |
| Máquinas de lavar louça | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Limpe o produto DOCS-SORA. |

شغل `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
Você pode fazer isso com o dinheiro. ارفق JSON المولد بالـ issue الخاصة بالموجة كي يتمكن مراجعو الحوكمة من تأكيد اعداد الدعوات دون اعادة تشغيل السجل بالكامل.

A solução de problemas `status.md` é uma ferramenta que pode ser usada para obter mais informações. Bem.

## معايير التراجع والتوقف

اوقف مسار الدعوات (واخطر الحوكمة) عند حدوث اي مما يلي:- حادثة وكيل Try it تطلبت rollback (`npm run manage:tryit-proxy`).
- ارهاق التنبيهات: >3 صفحات تنبيه لنقاط نهاية خاصة بالمعاينة خلال 7 ايام.
- فجوة امتثال: ارسال دعوة بدون شروط موقعة او بدون تسجيل قالب الطلب.
- Não há: incompatibilidade na soma de verificação do valor `scripts/preview_verify.sh`.

Faça o download do seu cartão de crédito e verifique o valor do seu cartão de crédito. 48 dias atrás.