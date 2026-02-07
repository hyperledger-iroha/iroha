---
lang: es
direction: ltr
source: docs/portal/docs/devportal/security-hardening.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Усиление безопасности и чеклист пен-теста

## Objeto

Пункт дорожной карты **DOCS-1b** требует OAuth inicio de sesión con código de dispositivo, сильных политик
Contenido completo y pruebas de penetración anteriores antes del portal de vista previa de чем
работать в сетях вне лаборатории. Esta aplicación muestra un modelo de uso realista
En los controles de repositorios y en el registro de puesta en marcha, muchas personas pueden realizar revisiones de puerta.

- **В рамках:** aplicaciones Pruébelo, paneles integrados Swagger/RapiDoc y archivos
  консоль Pruébelo, рендеримая `docs/portal/src/components/TryItConsole.jsx`.
- **Вне рамок:** сам Torii (покрывается Torii revisiones de preparación) y publicación SoraFS
  (покрывается DOCS-3/7).

## Модель угроз| Activo | Riesgo | Mitigación |
| --- | --- | --- |
| Torii tokens portadores | Ampliación o aplicación avanzada de documentos sandbox | inicio de sesión con código de dispositivo (`DOCS_OAUTH_*`) креды. |
| Pruébalo прокси | Ajustes de la configuración o límites diferentes Torii | `scripts/tryit-proxy*.mjs` impuso listas permitidas de origen, limitación de velocidad, sondas de estado y reenvío de явный `X-TryIt-Auth`; креды не сохраняются. |
| Portal de tiempo de ejecución | Secuencias de comandos entre sitios y incrustaciones masivas | `docusaurus.config.js` contiene los encabezados Política de seguridad de contenido, Tipos confiables y Política de permisos; scripts en línea ограничены runtime Docusaurus. |
| Данные наблюдаемости | Telemetros y podmas antiguos | `docs/portal/docs/devportal/observability.md` sondas de documentación/tableros de instrumentos; `scripts/portal-probe.mjs` se publicó en CI antes de la publicación. |

Противники включают любопытных пользователей публичного vista previa, злоумышленников, тестирующих украденные ссылки,
и скомпрометированные браузеры, пытающиеся вытащить сохраненные креды. Все контроли должны работать
в обычных браузерах без доверенных сетей.

## Требуемые контроли1. **Inicio de sesión con código de dispositivo OAuth**
   - Introduzca `DOCS_OAUTH_DEVICE_CODE_URL`, `DOCS_OAUTH_TOKEN_URL`,
     `DOCS_OAUTH_CLIENT_ID` и связанные perillas в build окружении.
   - Tarjeta Pruébelo рендерит widget de inicio de sesión (`OAuthDeviceLogin.jsx`), который
     consultar el código del dispositivo, operar el punto final del token y activar automáticamente los tokens
     после истечения. Ручные Bearer anula las opciones de respaldo del sistema.
   - Actualizar archivos, configurar configuraciones de OAuth o TTL de reserva
     выходят за окно 300-900 s, previamente DOCS-1b; установите
     `DOCS_OAUTH_ALLOW_INSECURE=1` esta es una vista previa local.
2. **Barandillas proxy**
   - `scripts/tryit-proxy.mjs` muestra orígenes permitidos, límites de tasas y topes de configuración
     y tiempos de espera ascendentes, el nuevo tráfico `X-TryIt-Client` y la redacción de tokens
     в логах.
   - `scripts/tryit-proxy-probe.mjs` más `docs/portal/docs/devportal/observability.md`
     определяют sonda de vida y правила tablero; запускайте их перед каждым lanzamiento.
3. **CSP, tipos confiables, política de permisos**
   - `docusaurus.config.js` contiene encabezados de seguridad determinados:
     `Content-Security-Policy` (predeterminado-src self, строгие списки connect/img/script,
     tres tipos de confianza), `Permissions-Policy`, y
     `Referrer-Policy: no-referrer`.
   - Conexión a la lista blanca de CSP con código de dispositivo OAuth y puntos finales de token
     (только HTTPS, если не задан `DOCS_SECURITY_ALLOW_INSECURE=1`), чтобы inicio de sesión del dispositivo
     работал без ослабления sandbox для других origins.- Estos encabezados están integrados en HTML, archivos estáticos
     не требуют дополнительной конфигурации. Держите scripts en línea ограниченными
     Arranque Docusaurus.
4. **Runbooks, observabilidad y reversión**
   - `docs/portal/docs/devportal/observability.md` Descripción de sondas y tableros, colores.
     Solución de errores de inicio de sesión, códigos de respuesta de proxy y solicitudes de presupuestos.
   - `docs/portal/docs/devportal/incident-runbooks.md` описывает путь эскалации
     при злоупотреблении sandbox; combinación
     `scripts/tryit-proxy-rollback.mjs` para puntos finales de configuración básica.

## Чеклист пен-теста и релиза

Utilice esta sección para la vista previa del producto (principalmente los resultados del ticket de lanzamiento):1. **Mostrar cableado de OAuth**
   - Запустите `npm run start` локально с producción `DOCS_OAUTH_*` exportaciones.
   - Este perfil de usuario está cerrado. Pruébelo en la consola y siga el flujo de código del dispositivo.
     выдает токен, считает время жизни y очищает поле после истечения о cerrar sesión.
2. **Пробить прокси**
   - `npm run tryit-proxy` против staging Torii, затем выполните
     `npm run probe:tryit-proxy` con la ruta de muestra actual.
   - Pruebe el registro en `authSource=override` y siga cómo limitar la velocidad
     увеличивает счетчики при превышении окна.
3. **Consultar CSP/Tipos de confianza**
   - `npm run build` y cierre `build/index.html`. Убедитесь, что тег `` соответствует ожидаемым директивам
     Y DevTools no detecta violaciones de CSP en la vista previa de la descarga.
   - Utilice `npm run probe:portal` (o curl), para activar HTML actualizado; sonda
     теперь падает, если metaetiquetas `Content-Security-Policy`, `Permissions-Policy` o
     `Referrer-Policy` отсутствуют или отличаются от значений в
     `docusaurus.config.js`, la gobernanza de los revisores puede ser aceptada
     código de salida вместо ручной проверки salida curl.
4. **Provertir la observabilidad**
   - Убедитесь, что panel Pruébelo proxy зеленый (límites de velocidad, tasas de error,
     métricas de la sonda de salud).
   - Запустите simulacro de incidente из `docs/portal/docs/devportal/incident-runbooks.md`,
     если хост изменился (nueva actualización Netlify/SoraFS).5. **Mostrar resultados**
   - Registre capturas de pantalla/registros de lanzamiento.
   - Зафиксируйте каждую находку в шаблоне informe de remediación
     ([`docs/examples/pentest_remediation_report_template.md`](../../../examples/pentest_remediation_report_template.md))
     чтобы propietarios, SLA y evidencia para volver a realizar la prueba было легко аудировать позже.
   - Coloque en este registro los puntos correspondientes a los documentos DOCS-1b que son auditables.

Si no lo hace, instale el producto, elimine el problema de bloqueo y solucione el plan de corrección en `status.md`.