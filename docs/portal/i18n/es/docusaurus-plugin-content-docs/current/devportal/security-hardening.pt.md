---
lang: es
direction: ltr
source: docs/portal/docs/devportal/security-hardening.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Hardening de seguranca e checklist de pen-test

## Visao general

El elemento de la hoja de ruta **DOCS-1b** exige iniciar sesión en el código del dispositivo OAuth, políticas fuertes de seguridad de contacto
e testes de penetracao repetiveis antes de o portalvista previa rodar en redes fora do laboratorio. Este
Apéndice explica el modelo de ameacas, los controles implementados no repositorio y una lista de verificación de go-live que
OS Gate revisa devem ejecutar.

- **Escopo:** el proxy Pruébalo, los Paineis Swagger/RapiDoc embutidos y una consola Pruébalo personalizado renderizado por
  `docs/portal/src/components/TryItConsole.jsx`.
- **Foro do escopo:** Torii em si (coberto por reviews de readiness do Torii) e publicacao SoraFS
  (cobertado por DOCS-3/7).

## Modelo de ameacas| activo | Risco | Mitigacao |
| --- | --- | --- |
| Portador de tokens do Torii | Roubo ou reuso fora do sandbox de docs | El código del dispositivo de inicio de sesión (`DOCS_OAUTH_*`) emite tokens de corta duración, los encabezados de red del proxy y la consola expiran las credenciales en el caché automáticamente. |
| Proxy Pruébalo | Abuso como relé abierto o bypass de límites de Torii | `scripts/tryit-proxy*.mjs` aplica listas permitidas de origen, limitación de velocidad, sondas de estado y reenvío explícito de `X-TryIt-Auth`; nenhuma credencial e persistida. |
| Tiempo de ejecución del portal | Cross-site scripting o incrustaciones maliciosas | `docusaurus.config.js` encabezados de inyección Contenido-Política-de-seguridad, Tipos confiables y Política-de-permisos; scripts en línea ficam restringidos en tiempo de ejecución de Docusaurus. |
| Datos de observabilidad | Telemetria ausente o adulterada | `docs/portal/docs/devportal/observability.md` sondas/tableros de instrumentos documenta; `scripts/portal-probe.mjs` roda em CI antes de publicar. |

Adversarios incluyen usuarios curiosos vendiendo o vista previa pública, atores maliciosos testando enlaces roubados e
browsers comprometidos tentando extrair credenciales armazenadas. Todos los controles deben funcionar en
Navegadores comunes sin redes confidenciales.

## Controles requeridos1. **Inicio de sesión con código de dispositivo OAuth**
   - Configurar `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` y perillas relacionadas no ambiente de construcción.
   - Tarjeta O Pruébelo renderiza un widget de inicio de sesión (`OAuthDeviceLogin.jsx`) que
     Busca el código del dispositivo, haz sondeo sin punto final de token y limpia automáticamente
     tokens cuando expiran. Anula los manuales de portador que permanecen disponibles
     para respaldo de emergencia.
   - Os builds agora falham cuando configuracao OAuth está ausente o cuando os
     TTL de respaldo saem da janela 300-900 s exigida por DOCS-1b; ajuste
     `DOCS_OAUTH_ALLOW_INSECURE=1` sólo para vistas previas locales descartadas.
2. **Las barandillas actúan como proxy**
   - `scripts/tryit-proxy.mjs` aplica orígenes permitidos, límites de tarifas, límites de tamaño de solicitud
     e timeouts upstream enquanto marca o trafego com `X-TryIt-Client` e
     eliminar tokens dos registros.
   - `scripts/tryit-proxy-probe.mjs` más `docs/portal/docs/devportal/observability.md`
     defina una sonda de vida y registros de tablero; ejecutar-os antes de cada
     lanzamiento.
3. **CSP, tipos confiables, política de permisos**
   - `docusaurus.config.js` ahora exporta headers de seguranca deterministas:
     `Content-Security-Policy` (predeterminado-src self, listas estritas de connect/img/script,
     requisitos de Trusted Types), `Permissions-Policy`, e
     `Referrer-Policy: no-referrer`.
   - Una lista de conexiones de la lista blanca de CSP de puntos finales OAuth, código de dispositivo y token
     (solo HTTPS a menos que `DOCS_SECURITY_ALLOW_INSECURE=1`) para iniciar sesión en el dispositivoFunciona sin relajar el sandbox para otros orígenes.
   - Los encabezados están integrados directamente en HTML, pero no hay hosts estadísticos necesarios.
     configuración adicional. Mantenga scripts en línea limitados en bootstrap de Docusaurus.
4. **Runbooks, observabilidad y reversión**
   - `docs/portal/docs/devportal/observability.md` descreve las sondas y paneles de control que
     Monitoram falhas de login, codigos de respuesta do proxy y presupuestos de solicitud.
   - `docs/portal/docs/devportal/incident-runbooks.md` cobre o camino de escalada
     se o arenero para abusado; combinar com
     `scripts/tryit-proxy-rollback.mjs` para viralizar endpoints con seguridad.

## Lista de verificación de pen-test y lanzamiento

Complete esta lista para cada promoción de vista previa (anexe resultados del ticket de lanzamiento):1. **Verificar cableado OAuth**
   - Ejecute `npm run start` localmente con os exports `DOCS_OAUTH_*` de producao.
   - A partir de un perfil de navegador limpio, abra una consola Pruébelo y confirme que o
     El código del dispositivo fluxo emite un token, conta a duracao e limpia el campo apos expirar.
     Puede cerrar sesión.
2. **Provar o proxy**
   - `npm run tryit-proxy` contra Torii puesta en escena, ejecución de depósito
     `npm run probe:tryit-proxy` con la ruta de muestra configurada.
   - Verifique los registros de las entradas `authSource=override` y confirme que o rate limiting
     contadores incrementales cuando la voz excede a janela.
3. **Confirmar CSP/Tipos confiables**
   - `npm run build` y abra `build/index.html`. Garanta que a tag `` corresponden como diretivas esperadas
     Y eso es lo que DevTools muestra violando CSP y cargando la vista previa.
   - Utilice `npm run probe:portal` (o curl) para buscar el HTML implementado; una sonda
     Ahora falta cuando se utilizan metaetiquetas `Content-Security-Policy`, `Permissions-Policy` o
     `Referrer-Policy` están ausentes o difieren dos valores declarados en
     `docusaurus.config.js`, asim revisores de gobierno pueden confiar no
     código de salida em vez de inspeccionar o salida do curl.
4. **Revisar observabilidade**
   - Verifique si el panel de control del proxy Pruébelo esta verde (límites de tasa, tasas de error,
     métricas de sonda sanitaria).
   - Ejecutar o simular incidentes en `docs/portal/docs/devportal/incident-runbooks.md`se o host mudou (implementación nueva Netlify/SoraFS).
5. **Documentar resultados**
   - Capturas de pantalla/registros anexos del ticket de lanzamiento.
   - Capture cada hallazgo sin plantilla de relatorio de remediacao
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     para que propietarios, SLA y evidencia de retest sejam faceis de auditar depois.
   - Enlace de vuelta a esta lista de verificación para que el elemento de la hoja de ruta DOCS-1b continúe siendo auditable.

Se algum passo falhar, pare a promocao, abra uma issues bloqueante e anote o plano de remediacao em `status.md`.