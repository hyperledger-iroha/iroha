---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تشديد الامن وقائمة فحص pen-test

## نظرة عامة

Vous pouvez également utiliser **DOCS-1b** pour télécharger le code de périphérique OAuth et le code de périphérique OAuth.
واختبارات اختراق قابلة للتكرار قبل ان يعمل بوابة المعاينة على شبكات خارج المختبر. يشرح هذا
الملحق نموذج التهديدات، والضوابط المطبقة في المستودع، وقائمة go-live التي يجب على مراجعات
البوابة تنفيذها.

- **النطاق :** et Essayez-le, et Swagger/RapiDoc et Essayez-le et essayez-le.
  `docs/portal/src/components/TryItConsole.jsx`.
- **Détails :** Torii (avec Torii) et SoraFS (avec Torii) DOCS-3/7).

## نموذج التهديدات

| الاصل | الخطر | التخفيف |
| --- | --- | --- |
| Porteur Torii | Bac à sable et bac à sable | Utilisez le code de l'appareil (`DOCS_OAUTH_*`) pour télécharger les en-têtes et les en-têtes. الاعتماد المخزنة تلقائيا. |
| وكيل Essayez-le | سوء الاستخدام كمرحّل مفتوح او تجاوز حدود Torii | `scripts/tryit-proxy*.mjs` pour les sondes de santé et de limitation de débit `X-TryIt-Auth` pour les sondes de santé لا يتم حفظ بيانات الاعتماد. |
| زمن تشغيل البوابة | XSS et fichiers XSS | `docusaurus.config.js` contient la stratégie de sécurité du contenu et les types de confiance et la stratégie d'autorisations. Il existe des scripts en ligne pour le runtime de Docusaurus. |
| بيانات المراقبة | غياب القياس عن بعد او العبث | `docs/portal/docs/devportal/observability.md` pour sondes/tableaux de bord `scripts/portal-probe.mjs` يعمل في CI قبل النشر. |تشمل الجهات المعادية مستخدمين فضوليين يشاهدون المعاينة العامة، ومهاجمين يجربون روابط مسروقة،
ومتصفحات مخترقة تحاول استخراج بيانات اعتماد مخزنة. يجب ان تعمل كل الضوابط على متصفحات عادية
دون شبكات موثوقة.

## الضوابط المطلوبة1. **Connexion par code de périphérique OAuth**
   - `DOCS_OAUTH_DEVICE_CODE_URL` et `DOCS_OAUTH_TOKEN_URL` et `DOCS_OAUTH_CLIENT_ID`
     والاعدادات ذات الصلة في بيئة البناء.
   - بطاقة Essayez-le تعرض عنصر تسجيل الدخول (`OAuthDeviceLogin.jsx`) dans le code de l'appareil
     Le point de terminaison du jeton est également disponible. تبقى remplacements
     اليدوية لـ Bearer متاحة كخيار طارئ.
   - Les applications OAuth et les TTLs de secours sont également disponibles.
     300-900 s المطلوبة pour DOCS-1b؛ `DOCS_OAUTH_ALLOW_INSECURE=1` فقط
     للمعاينات المحلية المؤقتة.
2. **حواجز وكيل البروكسي**
   - `scripts/tryit-proxy.mjs` يفرض origines autorisées وlimites de taux وقيود حجم الطلب
     Les délais d'attente en amont sont compatibles avec `X-TryIt-Client` et les délais d'attente en amont sont définis par `X-TryIt-Client`.
   - `scripts/tryit-proxy-probe.mjs` ou `docs/portal/docs/devportal/observability.md`
     يحددان sonde de vivacité et tableau de bord؛ نفذها قبل كل déploiement.
3. **CSP, types de confiance, politique d'autorisations**
   - `docusaurus.config.js` يصدر الان ترويسات امنية حتمية:
     `Content-Security-Policy` (default-src self, est disponible pour connect/img/script,
     (Types de confiance) et `Permissions-Policy` et `Referrer-Policy: no-referrer`.
   - Permet de se connecter aux points de terminaison CSP avec le code de périphérique OAuth et le jeton et la liste blanche
     (HTTPS فقط ما لم يتم ضبط `DOCS_SECURITY_ALLOW_INSECURE=1`) Permet de connecter l'appareil
     دون تخفيف sandbox لبقية الاصول.
   - يتم تضمين الترويسات مباشرة في HTML المولد، لذلك لا تحتاج المضيفات الثابتة
     الى اعدادات اضافية. Les scripts en ligne sont des outils d'amorçage pour Docusaurus.4. **Runbooks, observabilité, والرجوع**
   - `docs/portal/docs/devportal/observability.md` pour sondes et tableaux de bord
     فشل تسجيل الدخول، واكواد استجابة البروكسي، وميزانيات الطلبات.
   - `docs/portal/docs/devportal/incident-runbooks.md` يغطي مسار التصعيد اذا تم
     اساءة استخدام sandbox؛ اجمعه مع
     `scripts/tryit-proxy-rollback.mjs` Points de terminaison pour ici.

## قائمة فحص pen-test والاصدار

اكمل هذه القائمة لكل ترقية معاينة (ارفق النتائج بتذكرة الاصدار):1. **التحقق من توصيل OAuth**
   - شغل `npm run start` محليا مع exports الخاصة بـ `DOCS_OAUTH_*` في الانتاج.
   - من ملف تعريف متصفح نظيف، افتح كونسول Essayez-le et consultez le code de l'appareil
     يصدر رمزا، ويعد العمر، ويمسح الحقل بعد انتهاء الصلاحية او تسجيل الخروج.
2. **اختبار البروكسي**
   - شغل `npm run tryit-proxy` ضد Torii staging، ثم نفذ
     `npm run probe:tryit-proxy` est un exemple de chemin d'accès.
   - Utilisé pour la limitation de débit `authSource=override`
     يزيد العدادات عند تجاوز النافذة.
3. **CSP/Types de confiance**
   - Utilisez `npm run build` et `build/index.html`. تاكد ان وسم `` يطابق التوجيهات المتوقعة
     Et DevTools pour CSP est également disponible.
   - Fichier `npm run probe:portal` (boucle) pour HTML HTML يفشل sonde
     Il s'agit de `Content-Security-Policy` et `Permissions-Policy` et
     `Referrer-Policy` مفقودة او مختلفة عن القيم المعلنة في
     `docusaurus.config.js`, بحيث يمكن لمراجعي الحوكمة الاعتماد على exit
     code بدلا من فحص خرج curl يدويا.
4. **Observabilité مراجعة**
   - تحقق من ان لوحة Essayez-le proxy خضراء (limites de taux, taux d'erreur, sonde de santé).
   - نفذ تمرين الحوادث في `docs/portal/docs/devportal/incident-runbooks.md`
     Je l'ai consulté (Netlify/SoraFS ici).
5. **توثيق النتائج**
   - ارفق لقطات الشاشة/السجلات بتذكرة الاصدار.
   - سجل كل trouver في قالب تقرير المعالجة
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))Les propriétaires de propriétaires et les SLA doivent retester cela.
   - Liste de contrôle pour la liste de contrôle et la feuille de route DOCS-1b.

Il s'agit d'un problème lié au numéro `status.md`.