---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# تشديد الامن وقائمة فحص pen-test

## نظرة عامة

عنصر خارطة الطريق **DOCS-1b** يتطلب تسجيل دخول OAuth device-code، وسياسات امن محتوى قوية،
واختبارات اختراق قابلة للتكرار قبل ان يعمل بوابة المعاينة على شبكات خارج المختبر. يشرح هذا
الملحق نموذج التهديدات، والضوابط المطبقة في المستودع، وقائمة go-live التي يجب على مراجعات
البوابة تنفيذها.

- **النطاق:** وكيل Try it، ولوحات Swagger/RapiDoc المضمنة، ووحدة Try it المخصصة المعروضة عبر
  `docs/portal/src/components/TryItConsole.jsx`.
- **خارج النطاق:** Torii نفسه (مغطى بمراجعات جاهزية Torii) ونشر SoraFS (مغطى بـ DOCS-3/7).

## نموذج التهديدات

| الاصل | الخطر | التخفيف |
| --- | --- | --- |
| رموز Torii bearer | السرقة او اعادة الاستخدام خارج sandbox للوثائق | تسجيل الدخول device-code (`DOCS_OAUTH_*`) يصدر رموزا قصيرة العمر، والوكيل ينقح headers، والكونسول تنهي صلاحية بيانات الاعتماد المخزنة تلقائيا. |
| وكيل Try it | سوء الاستخدام كمرحّل مفتوح او تجاوز حدود Torii | `scripts/tryit-proxy*.mjs` يفرض قوائم السماح بالاصل، وrate limiting، وhealth probes، وتمرير `X-TryIt-Auth` بشكل صريح؛ لا يتم حفظ بيانات الاعتماد. |
| زمن تشغيل البوابة | XSS او تضمينات خبيثة | `docusaurus.config.js` يحقن ترويسات Content-Security-Policy وTrusted Types وPermissions-Policy؛ يتم تقييد scripts inline على runtime الخاص بـ Docusaurus. |
| بيانات المراقبة | غياب القياس عن بعد او العبث | `docs/portal/docs/devportal/observability.md` يوثق probes/dashboards؛ `scripts/portal-probe.mjs` يعمل في CI قبل النشر. |

تشمل الجهات المعادية مستخدمين فضوليين يشاهدون المعاينة العامة، ومهاجمين يجربون روابط مسروقة،
ومتصفحات مخترقة تحاول استخراج بيانات اعتماد مخزنة. يجب ان تعمل كل الضوابط على متصفحات عادية
دون شبكات موثوقة.

## الضوابط المطلوبة

1. **OAuth device-code login**
   - اضبط `DOCS_OAUTH_DEVICE_CODE_URL` و`DOCS_OAUTH_TOKEN_URL` و`DOCS_OAUTH_CLIENT_ID`
     والاعدادات ذات الصلة في بيئة البناء.
   - بطاقة Try it تعرض عنصر تسجيل الدخول (`OAuthDeviceLogin.jsx`) الذي يجلب device code
     ويستطلع token endpoint ويمسح الرموز تلقائيا عند انتهاء صلاحيتها. تبقى overrides
     اليدوية لـ Bearer متاحة كخيار طارئ.
   - تفشل عمليات البناء الان عند غياب اعدادات OAuth او عند خروج TTLs الخاصة بالـ fallback
     عن نافذة 300-900 s المطلوبة في DOCS-1b؛ اضبط `DOCS_OAUTH_ALLOW_INSECURE=1` فقط
     للمعاينات المحلية المؤقتة.
