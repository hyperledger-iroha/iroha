---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# تشديد الامن وقائمة فحص pen-test

## نظرة عامة

عنصر خارطة الطريق **DOCS-1b** يتطلب تسجيل دخول OAuth device-code, وسياسات امن محتوى قوية،
Certifique-se de que o dispositivo esteja funcionando corretamente e que você não tenha problemas com isso. يشرح هذا
الملحق نموذج التهديدات, والضوابط المطبقة في المستودع, وقائمة go-live التي يجب على مراجعات
Não há problema.

- **النطاق:** وكيل Try it, ولوحات Swagger/RapiDoc المضمنة, ووحدة Try it المخصصة المعروضة عبر
  `docs/portal/src/components/TryItConsole.jsx`.
- **خارج النطاق:** Torii نفسه (مغطى بمراجعات جاهزية Torii) e SoraFS (مغطى بـ DOCS-3/7).

## نموذج التهديدات

| الاصل | الخطر | التخفيف |
| --- | --- | --- |
| Suporte Torii | السرقة او اعادة الاستخدام خارج sandbox للوثائق | O código do dispositivo (`DOCS_OAUTH_*`) é definido como o código do dispositivo e os cabeçalhos e os cabeçalhos بيانات الاعتماد المخزنة تلقائيا. |
| وكيل Experimente | سوء الاستخدام كمرحّل مفتوح او تجاوز حدود Torii | `scripts/tryit-proxy*.mjs` é um dispositivo de teste, limitador de taxa, sondas de saúde e `X-TryIt-Auth`. Não há nada que você possa fazer. |
| Produtos de limpeza | XSS e serviços de streaming | `docusaurus.config.js` é definido como Política de segurança de conteúdo, Tipos confiáveis ​​e Política de permissões; Não há scripts inline em tempo de execução como Docusaurus. |
| بيانات المراقبة | غياب القياس عن بعد او العبث | `docs/portal/docs/devportal/observability.md` sondas/painel de controle; `scripts/portal-probe.mjs` está no CI. |

تشمل الجهات المعادية مستخدمين فضوليين يشاهدون المعاينة العامة, ومهاجمين يجربون روابط مسروقة,
Você pode fazer isso com uma chave de fenda. Não há necessidade de fazer isso sem problemas
Eu não sei.

## الضوابط المطلوبة

1. **Login com código de dispositivo OAuth**
   - `DOCS_OAUTH_DEVICE_CODE_URL` e `DOCS_OAUTH_TOKEN_URL` e `DOCS_OAUTH_CLIENT_ID`
     والاعدادات ذات الصلة em qualquer lugar.
   - بطاقة Try it تعرض عنصر تسجيل الدخول (`OAuthDeviceLogin.jsx`) الذي يجلب código do dispositivo
     O ponto de extremidade do token é definido como um ponto de extremidade do token. Substituições de تبقى
     O portador do portador pode ser um problema.
   - تفشل عمليات البناء الان عند غياب اعدادات OAuth او عند خروج TTLs الخاصة بالـ fallback
     Entre 300 e 900 s no DOCS-1b; `DOCS_OAUTH_ALLOW_INSECURE=1` `DOCS_OAUTH_ALLOW_INSECURE=1`
     للمعاينات المحلية المؤقتة.
2. **حواجز وكيل البروكسي**
   - `scripts/tryit-proxy.mjs` يفرض origens permitidas e limites de taxa وقيود حجم الطلب
     e os tempos limite de upstream são os tempos limite de `X-TryIt-Client` e os tempos limite de upstream.
   - `scripts/tryit-proxy-probe.mjs` ou `docs/portal/docs/devportal/observability.md`
     يحددان sonda de vivacidade e painel de controle; Este é o lançamento.
3. **CSP, tipos confiáveis, política de permissões**
   - `docusaurus.config.js` يصدر الان ترويسات امنية حتمية:
     `Content-Security-Policy` (default-src self), é usado para conectar/img/script,
     متطلبات Tipos confiáveis), و`Permissions-Policy` و`Referrer-Policy: no-referrer`.
   - Conecte-se ao CSP com endpoints, código de dispositivo OAuth e token na lista de permissões
     (HTTPS فقط ما لم يتم ضبط `DOCS_SECURITY_ALLOW_INSECURE=1`) حتى يعمل login do dispositivo
     Você pode usar sandbox para isso.
   - يتم تضمين الترويسات مباشرة em HTML المولد, لذلك لا تحتاج المضيفات الثابتة
     Não há problema. Os scripts in-line são o bootstrap do Docusaurus.
4. **Runbooks, observabilidade, e outros**
   - `docs/portal/docs/devportal/observability.md` Testes e painéis de controle
     فشل تسجيل الدخول, واكواد استجابة البروكسي, وميزانيات الطلبات.
   - `docs/portal/docs/devportal/incident-runbooks.md` يغطي مسار التصعيد اذا تم
     Como usar sandbox; اجمعه مع
     `scripts/tryit-proxy-rollback.mjs` para endpoints não é compatível.

## قائمة فحص pen-test والاصدار

اكمل هذه القائمة لكل ترقية معاينة (ارفق النتائج بتذكرة الاصدار):1. **Como usar o OAuth**
   - O `npm run start` é exportado para `DOCS_OAUTH_*` no país.
   - من ملف تعريف متصفح نظيف, افتح كونسول Experimente وتأكد ان تدفق código do dispositivo
     يصدر رمزا, ويعد العمر, ويمسح الحقل بعد انتهاء الصلاحية او تسجيل الخروج.
2. **اختبار البروكسي**
   - `npm run tryit-proxy` ou Torii staging;
     `npm run probe:tryit-proxy` é o caminho de amostra do caminho.
   - تحقق من السجلات بحثا عن `authSource=override` e ان rate limiting
     Não use nenhum recurso para isso.
3. **CSP/tipos confiáveis**
   - É `npm run build` e `build/index.html`. تاكد ان وسم `<meta
     http-equiv="Política de Segurança de Conteúdo">` يطابق التوجيهات المتوقعة
     E o DevTools não permite que o CSP seja instalado.
   - Use `npm run probe:portal` (como curl) para HTML. sonda
     O código é `Content-Security-Policy` e `Permissions-Policy` e
     `Referrer-Policy` é um dispositivo e um dispositivo de armazenamento de dados
     `docusaurus.config.js`, بحيث يمكن لمراجعي الحوكمة الاعتماد على exit
     code é o que você precisa para curl يدويا.
4. **observabilidade significativa**
   - تحقق من ان لوحة Experimente proxy خضراء (limites de taxa, taxas de erro, investigação de saúde).
   - O código de barras está em `docs/portal/docs/devportal/incident-runbooks.md`
     Isso é feito por meio de Netlify/SoraFS.
5. **توثيق النتائج**
   - ارفق لقطات الشاشة/السجلات بتذكرة الاصدار.
   - سجل كل encontrar في قالب تقرير المعالجة
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     Você pode verificar os proprietários e SLAs e testar novamente.
   - اربط هذا checklist مرة اخرى حتى يبقى عنصر roadmap DOCS-1b قابلا للتدقيق.

Isso é um problema que pode ser resolvido e emitido em `status.md`.