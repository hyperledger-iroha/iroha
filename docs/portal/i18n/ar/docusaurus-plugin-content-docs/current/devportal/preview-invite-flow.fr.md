---
lang: ar
direction: rtl
source: docs/portal/docs/devportal/preview-invite-flow.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# معاينة تدفق الدعوة

## موضوعي

يستشهد عنصر خريطة الطريق **DOCS-SORA** بضم المستشارين ومعاينة برنامج الدعوات العامة كأحدث الحواجز قبل الإصدار التجريبي. تتضمن هذه الصفحة تعليقًا على كل دعوات غامضة، والتي لها تأثيرها ومبالغها قبل إرسال الدعوات والتعليق على أن التدفق قابل للتدقيق. الاستخدام مع:

- [`devportal/reviewer-onboarding`](./reviewer-onboarding.md) للإدارة بواسطة القارئ.
- [`devportal/preview-integrity-plan`](./preview-integrity-plan.md) لضمانات المجموع الاختباري.
- [`devportal/observability`](./observability.md) لتصدير أجهزة القياس عن بعد وخطافات التنبيه.

## خطة الغموض

| غامضة | الجمهور | معايير الدخول | معايير الطلعة | ملاحظات |
| --- | --- | --- | --- | --- |
| **W0 - جوهر المشرفين** | مستندات الصيانة/SDK صالحة للمحتوى اليومي. | تجهيز GitHub `docs-portal-preview` peuplee، المجموع الاختباري للبوابة `npm run serve` en vert، Alertmanager silencieux 7 jours. | جميع المستندات P0 relus، العلامات المتراكمة، كل الحوادث كبيرة. | قم بإدخال تدفق صحيح؛ بدون إرسال دعوة عبر البريد الإلكتروني، سيتم مشاركة العناصر فقط من خلال معاينة. |
| **W1 - الشركاء** | المشغلون SoraFS، المتكاملون Torii، الحكامة على NDA. | W0 termine، termes juridiques approuves، proxy Try-it en Stage. | يقوم شركاء تسجيل الخروج بجمع (إصدار أو توقيع صيغة)، جهاز القياس عن بعد <= 10 وحدات متزامنة، عدم الانحدار الآمن لمدة 14 يومًا. | فرض نموذج دعوة + تذاكر الطلب. |
| **W2 - التواصل** | يتم اختيار المساهمين من خلال قائمة الحضور المجتمعي. | نهاية W1، تكرار الحوادث، الأسئلة الشائعة العامة خلال اليوم. | ردود الفعل تختلف، >= 2 إصدارات مستندية عاجلة عبر معاينة المسار بدون التراجع. | المحدد يدعو المتزامنين (<= 25) ويجمع كل مرة على حدة. |

Documentez quelle vague est active dans `status.md` et dans le Tracker desطالبات معاينة حتى تكون الحكومة في حالة انقلاب.

## قائمة المراجعة المبدئية

قم بإنهاء هذه الإجراءات **المسبقة** لتخطيط الدعوات بشكل غامض:

1. **القطع الأثرية المتوفرة في CI**
   - Dernier `docs-portal-preview` + الواصف المسؤول الاسمي `.github/workflows/docs-portal-preview.yml`.
   - دبوس SoraFS لاحظ في `docs/portal/docs/devportal/deploy-guide.md` (واصف القطع موجود).
2. **تطبيق المجموع الاختباري**
   - استدعاء `docs/portal/scripts/serve-verified-preview.mjs` عبر `npm run serve`.
   - تعليمات `scripts/preview_verify.sh` تم اختبارها على macOS + Linux.
3. **القياس عن بعد الأساسي**
   - `dashboards/grafana/docs_portal.json` قم بمراقبة حركة المرور حاول أن تكون صادقًا وتنبيه `docs.preview.integrity` هو في الاتجاه الصحيح.
   - آخر ملحق لـ `docs/portal/docs/devportal/observability.md` هو يوم مع الامتيازات Grafana.
4. **حوكمة المصنوعات اليدوية**
   - إصدار دعوة تعقب prete (مسألة اسمية غامضة).
   - نسخة من نموذج تسجيل القراء (العرض [`docs/examples/docs_preview_request_template.md`](../../../examples/docs_preview_request_template.md)).
   - الموافقات القانونية وSRE تتطلب من الملحقين إصدارًا.

