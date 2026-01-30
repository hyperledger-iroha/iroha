---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# مسار دعوات المعاينة

## الغرض

بند خارطة الطريق **DOCS-SORA** يحدد تأهيل المراجعين وبرنامج الدعوات للمعاينة العامة باعتبارهما اخر العوائق قبل خروج البوابة من beta. تصف هذه الصفحة كيفية فتح كل موجة دعوات، وما هي الاثار التي يجب شحنها قبل ارسال الدعوات، وكيفية اثبات قابلية التدقيق. استخدمها مع:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) للتعامل مع كل مراجع.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) لضمانات checksum.
- [`devportal/observability`](./observability.md) لصادرات القياس عن بعد وروابط التنبيه.

## خطة الموجات

| الموجة | الجمهور | معايير الدخول | معايير الخروج | ملاحظات |
| --- | --- | --- | --- | --- |
| **W0 - Maintainers الاساسيون** | Maintainers من Docs/SDK يتحققون من محتوى اليوم الاول. | فريق GitHub `docs-portal-preview` مكتمل، بوابة checksum في `npm run serve` خضراء، Alertmanager هادئ لمدة 7 ايام. | كل مستندات P0 تمت مراجعتها، backlog موسوم، لا توجد حوادث مانعة. | تستخدم للتحقق من المسار؛ لا بريد دعوة، فقط مشاركة اثار المعاينة. |
| **W1 - Partners** | مشغلو SoraFS، متكاملوا Torii، ومراجعو الحوكمة تحت NDA. | خروج W0، الموافقات القانونية، وكيل Try-it في staging. | جمع موافقة الشركاء (issue او نموذج موقع)، القياس عن بعد يظهر <=10 مراجعين متزامنين، لا تراجعات امنية لمدة 14 يوما. | فرض قالب الدعوة + تذاكر الطلب. |
| **W2 - المجتمع** | مساهمون مختارون من قائمة الانتظار المجتمعية. | خروج W1، تدريبات الحوادث مجربة، FAQ العامة محدثة. | استيعاب الملاحظات، >=2 اصدارات توثيق تم شحنها عبر خط المعاينة دون rollback. | تحديد الدعوات المتزامنة (<=25) وجدولة اسبوعية. |

وثق اي موجة نشطة داخل `status.md` وفي متعقب طلبات المعاينة حتى ترى الحوكمة الوضع بنظرة واحدة.

## قائمة تحقق preflight

اتم هذه الاجراءات **قبل** جدولة الدعوات لموجة:

1. **اثار CI متاحة**
   - اخر `docs-portal-preview` + descriptor تم رفعه بواسطة `.github/workflows/docs-portal-preview.yml`.
   - Pin لـ SoraFS موثق في `docs/portal/docs/devportal/deploy-guide.md` (descriptor التحويل موجود).
2. **فرض checksum**
   - `docs/portal/scripts/serve-verified-preview.mjs` تم استدعاؤه عبر `npm run serve`.
   - تم اختبار تعليمات `scripts/preview_verify.sh` على macOS + Linux.
3. **خط اساس للقياس عن بعد**
   - `dashboards/grafana/docs_portal.json` يظهر حركة Try it بصحة، وتنبيه `docs.preview.integrity` باللون الاخضر.
   - اخر ملحق في `docs/portal/docs/devportal/observability.md` محدث بروابط Grafana.
4. **اثار الحوكمة**
   - Issue لمتعقب الدعوات جاهزة (issue واحدة لكل موجة).
   - قالب سجل المراجعين منسوخ (انظر [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - الموافقات القانونية وSRE المطلوبة مرفقة بالـ issue.

سجل اتمام preflight في متعقب الدعوات قبل ارسال اي بريد.

## خطوات المسار

1. **اختيار المرشحين**
   - السحب من قائمة الانتظار او طابور الشركاء.
   - التأكد من ان لكل مرشح قالب طلب مكتمل.
2. **الموافقة على الوصول**
   - تعيين موافق على issue متعقب الدعوات.
   - التحقق من المتطلبات (CLA/عقد، استخدام مقبول، موجز امني).
3. **ارسال الدعوات**
   - اكمال الحقول في [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, جهات الاتصال).
   - ارفاق descriptor + hash الارشيف، عنوان staging لواجهة Try it، وقنوات الدعم.
   - حفظ البريد النهائي (او محضر Matrix/Slack) في الـ issue.
4. **تتبع التأهيل**
   - تحديث متعقب الدعوات بالقيم `invite_sent_at` و`expected_exit_at` والحالة (`pending`, `active`, `complete`, `revoked`).
   - ربط طلب دخول المراجع لضمان التدقيق.
5. **مراقبة القياس عن بعد**
   - مراقبة `docs.preview.session_active` وتنبيهات `TryItProxyErrors`.
   - فتح حادثة اذا انحرفت القياس عن الخط الاساس وتسجيل النتيجة بجانب مدخل الدعوة.
6. **جمع الملاحظات والخروج**
   - اغلاق الدعوات عند وصول الملاحظات او انتهاء `expected_exit_at`.
   - تحديث issue الموجة بملخص قصير (نتائج، حوادث، خطوات تالية) قبل الانتقال للمجموعة التالية.

## الادلة والتقارير

| الاثر | مكان التخزين | وتيرة التحديث |
| --- | --- | --- |
| Issue متعقب الدعوات | مشروع GitHub `docs-portal-preview` | تحديث بعد كل دعوة. |
| تصدير قائمة المراجعين | السجل المرتبط في `docs/portal/docs/devportal/reviewer-onboarding.md` | اسبوعي. |
| لقطات القياس عن بعد | `docs/source/sdk/android/readiness/dashboards/<date>/` (اعادة استخدام حزمة القياس عن بعد) | لكل موجة + بعد الحوادث. |
| موجز الملاحظات | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (انشاء مجلد لكل موجة) | خلال 5 ايام من خروج الموجة. |
| ملاحظة اجتماع الحوكمة | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | تعبئة قبل كل مزامنة DOCS-SORA. |

شغل `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
بعد كل دفعة لانتاج موجز مقروء اليا. ارفق JSON المولد بالـ issue الخاصة بالموجة كي يتمكن مراجعو الحوكمة من تأكيد اعداد الدعوات دون اعادة تشغيل السجل بالكامل.

ارفق قائمة الادلة بـ `status.md` عند انتهاء كل موجة حتى يتم تحديث مدخل خارطة الطريق بسرعة.

## معايير التراجع والتوقف

اوقف مسار الدعوات (واخطر الحوكمة) عند حدوث اي مما يلي:

- حادثة وكيل Try it تطلبت rollback (`npm run manage:tryit-proxy`).
- ارهاق التنبيهات: >3 صفحات تنبيه لنقاط نهاية خاصة بالمعاينة خلال 7 ايام.
- فجوة امتثال: ارسال دعوة بدون شروط موقعة او بدون تسجيل قالب الطلب.
- خطر نزاهة: mismatch في checksum تم اكتشافه بواسطة `scripts/preview_verify.sh`.

استأنف فقط بعد توثيق المعالجة في متعقب الدعوات وتأكيد استقرار لوحة القياس عن بعد لمدة 48 ساعة على الاقل.
