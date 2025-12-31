<!-- Auto-generated stub for Spanish (es) translation. Replace this content with the full translation. -->

---
lang: es
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Consola Try-It de Norito
description: Usa el proxy del portal de desarrolladores y los widgets de Swagger y RapiDoc para enviar solicitudes reales de Torii / Norito-RPC directamente desde el sitio de documentación.
---

El portal agrupa tres superficies interactivas que retransmiten tráfico a Torii:

- **Swagger UI** en `/reference/torii-swagger` renderiza la especificación OpenAPI firmada y reescribe automáticamente las solicitudes a través del proxy cuando `TRYIT_PROXY_PUBLIC_URL` está configurado.
- **RapiDoc** en `/reference/torii-rapidoc` expone el mismo esquema con cargas de archivos y selectores de tipo de contenido que funcionan bien para `application/x-norito`.
- **Try it sandbox** en la página de resumen de Norito ofrece un formulario ligero para solicitudes REST ad hoc e inicios de sesión OAuth por dispositivo.

Los tres widgets envían solicitudes al **proxy Try-It** local (`docs/portal/scripts/tryit-proxy.mjs`). El proxy verifica que `static/openapi/torii.json` coincida con el digest firmado en `static/openapi/manifest.json`, aplica un limitador de tasa, redacta los encabezados `X-TryIt-Auth` en los logs y etiqueta cada llamada upstream con `X-TryIt-Client` para que los operadores de Torii puedan auditar las fuentes de tráfico.

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
- `TRYIT_PROXY_ALLOWED_ORIGINS` debe incluir cada origen del portal (servidor local, hostname de producción, URL de vista previa) que debe incrustar la consola.
- `TRYIT_PROXY_PUBLIC_URL` se consume en `docusaurus.config.js` y se inyecta en los widgets vía `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` solo se carga cuando `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`; de lo contrario los usuarios deben proporcionar su propio token vía la consola o el flujo de dispositivo OAuth.
- `TRYIT_PROXY_CLIENT_ID` establece la etiqueta `X-TryIt-Client` que se transporta en cada solicitud.
  Proporcionar `X-TryIt-Client` desde el navegador está permitido pero los valores se recortan
  y se rechazan si contienen caracteres de control.

Al iniciar, el proxy ejecuta `verifySpecDigest` y sale con una pista de remediación si el manifiesto está obsoleto. Ejecuta `npm run sync-openapi -- --latest` para descargar la especificación más reciente de Torii o pasa `TRYIT_PROXY_ALLOW_STALE_SPEC=1` para anulaciones de emergencia.

Para actualizar o revertir el destino del proxy sin editar archivos de entorno a mano, usa el helper:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Conecta los widgets

