---
lang: es
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مسار دعوات المعاينة

## الغرض

بند خارطة الطريق **DOCS-SORA** يحدد تأهيل المراجعين وبرنامج الدعوات للمعاينة العامة باعتبارهما اخر العوائق قبل خروج البوابة من beta. تصف هذه الصفحة كيفية فتح كل موجة دعوات، وما هي الاثار التي يجب شحنها قبل ارسال الدعوات، وكيفية اثبات قابلية التدقيق. استخدمها مع:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) للتعامل مع كل مراجع.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) suma de comprobación.
- [`devportal/observability`](./observability.md) لصادرات القياس عن بعد وروابط التنبيه.

## خطة الموجات| الموجة | الجمهور | معايير الدخول | معايير الخروج | ملاحظات |
| --- | --- | --- | --- | --- |
| **W0 - Mantenedores الاساسيون** | Los mantenedores de Docs/SDK son compatibles con el software. | Después de GitHub `docs-portal-preview`, realizó una suma de verificación en `npm run serve`, y Alertmanager comenzó a funcionar durante 7 días. | كل مستندات P0 تمت مراجعتها، backlog موسوم، لا توجد حوادث مانعة. | تستخدم للتحقق من المسار؛ لا بريد دعوة، فقط مشاركة اثار المعاينة. |
| **W1 - Socios** | El SoraFS, el Torii y el NDA. | خروج W0, الموافقات القانونية, وكيل Try-it في puesta en escena. | جمع موافقة الشركاء (problema او نموذج موقع)، القياس عن بعد يظهر =2 اصدارات توثيق تم شحنها عبر خط المعاينة دون rollback. | تحديد الدعوات المتزامنة (<=25) وجدولة اسبوعية. |

Utilice el dispositivo `status.md` y conecte los cables de conexión a la red.

## قائمة تحقق verificación previa

اتم هذه الاجراءات **قبل** جدولة الدعوات لموجة:1. **اثار CI متاحة**
   - اخر `docs-portal-preview` + descriptor تم رفعه بواسطة `.github/workflows/docs-portal-preview.yml`.
   - Pin de SoraFS a `docs/portal/docs/devportal/deploy-guide.md` (descriptor de nombre).
2. **فرض suma de comprobación**
   - `docs/portal/scripts/serve-verified-preview.mjs` تم استدعاؤه عبر `npm run serve`.
   - تم اختبار تعليمات `scripts/preview_verify.sh` para macOS + Linux.
3. **خط اساس للقياس عن بعد**
   - `dashboards/grafana/docs_portal.json` Pruébelo nuevamente y `docs.preview.integrity`.
   - Utilice el `docs/portal/docs/devportal/observability.md` para conectar el Grafana.
4. **اثار الحوكمة**
   - Problema لمتعقب الدعوات جاهزة (problema واحدة لكل موجة).
   - قالب سجل المراجعين منسوخ (انظر [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Problema de الموافقات القانونية و SRE المطلوبة مرفقة بالـ.

سجل اتمام prevuelo في متعقب الدعوات قبل ارسال اي بريد.

## خطوات المسار1. **اختيار المرشحين**
   - السحب من قائمة الانتظار او طابور الشركاء.
   - التأكد من ان لكل مرشح قالب طلب مكتمل.
2. **الموافقة على الوصول**
   - تعيين موافق على problema متعقب الدعوات.
   - التحقق من المتطلبات (CLA/عقد، استخدام مقبول، موجز امني).
3. **ارسال الدعوات**
   - اكمال الحقول في [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, جهات الاتصال).
   - ارفاق descriptor + hash الارشيف، عنوان staging لواجهة Pruébelo, وقنوات الدعم.
   - حفظ البريد النهائي (او محضر Matrix/Slack) في الـ problema.
4. **تتبع التأهيل**
   - Soportes de conexión `invite_sent_at`, `expected_exit_at` y (`pending`, `active`, `complete`, `revoked`).
   - ربط طلب دخول المراجع لضمان التدقيق.
5. **مراقبة القياس عن بعد**
   - مراقبة `docs.preview.session_active` y تنبيهات `TryItProxyErrors`.
   - فتح حادثة اذا انحرفت القياس عن الخط الاساس وتسجيل النتيجة بجانب مدخل الدعوة.
6. **جمع الملاحظات والخروج**
   - اغلاق الدعوات عند وصول الملاحظات او انتهاء `expected_exit_at`.
   - Problema de تحديث الموجة بملخص قصير (نتائج، حوادث، خطوات تالية) قبل الانتقال للمجموعة التالية.

## الادلة والتقارير| الاثر | مكان التخزين | وتيرة التحديث |
| --- | --- | --- |
| Número متعقب الدعوات | Soporte GitHub `docs-portal-preview` | تحديث بعد كل دعوة. |
| تصدير قائمة المراجعين | Fuente de alimentación para `docs/portal/docs/devportal/reviewer-onboarding.md` | اسبوعي. |
| لقطات القياس عن بعد | `docs/source/sdk/android/readiness/dashboards/<date>/` (اعادة استخدام حزمة القياس عن بعد) | لكل موجة + بعد الحوادث. |
| موجز الملاحظات | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (انشاء مجلد لكل موجة) | Haga 5 pasos desde el principio. |
| ملاحظة اجتماع الحوكمة | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Utilice el software DOCS-SORA. |

Número `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
بعد كل دفعة لانتاج موجز مقروء اليا. Problema con JSON بالكامل.

Asegúrese de que el dispositivo esté conectado a `status.md` y que el dispositivo esté encendido.

## معايير التراجع والتوقف

اوقف مسار الدعوات (واخطر الحوكمة) عند حدوث اي مما يلي:

- حادثة وكيل Pruébelo con la reversión (`npm run manage:tryit-proxy`).
- ارهاق التنبيهات: >3 صفحات تنبيه لنقاط نهاية خاصة بالمعاينة خلال 7 ايام.
- فجوة امتثال: ارسال دعوة بدون شروط موقعة او بدون تسجيل قالب الطلب.
- Error: falta de coincidencia en la suma de comprobación del código `scripts/preview_verify.sh`.

استأنف فقط بعد توثيق المعالجة في متعقب الدعوات وتأكيد استقرار لوحة القياس عن بعد لمدة 48 ساعة على الاقل.