قم بتسجيل إنجاز الاختبار المبدئي في متتبع الدعوة قبل إرسال البريد الإلكتروني الجديد.

## إيتاب دو فلوكس

1. **اختيار المرشحين**
   - قم بسحب قائمة الاهتمام أو شركاء الملف.
   - التأكد من أن كل مرشح لديه نموذج طلب كامل.
2. **الموافقة على الدخول**
   - تعيين موافق على إصدار متتبع الدعوة.
   - التحقق من المتطلبات المسبقة (CLA/العقد، الاستخدام المقبول، الأمان المختصر).
3. **Envoyer les Invitations**
   - إكمال العناصر النائبة لـ [`docs/examples/docs_preview_invite_template.md`](../../../examples/docs_preview_invite_template.md) (`<preview_tag>`، `<request_ticket>`، جهات الاتصال).
   - انضم إلى الواصف + تجزئة الأرشيف وعنوان URL المرحلي وجربه ودعم القنوات.
   - مخزن البريد الإلكتروني النهائي (أو نسخة Matrix/Slack) في الإصدار.
4. **المتابعة المستمرة**
   - قم بمتابعة متتبع الدعوة يوميًا باستخدام `invite_sent_at` و`expected_exit_at` والحالة (`pending` و`active` و`complete` و`revoked`).
   - طلب إدخال القارئ للتدقيق.
5. **مراقب القياس عن بعد**
   - المراقب `docs.preview.session_active` والتنبيهات `TryItProxyErrors`.
   - افتح حادثة إذا خرج القياس عن بعد من خط الأساس وقم بتسجيل النتيجة في قائمة الدعوة.
6. **جمع التعليقات وفرزها**
   - قم بطباعة الدعوات عند وصول التعليقات أو وصول `expected_exit_at`.
   - Mettre a jour l'issue de vague avec un Court استئناف (إحصاءات، حوادث، إجراءات prochaines) قبل المرور إلى المجموعة التالية.

## الأدلة والتقارير

| قطعة أثرية | أوو مخزن | إيقاع اليوم |
| --- | --- | --- |
| إصدار متتبع الدعوة | بروجيت جيثب `docs-portal-preview` | Mettre a jour apres chaque دعوة. |
| تصدير قائمة du relecteurs | قم بالتسجيل في `docs/portal/docs/devportal/reviewer-onboarding.md` | هيبدومادير. |
| لقطات القياس عن بعد | `docs/source/sdk/android/readiness/dashboards/<date>/` (إعادة استخدام حزمة القياس عن بعد) | حوادث غامضة + ما بعد الحادث. |
| ملخص ردود الفعل | `docs/portal/docs/devportal/preview-feedback/<wave>/summary.md` (إنشاء ملف بشكل غامض) | في الخمس أيام التالية لنوع غامض. |
| ملاحظة دي ريونيون الحكم | `docs/portal/docs/devportal/preview-invite-notes/<date>.md` | نموذج لحوكمة المزامنة المتقدمة DOCS-SORA. |

لانسيز `cargo xtask docs-preview summary --wave <wave_label> --json artifacts/docs_portal_preview/<wave_label>_summary.json`
بعد كل دفعة، من أجل إنتاج آلة هضم غير مقبولة. يؤدي إضافة ملف JSON إلى إصدار أمر غامض حتى يتمكن الباحثون من التحكم في حسابات الدعوات دون تجديد السجل.

انضم إلى قائمة الاقتراحات إلى `status.md` في كل مرة من الغموض حتى تتمكن من تنفيذ خريطة الطريق الرئيسية بسرعة.

## معايير التراجع والإيقاف المؤقت

توقف مؤقتًا عن تدفق الدعوات (وإخطار الإدارة) عند بقاء إحدى الحالات التالية:

- وكيل الحادث حاول ذلك قبل أن تحتاج إلى التراجع (`npm run manage:tryit-proxy`).
- تنبيهات التعب: >3 صفحات تنبيه لمعاينة نقاط النهاية لمدة 7 أيام فقط.
- بطاقة مطابقة: دعوة مبعوث بدون شروط التوقيع أو بدون تسجيل قالب الطلب.
- خطر التكامل: عدم تطابق المجموع الاختباري المكتشف على `scripts/preview_verify.sh`.

أعد إعداده مرة أخرى بعد توثيق الإصلاح في متتبع الدعوة وتأكد من أن القياس عن بعد للوحة القيادة مستقر على الأقل لمدة 48 ساعة.