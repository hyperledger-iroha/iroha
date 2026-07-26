---
lang: es
direction: ltr
source: docs/portal/docs/devportal/security-hardening.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Refuerzo de seguridad y checklist de pen-test

## Resumen

El elemento del roadmap **DOCS-1b** requiere iniciar sesión OAuth con código de dispositivo, políticas de seguridad de contenido fuertes y
pen tests repetibles antes de que el portal de vista previa pueda correr en redes fuera del laboratorio. Este apéndice
explica el modelo de amenazas, los controles implementados en el repositorio y el checklist de go-live que deben ejecutar
las revisión de la puerta.

- **Alcance:** el proxy de Try it, paneles Swagger/RapiDoc embebidos y la consola Try it custom renderizada por
  `docs/portal/src/components/TryItConsole.jsx`.
- **Fuera de alcance:** Torii en si (cubierto por reviews de readiness de Torii) y el Publishing de SoraFS
  (cubierto por DOCS-3/7).

## Modelo de amenazas| Activo | Riesgo | Mitigación |
| --- | --- | --- |
| Portador de tokens de Torii | Robo o reuso fuera del sandbox de docs | El código de dispositivo de inicio de sesión (`DOCS_OAUTH_*`) emite tokens de corta vida, el proxy redacta encabezados y la consola expira las credenciales cacheadas automáticamente. |
| Proxy de Pruébalo | Abuso como relé abierto o bypass de límites de Torii | `scripts/tryit-proxy*.mjs` aplica permitidos de origen, limitación de velocidad, sondas de salud y reenvío explícito de `X-TryIt-Auth`; no se persisten credenciales. |
| Tiempo de ejecución del portal | Cross-site scripting o incrustaciones maliciosas | `docusaurus.config.js` inyecta headers Contenido-Seguridad-Política, Tipos de confianza y Permisos-Política; Los scripts en línea se limitan al tiempo de ejecución de Docusaurus. |
| Datos de observabilidad | Telemetria faltante o manipulacion | `docs/portal/docs/devportal/observability.md` documenta los probes/dashboards; `scripts/portal-probe.mjs` corre en CI antes de publicar. |

Adversarios incluyen usuarios curiosos viendo el avance público, actores maliciosos probando enlaces robados y
navegadores comprometidos que intentan extraer credenciales almacenadas. Todos los controles deben funcionar en
Navegadores de uso comunes sin redes confiables.

## Controles requeridos1. **Inicio de sesión con código de dispositivo OAuth**
   - Configurar `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` y perillas relacionadas en el entorno de compilación.
   - La tarjeta Pruébalo renderiza un widget de inicio de sesión (`OAuthDeviceLogin.jsx`) que
     Obtiene el código del dispositivo, hace sondeo al punto final del token y limpia automáticamente los tokens.
     una vez que expiran. Los manuales de anulación de Bearer siguen disponibles para
     respaldo de emergencia.
   - Los builds ahora fallan cuando falta la configuración OAuth o cuando los TTLs de
     fallback se salen de la ventana 300-900 s exigida por DOCS-1b; ajusta
     `DOCS_OAUTH_ALLOW_INSECURE=1` solo para vistas previas locales descartables.
2. **Barandillas del proxy**
   - `scripts/tryit-proxy.mjs` aplica orígenes permitidos, límites de tarifas, topes de tamaño de solicitud
     y timeouts upstream mientras etiqueta el trafico con `X-TryIt-Client` y redacta tokens
     de los troncos.
   - `scripts/tryit-proxy-probe.mjs` más `docs/portal/docs/devportal/observability.md`
     definen el liveness probe y las reglas de tablero; ejecutalos antes de cada
     lanzamiento.
3. **CSP, tipos confiables, política de permisos**
   - `docusaurus.config.js` ahora exporta encabezados de seguridad deterministas:
     `Content-Security-Policy` (default-src self, listas estrictas de connect/img/script,
     requisitos de Trusted Types), `Permissions-Policy` y
     `Referrer-Policy: no-referrer`.
   - La lista de conexión de CSP permite los puntos finales OAuth código de dispositivo y token(solo HTTPS a menos que `DOCS_SECURITY_ALLOW_INSECURE=1`) para que el login del dispositivo
     funcione sin relajar el sandbox para otros orígenes.
   - Los encabezados se incrustan directamente en el HTML generado, por lo que los hosts
     estáticas no necesitan configuración adicional. Mantener los scripts en línea
     Limitados al bootstrap de Docusaurus.
4. **Runbooks, observabilidad y rollback**
   - `docs/portal/docs/devportal/observability.md` describe las sondas y paneles que
     Vigilan fallas de inicio de sesión, códigos de respuesta del proxy y presupuestos de solicitudes.
   - `docs/portal/docs/devportal/incident-runbooks.md` cubre la ruta de escalada
     si el arenero está abusado; combinarlo con
     `scripts/tryit-proxy-rollback.mjs` para cambiar los puntos finales de forma segura.

## Lista de verificación de pen-test y lanzamiento

Completa esta lista para cada promoción de vista previa (resultados adjuntos al ticket de lanzamiento):1. **Verificar cableado OAuth**
   - Ejecuta `npm run start` localmente con los exports `DOCS_OAUTH_*` de produccion.
   - Desde un perfil de navegador limpio, abre la consola Pruébalo y confirma que el flujo
     El código del dispositivo emite un token, cuenta regresivamente el tiempo de vida y limpia el campo.
     despues de expirar o cerrar sesion.
2. **Probar el proxy**
   - `npm run tryit-proxy` contra Torii puesta en escena, luego ejecuta
     `npm run probe:tryit-proxy` con la ruta de muestra configurada.
   - Revisa los registros por entradas `authSource=override` y confirma que el rate limiting
     contadores incrementales cuando excede la ventana.
3. **Confirmar CSP/Tipos confiables**
   - `npm run build` y abre `build/index.html`. Asegura que la etiqueta `` coincide con las directivas esperadas
     y que DevTools no muestra violaciones CSP al cargar la vista previa.
   - Usa `npm run probe:portal` (o curl) para obtener el HTML desplegado; la sonda
     ahora falla cuando los meta tags `Content-Security-Policy`, `Permissions-Policy` o
     `Referrer-Policy` faltan o difieren de los valores declarados en
     `docusaurus.config.js`, así que los revisores de gobernanza pueden confiar en el
     código de salida en lugar de inspeccionar la salida de curl.
4. **Revisar observabilidad**
   - Verifica que el panel de Pruébalo proxy está verde (límites de tasa, tasas de error,
     métricas de sonda sanitaria).- Ejecuta el taladro de incidentes en `docs/portal/docs/devportal/incident-runbooks.md`
     si cambia el host (nuevo despliegue Netlify/SoraFS).
5. **Documentar los resultados**
   - Adjunta capturas de pantalla/registros al ticket de lanzamiento.
   - Captura cada hallazgo en la plantilla de informe de remediacion
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     para que propietarios, SLAs y evidencia de retest sean fáciles de auditar después.
   - Enlaza de vuelta a esta lista de verificación para que el elemento de la hoja de ruta DOCS-1b siga auditable.

Si algún paso falla, detene la promoción, abre un tema bloqueante y anota el plan de remediación en `status.md`.