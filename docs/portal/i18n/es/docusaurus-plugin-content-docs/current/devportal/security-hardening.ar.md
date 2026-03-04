---
lang: es
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تشديد الامن وقائمة فحص prueba de penetración

## نظرة عامة

عنصر خارطة الطريق **DOCS-1b** يتطلب تسجيل دخول OAuth dispositivo-código, y سياسات امن محتوى قوية,
واختبارات اختراق قابلة للتكرار قبل ان يعمل بوابة المعاينة على شبكات خارج المختبر. يشرح هذا
الملحق نموذج التهديدات، والضوابط المطبقة في المستودع، وقائمة go-live التي يجب على مراجعات
البوابة تنفيذها.

- **النطاق:** وكيل Pruébalo, ولوحات Swagger/RapiDoc المضمنة، ووحدة Pruébalo المخصصة المعروضة عبر
  `docs/portal/src/components/TryItConsole.jsx`.
- **خارج النطاق:** Torii نفسه (مغطى بمراجعات جاهزية Torii) and نشر SoraFS (مغطى بـ DOCS-3/7).

## نموذج التهديدات

| الاصل | الخطر | التخفيف |
| --- | --- | --- |
| portador Torii | Herramientas y accesorios Sandbox | Código de dispositivo (`DOCS_OAUTH_*`) Configuración de encabezados y encabezados الاعتماد المخزنة تلقائيا. |
| وكيل Pruébalo | Accesorios para el hogar y accesorios Torii | `scripts/tryit-proxy*.mjs` يفرض قوائم السماح بالاصل, وrate limiting, وhealth probes, وتمرير `X-TryIt-Auth` بشكل صريح؛ لا يتم حفظ بيانات الاعتماد. |
| زمن تشغيل البوابة | XSS y aplicaciones | `docusaurus.config.js` يحقن ترويسات Política-de-seguridad-de-contenido وTipos de confianza وPolítica-de-permisos؛ Hay scripts en línea en tiempo de ejecución como Docusaurus. |
| بيانات المراقبة | غياب القياس عن بعد او العبث | `docs/portal/docs/devportal/observability.md` يوثق sondas/tableros؛ `scripts/portal-probe.mjs` يعمل في CI قبل النشر. |تشمل الجهات المعادية مستخدمين فضوليين يشاهدون المعاينة العامة، ومهاجمين يجربون روابط مسروقة،
ومتصفحات مخترقة تحاول استخراج بيانات اعتماد مخزنة. يجب ان تعمل كل الضوابط على متصفحات عادية
دون شبكات موثوقة.

## الضوابط المطلوبة1. **Inicio de sesión con código de dispositivo OAuth**
   - Nombre `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL` y `DOCS_OAUTH_CLIENT_ID`.
     والاعدادات ذات الصلة في بيئة البناء.
   - بطاقة Pruébelo تعرض عنصر تسجيل الدخول (`OAuthDeviceLogin.jsx`) الذي يجلب código de dispositivo
     ويستطلع punto final del token ويمسح الرموز تلقائيا عند انتهاء صلاحيتها. تبقى anula
     اليدوية لـ Portador متاحة كخيار طارئ.
   - تفشل عمليات البناء الان عند غياب اعدادات OAuth y عند خروج TTLs الخاصة بالـ fallback
     عن نافذة 300-900 s المطلوبة في DOCS-1b؛ Número `DOCS_OAUTH_ALLOW_INSECURE=1`
     للمعاينات المحلية المؤقتة.
2. **حواجز وكيل البروكسي**
   - `scripts/tryit-proxy.mjs` يفرض orígenes permitidos وlímites de velocidad وقيود حجم الطلب
     وسم الحركة بـ `X-TryIt-Client` وتنقيح الرموز من السجلات.
   - `scripts/tryit-proxy-probe.mjs` a `docs/portal/docs/devportal/observability.md`
     يحددان sonda de vida y panel de control؛ نفذها قبل كل lanzamiento.
