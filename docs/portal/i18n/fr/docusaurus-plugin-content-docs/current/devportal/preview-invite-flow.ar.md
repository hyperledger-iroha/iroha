---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/preview-invite-flow.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# مسار دعوات المعاينة

## الغرض

بند خارطة الطريق **DOCS-SORA** يحدد تأهيل المراجعين وبرنامج الدعوات للمعاينة العامة باعتبارهما اخر La version bêta est la version bêta. تصف هذه الصفحة كيفية فتح كل موجة دعوات، وما هي الاثار التي يجب شحنها قبل ارسال الدعوات، وكيفية اثبات قابلية التدقيق. استخدمها مع:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) للتعامل مع كل مراجع.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) est la somme de contrôle.
- [`devportal/observability`](./observability.md) لصادرات القياس عن بعد وروابط التنبيه.

## خطة الموجات| الموجة | الجمهور | معايير الدخول | معايير الخروج | ملاحظات |
| --- | --- | --- | --- | --- |
| **W0 - Mainteneurs الاساسيون** | Les responsables de Docs/SDK sont également disponibles. | GitHub `docs-portal-preview` utilise la somme de contrôle pour `npm run serve` d'Alertmanager depuis 7 ans. | Le projet P0 est également responsable du backlog pour le travail en cours. | تستخدم للتحقق من المسار؛ لا بريد دعوة، فقط مشاركة اثار المعاينة. |
| **W1 - Partenaires** | Il s'agit de SoraFS, de Torii et de NDA. | خروج W0، الموافقات القانونية، وكيل Try-it pour la mise en scène. | جمع موافقة الشركاء (numéro او نموذج موقع)، القياس عن بعد يظهر =2 اصدارات توثيق تم شحنها عبر خط المعاينة دون rollback. | تحديد الدعوات المتزامنة (<=25) وجدولة اسبوعية. |

وثق اي موجة نشطة داخل `status.md` وفي متعقب طلبات المعاينة حتى ترى الحوكمة الوضع بنظرة واحدة.

## قائمة تحقق contrôle en amont

اتم هذه الاجراءات **قبل** جدولة الدعوات لموجة:1. **اثار CI متاحة**
   - اخر `docs-portal-preview` + descripteur تم رفعه بواسطة `.github/workflows/docs-portal-preview.yml`.
   - Pin pour SoraFS par `docs/portal/docs/devportal/deploy-guide.md` (descripteur pour موجود).
2. ** فرض somme de contrôle **
   - `docs/portal/scripts/serve-verified-preview.mjs` est remplacé par `npm run serve`.
   - Vous pouvez utiliser `scripts/preview_verify.sh` pour macOS + Linux.
3. **خط اساس للقياس عن بعد**
   - `dashboards/grafana/docs_portal.json` يظهر حركة Essayez-le بصحة، وتنبيه `docs.preview.integrity` باللون الاخضر.
   - Utilisez `docs/portal/docs/devportal/observability.md` pour Grafana.
4. **اثار الحوكمة**
   - Numéro لمتعقب الدعوات جاهزة (numéro واحدة لكل موجة).
   - قالب سجل المراجعين منسوخ (انظر [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - Problème de الموافقات القانونية وSRE المطلوبة مرفقة بالـ.

سجل اتمام preflight في متعقب الدعوات قبل ارسال اي بريد.

## خطوات المسار1. **اختيار المرشحين**
   - السحب من قائمة الانتظار او طابور الشركاء.
   - التأكد من ان لكل مرشح قالب طلب مكتمل.
2. **الموافقة على الوصول**
   - تعيين موافق على issue متعقب الدعوات.
   - التحقق من المتطلبات (CLA/عقد، استخدام مقبول، موجز امني).
3. **ارسال الدعوات**
   - اكمال الحقول في [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`, `<request_ticket>`, جهات الاتصال).
   - ارفاق descriptor + hash الارشيف، عنوان staging لواجهة Essayez-le, وقنوات الدعم.
   - Il s'agit d'un problème (comme Matrix/Slack).
4. **تتبع التأهيل**
   - تحديث متعقب الدعوات بالقيم `invite_sent_at` et `expected_exit_at` et (`pending`, `active`, `complete`, `revoked`).
   - ربط طلب دخول المراجع لضمان التدقيق.
5. **مراقبة القياس عن بعد**
   - مراقبة `docs.preview.session_active` et `TryItProxyErrors`.
   - فتح حادثة اذا انحرفت القياس عن الخط الاساس وتسجيل النتيجة بجانب مدخل الدعوة.
6. **جمع الملاحظات والخروج**
   - اغلاق الدعوات عند وصول الملاحظات او انتهاء `expected_exit_at`.
   - تحديث issue الموجة بملخص قصير (نتائج، حوادث، خطوات تالية) قبل الانتقال للمجموعة التالية.

## الادلة والتقارير| الاثر | مكان التخزين | وتيرة التحديث |
| --- | --- | --- |
| Numéro متعقب الدعوات | Utiliser GitHub `docs-portal-preview` | تحديث بعد كل دعوة. |
| تصدير قائمة المراجعين | السجل المرتبط في `docs/portal/docs/devportal/reviewer-onboarding.md` | اسبوعي. |
| لقطات القياس عن بعد | `docs/source/sdk/android/readiness/dashboards/<date>/` (اعادة استخدام حزمة القياس عن بعد) | لكل موجة + بعد الحوادث. |
| موجز الملاحظات | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (انشاء مجلد لكل موجة) | خلال 5 ايام من خروج الموجة. |
| ملاحظة اجتماع الحوكمة | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | Téléchargez le document DOCS-SORA. |

Voir `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
بعد كل دفعة لانتاج موجز مقروء اليا. ارفق JSON المولد بالـ issue الخاصة بالموجة كي يتمكن مراجعو الحوكمة من تأكيد اعداد الدعوات دون اعادة تشغيل السجل بالكامل.

ارفق قائمة الادلة بـ `status.md` عند انتهاء كل موجة حتى يتم تحديث مدخل خارطة الطريق بسرعة.

## معايير التراجع والتوقف

اوقف مسار الدعوات (واخطر الحوكمة) عند حدوث اي مما يلي:

- حادثة وكيل Essayez-le تطلبت rollback (`npm run manage:tryit-proxy`).
- ارهاق التنبيهات: >3 صفحات تنبيه لنقاط نهاية خاصة بالمعاينة خلال 7 ايام.
- فجوة امتثال: ارسال دعوة بدون شروط موقعة او بدون تسجيل قالب الطلب.
- Problème : non-concordance dans la somme de contrôle comme indiqué par `scripts/preview_verify.sh`.

استأنف فقط بعد توثيق المعالجة عن بعد لمدة 48 ساعة على الاقل.