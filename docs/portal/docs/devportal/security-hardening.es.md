---
lang: es
direction: ltr
source: docs/portal/docs/devportal/security-hardening.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 703ab06de69620a6930c5aae80a1fa310010a938b4db2b366003ea0ddd62ae8a
source_last_modified: "2025-12-19T22:35:27+00:00"
translation_last_reviewed: 2026-01-01
---

# Hardening de seguridad y checklist de pen-test

## Resumen

El item del roadmap **DOCS-1b** requiere login OAuth con device-code, politicas de seguridad de contenido fuertes y
pen tests repetibles antes de que el portal de preview pueda correr en redes fuera del laboratorio. Este apendice
explica el modelo de amenazas, los controles implementados en el repo y el checklist de go-live que deben ejecutar
las revisiones de gate.

- **Alcance:** el proxy de Try it, paneles Swagger/RapiDoc embebidos y la consola Try it custom renderizada por
  `docs/portal/src/components/TryItConsole.jsx`.
- **Fuera de alcance:** Torii en si (cubierto por reviews de readiness de Torii) y el publishing de SoraFS
  (cubierto por DOCS-3/7).

## Modelo de amenazas

| Activo | Riesgo | Mitigacion |
| --- | --- | --- |
| Tokens bearer de Torii | Robo o reuso fuera del sandbox de docs | El login device-code (`DOCS_OAUTH_*`) emite tokens de corta vida, el proxy redacta headers y la consola expira credenciales cacheadas automaticamente. |
| Proxy de Try it | Abuso como relay abierto o bypass de limites de Torii | `scripts/tryit-proxy*.mjs` aplica allowlists de origen, rate limiting, health probes y forwarding explicito de `X-TryIt-Auth`; no se persisten credenciales. |
| Runtime del portal | Cross-site scripting o embeds maliciosos | `docusaurus.config.js` inyecta headers Content-Security-Policy, Trusted Types y Permissions-Policy; los scripts inline se limitan al runtime de Docusaurus. |
| Datos de observabilidad | Telemetria faltante o manipulacion | `docs/portal/docs/devportal/observability.md` documenta los probes/dashboards; `scripts/portal-probe.mjs` corre en CI antes de publicar. |

Adversarios incluyen usuarios curiosos viendo el preview publico, actores maliciosos probando links robados y
browsers comprometidos que intentan extraer credenciales almacenadas. Todos los controles deben funcionar en
browsers de uso comun sin redes confiables.

## Controles requeridos

1. **OAuth device-code login**
   - Configura `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` y knobs relacionados en el entorno de build.
   - La tarjeta Try it renderiza un widget de sign-in (`OAuthDeviceLogin.jsx`) que
     obtiene el device code, hace polling al token endpoint y auto-limpia tokens
     una vez que expiran. Las overrides manuales de Bearer siguen disponibles para
     fallback de emergencia.
   - Los builds ahora fallan cuando falta la configuracion OAuth o cuando los TTLs de
     fallback se salen de la ventana 300-900 s exigida por DOCS-1b; ajusta
     `DOCS_OAUTH_ALLOW_INSECURE=1` solo para previews locales descartables.
2. **Guardrails del proxy**
   - `scripts/tryit-proxy.mjs` aplica allowed origins, rate limits, caps de tamanio de request
     y timeouts upstream mientras etiqueta el trafico con `X-TryIt-Client` y redacta tokens
     de los logs.
   - `scripts/tryit-proxy-probe.mjs` mas `docs/portal/docs/devportal/observability.md`
     definen el liveness probe y las reglas de dashboard; ejecutalos antes de cada
     rollout.
3. **CSP, Trusted Types, Permissions-Policy**
   - `docusaurus.config.js` ahora exporta headers de seguridad deterministas:
     `Content-Security-Policy` (default-src self, listas estrictas de connect/img/script,
     requisitos de Trusted Types), `Permissions-Policy` y
     `Referrer-Policy: no-referrer`.
   - La lista de connect de CSP permite los endpoints OAuth device-code y token
     (solo HTTPS a menos que `DOCS_SECURITY_ALLOW_INSECURE=1`) para que el login de device
     funcione sin relajar el sandbox para otros origenes.
   - Los headers se incrustan directamente en el HTML generado, por lo que los hosts
     estaticos no necesitan configuracion extra. Mantener los scripts inline
     limitados al bootstrap de Docusaurus.
4. **Runbooks, observabilidad y rollback**
   - `docs/portal/docs/devportal/observability.md` describe los probes y dashboards que
     vigilan fallas de login, codigos de respuesta del proxy y budgets de requests.
   - `docs/portal/docs/devportal/incident-runbooks.md` cubre la ruta de escalamiento
     si el sandbox es abusado; combinelo con
     `scripts/tryit-proxy-rollback.mjs` para cambiar endpoints de forma segura.

## Checklist de pen-test y release

Completa esta lista para cada promocion de preview (adjunta resultados al ticket de release):

1. **Verificar wiring OAuth**
   - Ejecuta `npm run start` localmente con los exports `DOCS_OAUTH_*` de produccion.
   - Desde un perfil de browser limpio, abre la consola Try it y confirma que el flujo
     device-code emite un token, cuenta regresivamente el lifetime y limpia el campo
     despues de expirar o cerrar sesion.
2. **Probar el proxy**
   - `npm run tryit-proxy` contra Torii staging, luego ejecuta
     `npm run probe:tryit-proxy` con el sample path configurado.
   - Revisa logs por entradas `authSource=override` y confirma que el rate limiting
     incrementa counters cuando excedes la ventana.
3. **Confirmar CSP/Trusted Types**
   - `npm run build` y abre `build/index.html`. Asegura que el tag `<meta
     http-equiv="Content-Security-Policy">` coincide con las directivas esperadas
     y que DevTools no muestra violaciones CSP al cargar el preview.
   - Usa `npm run probe:portal` (o curl) para obtener el HTML desplegado; el probe
     ahora falla cuando los meta tags `Content-Security-Policy`, `Permissions-Policy` o
     `Referrer-Policy` faltan o difieren de los valores declarados en
     `docusaurus.config.js`, asi que los revisores de gobernanza pueden confiar en el
     exit code en lugar de inspeccionar output de curl.
4. **Revisar observabilidad**
   - Verifica que el dashboard de Try it proxy esta verde (rate limits, error ratios,
     metricas de health probe).
   - Ejecuta el drill de incidentes en `docs/portal/docs/devportal/incident-runbooks.md`
     si cambio el host (nuevo despliegue Netlify/SoraFS).
5. **Documentar los resultados**
   - Adjunta screenshots/logs al ticket de release.
   - Captura cada hallazgo en la plantilla de reporte de remediacion
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     para que owners, SLAs y evidencia de retest sean faciles de auditar despues.
   - Enlaza de vuelta a este checklist para que el item del roadmap DOCS-1b siga auditable.

Si algun paso falla, detene la promocion, abre una issue bloqueante y anota el plan de remediacion en `status.md`.