2. **حواجز وكيل البروكسي**
   - `scripts/tryit-proxy.mjs` يفرض allowed origins وrate limits وقيود حجم الطلب
     وupstream timeouts مع وسم الحركة بـ `X-TryIt-Client` وتنقيح الرموز من السجلات.
   - `scripts/tryit-proxy-probe.mjs` مع `docs/portal/docs/devportal/observability.md`
     يحددان liveness probe وقواعد dashboard؛ نفذها قبل كل rollout.
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` يصدر الان ترويسات امنية حتمية:
     `Content-Security-Policy` (default-src self، قوائم صارمة لـ connect/img/script،
     متطلبات Trusted Types)، و`Permissions-Policy` و`Referrer-Policy: no-referrer`.
   - قائمة connect في CSP تضع endpoints الخاصة بـ OAuth device-code وtoken ضمن whitelist
     (HTTPS فقط ما لم يتم ضبط `DOCS_SECURITY_ALLOW_INSECURE=1`) حتى يعمل device login
     دون تخفيف sandbox لبقية الاصول.
   - يتم تضمين الترويسات مباشرة في HTML المولد، لذلك لا تحتاج المضيفات الثابتة
     الى اعدادات اضافية. ابق scripts inline ضمن bootstrap الخاص بـ Docusaurus.
4. **Runbooks، observability، والرجوع**
   - `docs/portal/docs/devportal/observability.md` يصف probes وdashboards التي تراقب
     فشل تسجيل الدخول، واكواد استجابة البروكسي، وميزانيات الطلبات.
   - `docs/portal/docs/devportal/incident-runbooks.md` يغطي مسار التصعيد اذا تم
     اساءة استخدام sandbox؛ اجمعه مع
     `scripts/tryit-proxy-rollback.mjs` لتبديل endpoints بشكل امن.

## قائمة فحص pen-test والاصدار

اكمل هذه القائمة لكل ترقية معاينة (ارفق النتائج بتذكرة الاصدار):

1. **التحقق من توصيل OAuth**
   - شغل `npm run start` محليا مع exports الخاصة بـ `DOCS_OAUTH_*` في الانتاج.
   - من ملف تعريف متصفح نظيف، افتح كونسول Try it وتأكد ان تدفق device-code
     يصدر رمزا، ويعد العمر، ويمسح الحقل بعد انتهاء الصلاحية او تسجيل الخروج.
2. **اختبار البروكسي**
   - شغل `npm run tryit-proxy` ضد Torii staging، ثم نفذ
     `npm run probe:tryit-proxy` مع sample path المكون.
   - تحقق من السجلات بحثا عن `authSource=override` وتأكد ان rate limiting
     يزيد العدادات عند تجاوز النافذة.
3. **تاكيد CSP/Trusted Types**
   - شغل `npm run build` وافتح `build/index.html`. تاكد ان وسم `<meta
     http-equiv="Content-Security-Policy">` يطابق التوجيهات المتوقعة
     وان DevTools لا يظهر مخالفات CSP عند تحميل المعاينة.
   - استخدم `npm run probe:portal` (او curl) لجلب HTML المنشور؛ يفشل probe
     الان عندما تكون وسوم `Content-Security-Policy` او `Permissions-Policy` او
     `Referrer-Policy` مفقودة او مختلفة عن القيم المعلنة في
     `docusaurus.config.js`، بحيث يمكن لمراجعي الحوكمة الاعتماد على exit
     code بدلا من فحص خرج curl يدويا.
4. **مراجعة observability**
   - تحقق من ان لوحة Try it proxy خضراء (rate limits، error ratios، مقاييس health probe).
   - نفذ تمرين الحوادث في `docs/portal/docs/devportal/incident-runbooks.md`
     اذا تغير المضيف (نشر Netlify/SoraFS جديد).
5. **توثيق النتائج**
   - ارفق لقطات الشاشة/السجلات بتذكرة الاصدار.
   - سجل كل finding في قالب تقرير المعالجة
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     حتى يسهل تدقيق owners وSLAs وادلة retest لاحقا.
   - اربط هذا checklist مرة اخرى حتى يبقى عنصر roadmap DOCS-1b قابلا للتدقيق.

اذا فشل اي خطوة، اوقف الترقية، وافتح issue مانعة، ودون خطة المعالجة في `status.md`.
