---
lang: es
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# سیکیورٹی ہارڈننگ اور pen-test چیک لسٹ

## جائزہ

روڈمیپ آئٹم **DOCS-1b** Inicio de sesión con código de dispositivo OAuth, مضبوط políticas de seguridad de contenido,
اور pruebas de penetración repetibles کا تقاضا کرتا ہے اس سے پہلے کہ vista previa پورٹل لیب سے باہر
نیٹ ورکس پر چل سکے۔ یہ ضمیمہ modelo de amenaza, repositorio میں implementar کئے گئے controles, اور go-live
چیک لسٹ بیان کرتا ہے جسے revisiones de puerta کو ejecutar کرنا ہوتا ہے۔

- **اسکوپ:** Pruébelo como proxy, paneles Swagger/RapiDoc integrados, o consola Pruébelo personalizada o
  `docs/portal/src/components/TryItConsole.jsx` سے render ہوتی ہے۔
- **آؤٹ آف اسکوپ:** Torii خود (Torii revisiones de preparación میں کور) اور SoraFS publicación
  (DOCS-3/7 میں کور).

## Modelo de amenaza| Activo | Riesgo | Mitigación |
| --- | --- | --- |
| Fichas al portador Torii | docs sandbox کے باہر robo یا reutilización | inicio de sesión con código de dispositivo (`DOCS_OAUTH_*`) tokens de corta duración mint کرتا ہے، encabezados de proxy کو redactar کرتا ہے، اور credenciales en caché de la consola کو caducidad automática کرتی ہے۔ |
| Pruébalo proxy | relé abierto کے طور پر abuso یا Torii límites de velocidad کا bypass | Listas permitidas de origen `scripts/tryit-proxy*.mjs`, limitación de velocidad, sondas de estado, y `X-TryIt-Auth`, aplicación de reenvío explícito, etc. Las credenciales de کوئی persisten نہیں ہوتے۔ |
| Tiempo de ejecución del portal | secuencias de comandos entre sitios e incrustaciones maliciosas | `docusaurus.config.js` Política de seguridad de contenido, tipos de confianza, y los encabezados de política de permisos inyectan کرتا ہے؛ scripts en línea Docusaurus tiempo de ejecución تک محدود ہیں۔ |
| Datos de observabilidad | falta telemetría یا manipulación | Documento de sondas/tableros `docs/portal/docs/devportal/observability.md` کرتا ہے؛ `scripts/portal-probe.mjs` publicar سے پہلے CI میں چلتا ہے۔ |

Adversarios میں vista previa pública دیکھنے والے usuarios curiosos، چوری شدہ enlaces کو prueba کرنے والے actores maliciosos،
Los navegadores comprometidos pueden raspar las credenciales almacenadas تمام controla کو
redes confiables کے بغیر navegadores básicos پر کام کرنا چاہیے۔

## Controles requeridos1. **Inicio de sesión con código de dispositivo OAuth**
   - `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` اور متعلقہ perillas کو construir entorno میں configurar کریں۔
   - Pruébelo tarjeta ایک widget de inicio de sesión (`OAuthDeviceLogin.jsx`) render کرتا ہے جو código de dispositivo buscar کرتا ہے،
     punto final del token پر encuesta کرتا ہے، اور caducidad پر tokens کو auto-clear کرتا ہے۔ Anulaciones manuales de portador
     respaldo de emergencia کے لئے دستیاب رہتے ہیں۔
   - Falta la configuración de OAuth ہو یا TTL alternativos DOCS-1b کے 300-900 s ventana سے باہر ہوں تو las compilaciones fallan ہوتے ہیں؛
     `DOCS_OAUTH_ALLOW_INSECURE=1` Vista previa local desechable کے لئے conjunto کریں۔
2. **Barandillas proxy**
   - `scripts/tryit-proxy.mjs` orígenes permitidos, límites de velocidad, límites de tamaño de solicitud, y tiempos de espera ascendentes que se aplican
     جبکہ `X-TryIt-Client` کے ساتھ etiqueta de tráfico کرتا ہے اور registros سے tokens redactar کرتا ہے۔
   - Sonda de vida `scripts/tryit-proxy-probe.mjs` o `docs/portal/docs/devportal/observability.md`
     Las reglas del panel de control definen کرتے ہیں؛ ہر lanzamiento سے پہلے انہیں چلائیں۔