Sirve el portal después de que el proxy esté escuchando:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` expone los siguientes ajustes:

| Variable | Propósito |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL inyectada en Swagger, RapiDoc y el sandbox Try it. Déjala sin definir para ocultar los widgets durante vistas previas no autorizadas. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Token predeterminado opcional almacenado en memoria. Requiere `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` y la guarda CSP solo-HTTPS (DOCS-1b) a menos que pases `DOCS_SECURITY_ALLOW_INSECURE=1` localmente. |
| `DOCS_OAUTH_*` | Habilita el flujo de dispositivo OAuth (`OAuthDeviceLogin` component) para que los revisores puedan acuñar tokens de corta duración sin salir del portal. |

Cuando las variables OAuth están presentes, el sandbox renderiza un botón **Sign in with device code** que recorre el servidor de Auth configurado (ver `config/security-helpers.js` para la forma exacta). Los tokens emitidos a través del flujo de dispositivo solo se almacenan en caché en la sesión del navegador.

## Enviar payloads Norito-RPC

1. Construye un payload `.norito` con el CLI o los fragmentos descritos en el [inicio rápido de Norito](./quickstart.md). El proxy reenvía cuerpos `application/x-norito` sin cambios, así que puedes reutilizar el mismo artefacto que enviarías con `curl`.
2. Abre `/reference/torii-rapidoc` (preferido para payloads binarios) o `/reference/torii-swagger`.
3. Selecciona el snapshot de Torii deseado del desplegable. Los snapshots están firmados; el panel muestra el digest del manifiesto registrado en `static/openapi/manifest.json`.
4. Elige el tipo de contenido `application/x-norito` en el cajón "Try it", haz clic en **Choose File**, y selecciona tu payload. El proxy reescribe la solicitud a `/proxy/v1/pipeline/submit` y la etiqueta con `X-TryIt-Client=docs-portal-rapidoc`.
5. Para descargar respuestas Norito, configura `Accept: application/x-norito`. Swagger/RapiDoc exponen el selector de headers en el mismo cajón y transmiten el binario de vuelta a través del proxy.

Para rutas solo JSON, el sandbox Try it embebido suele ser más rápido: introduce la ruta (por ejemplo, `/v1/accounts/ih58@wonderland/assets`), selecciona el método HTTP, pega un cuerpo JSON cuando sea necesario y pulsa **Send request** para inspeccionar headers, duración y payloads en línea.

## Solución de problemas

| Síntoma | Causa probable | Remediación |
| --- | --- | --- |
| La consola del navegador muestra errores CORS o el sandbox advierte que falta la URL del proxy. | El proxy no está en ejecución o el origen no está en la lista permitida. | Inicia el proxy, asegúrate de que `TRYIT_PROXY_ALLOWED_ORIGINS` cubra tu host del portal y reinicia `npm run start`. |
| `npm run tryit-proxy` sale con “digest mismatch”. | El bundle OpenAPI de Torii cambió upstream. | Ejecuta `npm run sync-openapi -- --latest` (o `--version=<tag>`) y reintenta. |
| Los widgets devuelven `401` o `403`. | Token ausente, caducado o con permisos insuficientes. | Usa el flujo de dispositivo OAuth o pega un bearer token válido en el sandbox. Para tokens estáticos debes exportar `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` desde el proxy. | Se excedió el límite de tasa por IP. | Aumenta `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` para entornos de confianza o limita scripts de prueba. Todos los rechazos por rate limit incrementan `tryit_proxy_rate_limited_total`. |
| Errores `502/504` con `ERR_STRICT_ADDRESS_REQUIRED` en los logs de Torii. | Solicitudes reenviadas sin soporte de parseo IH58/comprimido de Norito. | Confirma que el build objetivo de Torii incluye los cambios ADDR-5 (ver `crates/iroha_torii/tests/address_parsing.rs`) y que apuntas al entorno correcto. |

## Observabilidad

- `npm run probe:tryit-proxy` (wrapper alrededor de `scripts/tryit-proxy-probe.mjs`) llama a `/healthz`, opcionalmente prueba una ruta de ejemplo y emite textfiles de Prometheus para `probe_success` / `probe_duration_seconds`. Configura `TRYIT_PROXY_PROBE_METRICS_FILE` para integrarlo con node_exporter.
- Configura `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` para exponer contadores (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) e histogramas de latencia. El panel `dashboards/grafana/docs_portal.json` lee estas métricas para aplicar SLOs DOCS-SORA.
- Los logs de runtime viven en stdout. Cada entrada incluye el id de solicitud, el estado upstream, la fuente de autenticación (`default`, `override` o `client`), y la duración; los secretos se redactan antes de emitirlos.

Si necesitas validar que los payloads `application/x-norito` llegan a Torii sin cambios, ejecuta la suite de Jest (`npm test -- tryit-proxy`) o inspecciona los fixtures en `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. Las pruebas de regresión cubren binarios Norito comprimidos, manifiestos OpenAPI firmados y rutas de downgrade del proxy para que los despliegues NRPC mantengan un rastro de evidencia permanente.
