---
lang: es
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Consola Try-It de Norito
description: Usa el proxy del portal de desarrolladores y los widgets de Swagger y RapiDoc para enviar solicitudes reales de Torii / Norito-RPC directamente desde el sitio de documentacion.
---

El portal agrupa tres superficies interactivas que retransmiten trafico a Torii:

- **Swagger UI** en `/reference/torii-swagger` renderiza la especificacion OpenAPI firmada y reescribe automaticamente las solicitudes a traves del proxy cuando `TRYIT_PROXY_PUBLIC_URL` esta configurado.
- **RapiDoc** en `/reference/torii-rapidoc` expone el mismo esquema con cargas de archivos y selectores de tipo de contenido que funcionan bien para `application/x-norito`.
- **Try it sandbox** en la pagina de resumen de Norito ofrece un formulario ligero para solicitudes REST ad hoc e inicios de sesion OAuth por dispositivo.

Los tres widgets envian solicitudes al **proxy Try-It** local (`docs/portal/scripts/tryit-proxy.mjs`). El proxy verifica que `static/openapi/torii.json` coincida con el digest firmado en `static/openapi/manifest.json`, aplica un limitador de tasa, redacta los encabezados `X-TryIt-Auth` en los logs y etiqueta cada llamada upstream con `X-TryIt-Client` para que los operadores de Torii puedan auditar las fuentes de trafico.

## Inicia el proxy

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` es la URL base de Torii que quieres probar.
- `TRYIT_PROXY_ALLOWED_ORIGINS` debe incluir cada origen del portal (servidor local, hostname de produccion, URL de vista previa) que debe incrustar la consola.
- `TRYIT_PROXY_PUBLIC_URL` se consume en `docusaurus.config.js` y se inyecta en los widgets via `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` solo se carga cuando `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`; de lo contrario los usuarios deben proporcionar su propio token via la consola o el flujo de dispositivo OAuth.
- `TRYIT_PROXY_CLIENT_ID` establece la etiqueta `X-TryIt-Client` que se transporta en cada solicitud.
  Proporcionar `X-TryIt-Client` desde el navegador esta permitido pero los valores se recortan
  y se rechazan si contienen caracteres de control.

Al iniciar, el proxy ejecuta `verifySpecDigest` y sale con una pista de remediacion si el manifiesto esta obsoleto. Ejecuta `npm run sync-openapi -- --latest` para descargar la especificacion mas reciente de Torii o pasa `TRYIT_PROXY_ALLOW_STALE_SPEC=1` para anulaciones de emergencia.

Para actualizar o revertir el destino del proxy sin editar archivos de entorno a mano, usa el helper:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Conecta los widgets

Sirve el portal despues de que el proxy este escuchando:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` expone los siguientes ajustes:

| Variable | Proposito |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL inyectada en Swagger, RapiDoc y el sandbox Try it. Dejala sin definir para ocultar los widgets durante vistas previas no autorizadas. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Token predeterminado opcional almacenado en memoria. Requiere `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` y la guarda CSP solo-HTTPS (DOCS-1b) a menos que pases `DOCS_SECURITY_ALLOW_INSECURE=1` localmente. |
| `DOCS_OAUTH_*` | Habilita el flujo de dispositivo OAuth (`OAuthDeviceLogin` component) para que los revisores puedan acunar tokens de corta duracion sin salir del portal. |

Cuando las variables OAuth estan presentes, el sandbox renderiza un boton **Sign in with device code** que recorre el servidor de Auth configurado (ver `config/security-helpers.js` para la forma exacta). Los tokens emitidos a traves del flujo de dispositivo solo se almacenan en cache en la sesion del navegador.

## Enviar payloads Norito-RPC

1. Construye un payload `.norito` con el CLI o los fragmentos descritos en el [inicio rapido de Norito](./quickstart.md). El proxy reenvia cuerpos `application/x-norito` sin cambios, asi que puedes reutilizar el mismo artefacto que enviarias con `curl`.
2. Abre `/reference/torii-rapidoc` (preferido para payloads binarios) o `/reference/torii-swagger`.
3. Selecciona el snapshot de Torii deseado del desplegable. Los snapshots estan firmados; el panel muestra el digest del manifiesto registrado en `static/openapi/manifest.json`.
4. Elige el tipo de contenido `application/x-norito` en el cajon "Try it", haz clic en **Choose File**, y selecciona tu payload. El proxy reescribe la solicitud a `/proxy/v1/pipeline/submit` y la etiqueta con `X-TryIt-Client=docs-portal-rapidoc`.
5. Para descargar respuestas Norito, configura `Accept: application/x-norito`. Swagger/RapiDoc exponen el selector de headers en el mismo cajon y transmiten el binario de vuelta a traves del proxy.

Para rutas solo JSON, el sandbox Try it embebido suele ser mas rapido: introduce la ruta (por ejemplo, `/v1/accounts/i105.../assets`), selecciona el metodo HTTP, pega un cuerpo JSON cuando sea necesario y pulsa **Send request** para inspeccionar headers, duracion y payloads en linea.

## Solucion de problemas

| Sintoma | Causa probable | Remediacion |
| --- | --- | --- |
| La consola del navegador muestra errores CORS o el sandbox advierte que falta la URL del proxy. | El proxy no esta en ejecucion o el origen no esta en la lista permitida. | Inicia el proxy, asegurate de que `TRYIT_PROXY_ALLOWED_ORIGINS` cubra tu host del portal y reinicia `npm run start`. |
| `npm run tryit-proxy` sale con "digest mismatch". | El bundle OpenAPI de Torii cambio upstream. | Ejecuta `npm run sync-openapi -- --latest` (o `--version=<tag>`) y reintenta. |
| Los widgets devuelven `401` o `403`. | Token ausente, caducado o con permisos insuficientes. | Usa el flujo de dispositivo OAuth o pega un bearer token valido en el sandbox. Para tokens estaticos debes exportar `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` desde el proxy. | Se excedio el limite de tasa por IP. | Aumenta `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` para entornos de confianza o limita scripts de prueba. Todos los rechazos por rate limit incrementan `tryit_proxy_rate_limited_total`. |

## Observabilidad

- `npm run probe:tryit-proxy` (wrapper alrededor de `scripts/tryit-proxy-probe.mjs`) llama a `/healthz`, opcionalmente prueba una ruta de ejemplo y emite textfiles de Prometheus para `probe_success` / `probe_duration_seconds`. Configura `TRYIT_PROXY_PROBE_METRICS_FILE` para integrarlo con node_exporter.
- Configura `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` para exponer contadores (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) e histogramas de latencia. El panel `dashboards/grafana/docs_portal.json` lee estas metricas para aplicar SLOs DOCS-SORA.
- Los logs de runtime viven en stdout. Cada entrada incluye el id de solicitud, el estado upstream, la fuente de autenticacion (`default`, `override` o `client`), y la duracion; los secretos se redactan antes de emitirlos.

Si necesitas validar que los payloads `application/x-norito` llegan a Torii sin cambios, ejecuta la suite de Jest (`npm test -- tryit-proxy`) o inspecciona los fixtures en `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. Las pruebas de regresion cubren binarios Norito comprimidos, manifiestos OpenAPI firmados y rutas de downgrade del proxy para que los despliegues NRPC mantengan un rastro de evidencia permanente.