3. **CSP, tipos confiables, política de permisos**
   - `docusaurus.config.js` اب encabezados de seguridad deterministas exportan کرتا ہے:
     `Content-Security-Policy` (predeterminado-src self, lista de conexiones/img/script, requisitos de tipos confiables),
     `Permissions-Policy`, o `Referrer-Policy: no-referrer`۔
   - Lista de conexión de CSP Código de dispositivo OAuth اور puntos finales de token کو lista blanca کرتی ہے
     (صرف HTTPS، جب تک `DOCS_SECURITY_ALLOW_INSECURE=1` نہ ہو) تاکہ inicio de sesión del dispositivo کام کرے
     اور دوسرے origins کے لئے sandbox relax نہ ہو۔- encabezados براہ راست HTML generado میں incrustar ہوتے ہیں، اس لئے hosts estáticos کو اضافی configuración کی ضرورت نہیں۔
     scripts en línea mediante arranque Docusaurus تک محدود رکھیں۔
4. **Runbooks, observabilidad y reversión**
   - Sondas `docs/portal/docs/devportal/observability.md`, paneles de control, fallas de inicio de sesión, códigos de respuesta de proxy,
     اور solicitar presupuestos پر نظر رکھتے ہیں۔
   - Ruta de escalada `docs/portal/docs/devportal/incident-runbooks.md` کور کرتا ہے اگر abuso de sandbox ہو؛
     `scripts/tryit-proxy-rollback.mjs` کے ساتھ combinar کریں تاکہ puntos finales محفوظ طریقے سے switch ہوں۔

## Pen-test y lista de verificación de liberación

ہر promoción de vista previa کے لئے یہ فہرست مکمل کریں (resultados کو boleto de lanzamiento کے ساتھ adjuntar کریں):1. **Verificación del cableado de OAuth کریں**
   - `npm run start` کو producción `DOCS_OAUTH_*` exportaciones کے ساتھ local چلائیں۔
   - صاف perfil del navegador سے Pruébelo consola کھولیں اور confirmar کریں کہ token de flujo de código de dispositivo mint کرتا ہے،
     cuenta regresiva de por vida کرتا ہے، اور caducidad یا cerrar sesión کے بعد borrar campo کرتا ہے۔
2. **Sonda proxy کریں**
   - `npm run tryit-proxy` puesta en escena Torii کے خلاف چلائیں، پھر
     `npm run probe:tryit-proxy` ruta de muestra configurada کے ساتھ چلائیں۔
   - registros میں `authSource=override` entradas دیکھیں اور confirmar کریں کہ ventana límite de velocidad excede ہونے پر
     los contadores aumentan ہوتے ہیں۔
3. **CSP/Tipos de confianza confirman کریں**
   - `npm run build` چلائیں اور `build/index.html` کھولیں۔ یقینی بنائیں کہ `` etiqueta directivas esperadas سے coincide con کرتا ہے
     اور carga de vista previa ہوتے وقت DevTools کوئی violaciones de CSP نہیں دکھاتا۔
   - `npm run probe:portal` (یا curl) سے HTML Fetch implementado کریں؛ sonda اب falla ہو جاتا ہے اگر
     `Content-Security-Policy`, `Permissions-Policy`, یا Metaetiquetas `Referrer-Policy` غائب ہوں یا
     `docusaurus.config.js` میں valores declarados سے مختلف ہوں، لہذا código de salida de revisores de gobernanza پر
     اعتماد کر سکتے ہیں بجائے salida de rizo دیکھنے کے۔
4. **Revisión de observabilidad کریں**
   - Pruébelo en el panel proxy سبز ہو (límites de velocidad, índices de error, métricas de sondeo de estado) ۔
   - Host بدلا ہو (despliegue de Netlify/SoraFS) y `docs/portal/docs/devportal/incident-runbooks.md`
     Simulacro de incidente میں چلائیں۔5. **نتائج documento کریں**
   - capturas de pantalla/registros کو ticket de lanzamiento کے ساتھ adjuntar کریں۔
   - ہر encontrar کو plantilla de informe de remediación میں capturar کریں
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     تاکہ propietarios, SLA, اور volver a probar evidencia بعد میں آسانی سے auditoría ہو سکے۔
   - Lista de verificación کو enlace کریں تاکہ Elemento de la hoja de ruta DOCS-1b auditable رہے۔

اگر کوئی قدم fail ہو جائے تو promoción روک دیں، ایک problema de bloqueo فائل کریں، اور `status.md` میں plan de remediación نوٹ کریں۔