3. **CSP, tipos confiables, política de permisos**
   - `docusaurus.config.js` يصدر الان ترويسات امنية حتمية:
     `Content-Security-Policy` (predeterminado-src self, está conectado a connect/img/script,
     متطلبات Tipos de confianza), و`Permissions-Policy` و`Referrer-Policy: no-referrer`.
   - Conexión, CSP, puntos finales, código de dispositivo OAuth, token y lista blanca.
     (HTTPS فقط ما لم يتم ضبط `DOCS_SECURITY_ALLOW_INSECURE=1`) حتى يعمل inicio de sesión del dispositivo
     دون تخفيف sandbox لبقية الاصول.
   - يتم تضمين الترويسات مباشرة في HTML المولد، لذلك لا تحتاج المضيفات الثابتة
     الى اعدادات اضافية. Hay scripts en línea que funcionan con bootstrap en Docusaurus.4. **Runbooks, observabilidad, y والرجوع**
   - `docs/portal/docs/devportal/observability.md` Sondas y tableros de instrumentos التي تراقب
     فشل تسجيل الدخول، واكواد استجابة البروكسي، وميزانيات الطلبات.
   - `docs/portal/docs/devportal/incident-runbooks.md` يغطي مسار التصعيد اذا تم
     اساءة استخدام sandbox؛ اجمعه مع
     `scripts/tryit-proxy-rollback.mjs` Todos los puntos finales están disponibles.

## قائمة فحص prueba de penetración y والاصدار

اكمل هذه القائمة لكل ترقية معاينة (ارفق النتائج بتذكرة الاصدار):1. **التحقق من توصيل OAuth**
   - شغل `npm run start` محليا مع exports الخاصة بـ `DOCS_OAUTH_*` في الانتاج.
   - من ملف تعريف متصفح نظيف، افتح كونسول Pruébelo y تأكد ان تدفق código de dispositivo
     يصدر رمزا، ويعد العمر، ويمسح الحقل بعد انتهاء الصلاحية او تسجيل الخروج.
2. **اختبار البروكسي**
   - Estacionamiento `npm run tryit-proxy` y Torii, aquí
     `npm run probe:tryit-proxy` Aquí está la ruta de muestra المكون.
   - تحقق من السجلات بحثا عن `authSource=override` y تأكد ان limitación de velocidad
     يزيد العدادات عند تجاوز النافذة.
3. **Actualizar CSP/Tipos de confianza**
   - Aquí `npm run build` y `build/index.html`. تاكد ان وسم `` يطابق التوجيهات المتوقعة
     Y DevTools para crear CSP es una herramienta útil.
   - Texto `npm run probe:portal` (curl) en formato HTML sonda يفشل
     Aquí están los nombres de `Content-Security-Policy` y `Permissions-Policy` y
     `Referrer-Policy` Instalación y mantenimiento para el hogar
     `docusaurus.config.js`, بحيث يمكن لمراجعي الحوكمة الاعتماد على salida
     código بدلا من فحص خرج curl يدويا.
4. **observabilidad del مراجعة**
   - تحقق من ان لوحة Pruébelo proxy خضراء (límites de velocidad, tasas de error, مقاييس sonda de salud).
   - نفذ تمرين الحوادث في `docs/portal/docs/devportal/incident-runbooks.md`
     Esta es la versión de Netlify/SoraFS.
5. **توثيق النتائج**
   - ارفق لقطات الشاشة/السجلات بتذكرة الاصدار.
   - سجل كل encontrar في قالب تقرير المعالجة
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))حتى يسهل تدقيق propietarios وSLAs وادلة volver a realizar la prueba لاحقا.
   - Lista de verificación de la hoja de ruta DOCS-1b قابلا للتدقيق.

اذا فشل اي خطوة، اوقف الترقية، وافتح issues مانعة، ودون خطة المعالجة في `status.md`.