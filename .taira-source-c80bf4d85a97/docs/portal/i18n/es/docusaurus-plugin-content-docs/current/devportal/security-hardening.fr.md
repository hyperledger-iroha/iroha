---
lang: es
direction: ltr
source: docs/portal/docs/devportal/security-hardening.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Durcissement securite et checklist de pen-test

## Vista del conjunto

El elemento de hoja de ruta **DOCS-1b** requiere un código de dispositivo OAuth de inicio de sesión, políticas de seguridad de contenido fuertes
et des tests d'intrusion repetables avant que le portail previa vista previa puisse tourner sur des reseaux fuera de laboratorio.
Este apéndice explica el modelo de amenaza, los controles implementados en el repositorio y la lista de verificación de puesta en marcha.
que les gate revisa el ejecutor doivent.

- **Perímetro:** el proxy Pruébalo, los paneles Swagger/RapiDoc embarques y la consola Pruébalo personalizado
  rendir por `docs/portal/src/components/TryItConsole.jsx`.
- **Hors perimetre:** Torii lui-meme (cubierto por las revisiones de preparación Torii) y la publicación SoraFS
  (cubierta según DOCS-3/7).

## Modelo de amenaza| Activo | Riesgoso | Mitigación |
| --- | --- | --- |
| Portador de jetones Torii | Volumen y reutilización de documentos fuera del sandbox | El código del dispositivo de inicio de sesión (`DOCS_OAUTH_*`) activa los registros, el proxy redacta los encabezados y la consola caduca automáticamente las cachés de credenciales. |
| Proxy Pruébalo | Abus comme relais ouvert ou contournement des limites Torii | `scripts/tryit-proxy*.mjs` impone las listas permitidas de origen, la limitación de velocidad, las pruebas de seguridad y el reenvío explícito de `X-TryIt-Auth`; Esta credencial no es persistente. |
| Tiempo de ejecución del portal | Secuencias de comandos entre sitios o incrustaciones maliciosas | `docusaurus.config.js` inyecta los encabezados Content-Security-Policy, Trusted Types y Permissions-Policy; Los scripts en línea tienen limitaciones en el tiempo de ejecución Docusaurus. |
| Donnees d'observabilite | Manufactura telemétrica o falsificación | `docs/portal/docs/devportal/observability.md` documenta las sondas/tableros; `scripts/portal-probe.mjs` gira en CI antes de la publicación. |

Les adversaires incluent des utilisateurs curieux qui consultent le preview public, des acteurs malveillants
Testant des gravámenes voles et des navegantes compromete qui tentent d'exfiltrer des credenciales stockes. Todos
Los controles deben funcionar en los estándares del navegador sin fuentes de confianza.

## Controles requeridos1. **Inicio de sesión con código de dispositivo OAuth**
   - Configurador `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID`, y otras perillas en el entorno de construcción.
   - La carta Pruébelo y obtenga un widget de inicio de sesión (`OAuthDeviceLogin.jsx`) aquí
     recuperar el código del dispositivo, interrogar el punto final del token y borrar automáticamente
     les jetons a expiración. Les overrides manuels de Bearer restent disponibles
     para un respaldo de urgencia.
   - Las compilaciones se mantienen mantenidas si la configuración OAuth se mantiene o si los TTL de
     tipo de respaldo de la ventana 300-900 s impuesto por DOCS-1b; definir
     `DOCS_OAUTH_ALLOW_INSECURE=1` único para vistas previas de lugares jetables.
2. **Garde-fous du proxy**
   - `scripts/tryit-proxy.mjs` apliques de origen autorizado, límites de tarifas,
     Plafones de taille de requete et des timeouts upstream tout en taguant le
     trafic avec `X-TryIt-Client` et en redactant les jetons des logs.
   - `scripts/tryit-proxy-probe.mjs` más `docs/portal/docs/devportal/observability.md`
     define la sonda de vida y las reglas del tablero; ejecutar-les
     lanzamiento de avant chaque.
3. **CSP, tipos confiables, política de permisos**
   - `docusaurus.config.js` Exportar mantenimiento de encabezados de seguridad deterministas:
     `Content-Security-Policy` (src self predeterminado, lista estricta connect/img/script,
     exigencias de tipos confiables), `Permissions-Policy`, et
     `Referrer-Policy: no-referrer`.
   - La lista de conexión de CSP a la lista blanca de puntos finales, código y token de dispositivo OAuth(HTTPS único en `DOCS_SECURITY_ALLOW_INSECURE=1`) después de iniciar sesión en el dispositivo
     Función sin relacher le sandbox para otros orígenes.
   - Los encabezados se embarcan directamente en el género HTML, en los hosts
     Las estadísticas no pasan por la configuración complementaria. Guarde los guiones
     Limites en línea del bootstrap Docusaurus.
4. **Runbooks, observabilidad y reversión**
   - `docs/portal/docs/devportal/observability.md` decrit les probes et Dashboards qui
     Vigile los controles de inicio de sesión, los códigos de respuesta del proxy y los presupuestos.
     de solicitudes.
   - `docs/portal/docs/devportal/incident-runbooks.md` cubierta de escalada
     si le sandbox es abuso; combinez-le avec
     `scripts/tryit-proxy-rollback.mjs` para bloquear los puntos finales de forma segura.

## Lista de verificación de pen-test y liberación

Complete esta lista para cada promoción de vista previa (únase a los resultados del boleto de lanzamiento):1. **Verificador de cableado de archivos OAuth**
   - Ejecutor `npm run start` localment avec les exports `DOCS_OAUTH_*` de producción.
   - Después de un perfil de navegador propio, abra la consola Pruébelo y confirme que le
     código de dispositivo de flujo emet un jeton, compte la duree de vie et efface le champ apres
     caducidad o cierre de sesión.
2. **Sonder le proxy**
   - `npm run tryit-proxy` contra Torii puesta en escena, después del ejecutor
     `npm run probe:tryit-proxy` con configuración de ruta de muestra.
   - Verifique los registros para `authSource=override` y confirme la limitación de velocidad
     incremente los ordenadores cuando la ventana está apagada.
3. **CSP de confirmación/tipos de confianza**
   - `npm run build` luego abra `build/index.html`. Verificador que la balise `` corresponden a las directivas auxiliares asistentes
     Y DevTools no detecta ninguna infracción de CSP al cargar la vista previa.
   - Utilice `npm run probe:portal` (o curl) para recuperar la implementación HTML; la sonda
     echoue maintenant si las metaetiquetas `Content-Security-Policy`, `Permissions-Policy` o
     `Referrer-Policy` manquent ou divergent des valeurs declarees dans
     `docusaurus.config.js`, donc les revisores gouvernance peuvent se fier au
     code de sortie au lieu de lire la sortie curl.
4. **Revoir la observabilidad**
   - Verificador que le panel Pruébalo proxy est au vert (límites de tasa, ratios de error,
     métricas de sonda de salud).- Ejecutor del simulacro del incidente en `docs/portal/docs/devportal/incident-runbooks.md`
     Si aloja un cambio (nueva implementación Netlify/SoraFS).
5. **Documentar los resultados**
   - Joindre captura d'ecran/logs au ticket de liberación.
   - Capturer cada hallazgo en la plantilla de informe de remediación
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     Afin que los propietarios, SLA y preuves de retest soient facilitan un auditor más tarde.
   - Agregue esta nueva lista de verificación para que el elemento de la hoja de ruta DOCS-1b sea auditable.

Si una etapa de ecoue, detener la promoción, abrir un tema bloquante y anotar el plan de remediación
en `status.md`.