---
lang: es
direction: ltr
source: docs/portal/docs/devportal/try-it.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox de Pruébalo

El portal de desarrolladores incluye una consola opcional "Pruébelo" para que pueda llamar a los puntos finales de Torii sin salir de la documentación. La consola retransmite solicitudes a través del proxy incluido para que los navegadores puedan evitar los límites CORS mientras se siguen aplicando límites de tasa y autenticación.

##Requisitos previos

- Node.js 18.18 o más nuevo (coincide con los requisitos de compilación del portal)
- Acceso de red a un entorno de staging de Torii
- Un token al portador que pueda llamar las rutas Torii que planeas ejercitar

Toda la configuración del proxy se realiza mediante variables de entorno. La tabla siguiente lista los botones más importantes:| Variables | propuesta | Predeterminado |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL base de Torii a la que el proxy reenvia solicitudes | **Obligatorio** |
| `TRYIT_PROXY_LISTEN` | Dirección de escucha para desarrollo local (formato `host:port` o `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por comas de origen que pueden llamar al proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador colocado en `X-TryIt-Client` para cada solicitud upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token por defecto reenviado a Torii | _vacío_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permite a los usuarios proporcionar su propio token vía `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamano maximo del cuerpo de solicitud (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout upstream en milisegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Solicitudes permitidas por ventana de tasa por IP de cliente | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Ventana deslizante para limitación de velocidad (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Dirección de escucha opcional para el endpoint de métricas estilo Prometheus (`host:port` o `[ipv6]:port`) | _vacío (deshabilitado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Ruta HTTP servida por el endpoint de métricas | `/metrics` |

El proxy también exponen `GET /healthz`, devuelve errores JSON estructurados y tokens portadores ocultos de la salida de logs.Activa `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` al exponer el proxy a usuarios de docs para que los paneles Swagger y RapiDoc puedan reenviar los tokens de portador proporcionados por el usuario. El proxy aún aplica límites de tasa, credenciales ocultas y registro si una solicitud utiliza el token por defecto o una anulación por solicitud. Configura `TRYIT_PROXY_CLIENT_ID` con la etiqueta que quieres enviar como `X-TryIt-Client`
(por defecto `docs-portal`). El proxy recorta y valida valores `X-TryIt-Client` aportados por el cliente, volviendo a este default para que las gateways de staging puedan auditar la procedencia sin correlacionar metadatos del navegador.

## Inicia el proxy localmente

Instale dependencias la primera vez que configura el portal:

```bash
cd docs/portal
npm install
```

Ejecuta el proxy y apuntalo a tu instancia Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

El script registra la dirección enlazada y reenvía solicitudes desde `/proxy/*` al origen Torii configurado.

Antes de enlazar el socket el script valida que
`static/openapi/torii.json` coincide con el resumen registrado en
`static/openapi/manifest.json`. Si los archivos se desincronizan, el comando termina con un
error y te indica ejecutar `npm run sync-openapi -- --latest`. exporta
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` solo para anulación de emergencia; el proxy registrador una
Advertencia y continuará para que puedas recuperarte durante las ventanas de mantenimiento.

## Conecta los widgets del portalCuando haces build o sirves el portal de desarrolladores, define la URL que los widgets
deben usar para el proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Los siguientes componentes leen estos valores desde `docusaurus.config.js`:

- **Swagger UI** - renderizado en `/reference/torii-swagger`; preautoriza el esquema
  portador cuando hay un token, etiqueta las solicitudes con `X-TryIt-Client`,
  inyecta `X-TryIt-Auth`, y reescribe las llamadas a través del proxy cuando
  `TRYIT_PROXY_PUBLIC_URL` está definido.
- **RapiDoc** - renderizado en `/reference/torii-rapidoc`; refleja el campo de token,
  reutiliza los mismos encabezados que el panel Swagger, y apunta al proxy
  automáticamente cuando la URL está configurada.
- **Pruébalo consola** - incluido en la página de descripción general de la API; permite enviar
  solicitudes personalizadas, ver encabezados e inspeccionar cuerpos de respuesta.

Ambos paneles muestran un **selector de instantáneas** que lee
`docs/portal/static/openapi/versions.json`. Llena ese indice con
`npm run sync-openapi -- --version=<label> --mirror=current --latest` para que los revisores
puedan saltar entre especificaciones históricas, ver el resumen SHA-256 registrado y confirmar si un
La instantánea de lanzamiento trae un manifiesto firmado antes de usar los widgets interactivos.

Cambiar el token en cualquier widget solo afecta la sesión actual del navegador; el proxy nunca
persiste ni registra el token proporcionado.

## Tokens OAuth de corta vidaPara evitar distribuir tokens Torii de larga duración a los revisores, conecta la consola Pruébalo a
su servidor OAuth. Cuando las variables de entorno abajo están presentes el portal renderiza un
widget de inicio de sesión con código de dispositivo, genera tokens portadores de corta vida, y los inyecta automáticamente
en el formulario de la consola.

| Variables | propuesta | Predeterminado |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Punto final de autorización de dispositivo OAuth (`/oauth/device/code`) | _vacío (deshabilitado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Punto final de token que acepta `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vacío_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth registrado para la vista previa de documentos | _vacío_ |
| `DOCS_OAUTH_SCOPE` | Ámbitos separados por espacios solicitados durante el inicio de sesión | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audiencia de API opcional para vincular el token | _vacío_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo minimo de polling mientras espera aprobación (ms) | `5000` (valores < 5000 ms se rechazan) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Ventana de vencimiento del código del dispositivo (segundos) | `600` (debe mantenerse entre 300 s y 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duración del token de acceso (segundos) | `900` (debe mantenerse entre 300 s y 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Pon `1` para vistas previas locales que omiten la aplicación OAuth intencionalmente | _desarmado_ |

Ejemplo de configuración:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Cuando ejecutas `npm run start` o `npm run build`, el portal incrusta estos valores
es `docusaurus.config.js`. Durante una vista previa local la tarjeta Pruébalo muestra un
botón "Iniciar sesión con el código del dispositivo". Los usuarios ingresan el código mostrado en tu página de verificación OAuth; una vez que el flow del dispositivo tiene exito el widget:

- inyecta el token al portador emitido en el campo de la consola Pruébalo,
- etiqueta las solicitudes con los encabezados existentes `X-TryIt-Client` y `X-TryIt-Auth`,
- muestra el tiempo de vida restante, y
- borra automáticamente el token cuando expira.

La entrada manual Bearer sigue disponible; omite las variables OAuth cuando quieras
forzar a los reviewers a pegar un token temporal ellos mismos, o exporta
`DOCS_OAUTH_ALLOW_INSECURE=1` para vistas previas locales aisladas donde el acceso anónimo es
aceptable. Los builds sin OAuth configurados ahora fallan rápidamente para satisfacer el gate
de la hoja de ruta DOCS-1b.

Nota: Revisa la [Lista de verificación de prueba de penetración y refuerzo de seguridad](./security-hardening.md)
antes de exponer el portal fuera del laboratorio; documenta el modelo de amenaza,
el perfil CSP/Trusted Types, y los pasos de pen-test que ahora bloquean DOCS-1b.

## Muestras Norito-RPCLas solicitudes Norito-RPC comparten el mismo proxy y fontanería OAuth que las rutas JSON;
simplemente configuren `Content-Type: application/x-norito` y envian el payload Norito
precodificado descrito en la especificación NRPC
(`docs/source/torii/nrpc_spec.md`).
El repositorio incluye payloads canónicos bajo `fixtures/norito_rpc/` para que autores del portal,
Los propietarios de SDK y revisores pueden reproducir los bytes exactos que usa CI.

### Enviar un payload Norito desde la consola Pruébalo

1. Elija un dispositivo como `fixtures/norito_rpc/transfer_asset.norito`. estos
   archivos son sobres Norito en bruto; **no** los codificados en base64.
2. En Swagger o RapiDoc, localiza el endpoint NRPC (por ejemplo
   `POST /v2/pipeline/submit`) y cambia el selector **Content-Type** a
   `application/x-norito`.
3. Cambia el editor del cuerpo de solicitud a **binary** (modo "File" de Swagger o
   selector "Binary/File" de RapiDoc) y carga el archivo `.norito`. El widget
   transmite los bytes a través del proxy sin alterarlos.
4. Envía la solicitud. Si Torii devuelve `X-Iroha-Error-Code: schema_mismatch`,
   verifica que estás llamando un endpoint que acepta payloads binarios y confirma
   que el hash del esquema está registrado en `fixtures/norito_rpc/schema_hashes.json`
   coincide con el build de Torii que estas usando.La consola mantiene el archivo más reciente en memoria para que puedas reenviar el mismo.
carga útil mientras pruebas diferentes tokens de autorización o hosts de Torii. Agregar
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` a tu flujo de trabajo produce el paquete de
evidencia referenciada en el plan de adopción NRPC-4 (log + resumen JSON), lo cual combina
bien con capturar capturas de pantalla de la respuesta Pruébelo durante la revisión.

### Ejemplo CLI (curl)

Los mismos dispositivos se pueden reproducir fuera del portal vía `curl`, lo que es útil
cuando validas el proxy o depuras respuestas del gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Cambia el aparato por cualquier entrada listada en `transaction_fixtures.manifest.json`
o codifica tu propia carga útil con `cargo xtask norito-rpc-fixtures`. Cuando Torii esta en
modo canary puedes apuntar `curl` al proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) para ejercitar la misma
infraestructura que usan los widgets del portal.

## Observabilidad y operaciones

Cada solicitud se registra una vez con método, ruta, origen, estado aguas arriba y la fuente.
de autenticación (`override`, `default` o `client`). Los tokens nunca se almacenan: tanto
los encabezados portadores como los valores `X-TryIt-Auth` se redactan antes de registrador,
asi que puedes reenviar stdout a un colector central sin preocuparte por filtraciones.

### Sondas de salud y alertas

Ejecuta la sonda incluida durante despliegues o en un horario:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```Perillas de entorno:

- `TRYIT_PROXY_SAMPLE_PATH` - ruta Torii opcional (sin `/proxy`) para ejercitar.
- `TRYIT_PROXY_SAMPLE_METHOD` - por defecto `GET`; define `POST` para rutas de escritura.
- `TRYIT_PROXY_PROBE_TOKEN` - inyecta un token portador temporal para la llamada de muestra.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - describe el tiempo de espera por defecto de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destino opcional de texto Prometheus para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por comas anexados a las métricas (por defecto `job=tryit-proxy` y `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional del endpoint de métricas (por ejemplo, `http://localhost:9798/metrics`) que debe responder con salida cuando `TRYIT_PROXY_METRICS_LISTEN` está habilitado.

Alimenta los resultados en un recopilador de archivos de texto apuntando el probe a una ruta escribible
(por ejemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) y agregando etiquetas
personalizados:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

El script reescribe el archivo de métricas de forma atómica para que tu coleccionista siempre lea.
una carga útil completa.

Cuando `TRYIT_PROXY_METRICS_LISTEN` esté configurado, defina
`TRYIT_PROXY_PROBE_METRICS_URL` al endpoint de métricas para que el probe falle rapido si la
La superficie de raspado desaparece (por ejemplo, ingreso mal configurado o reglas de firewall faltantes).
Un ajuste típico de producción es
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.Para alertas livianas, conecte la sonda a su pila de monitoreo. Ejemplo de Prometheus que
pagina despues de dos fallos consecutivos:

```yaml
groups:
  - name: tryit-proxy
    rules:
      - alert: TryItProxyUnhealthy
        expr: probe_success{job="tryit-proxy"} == 0
        for: 2m
        labels:
          severity: page
        annotations:
          summary: Try It proxy is failing health checks
          description: |
            The try-it proxy at {{ $labels.instance }} is not responding to probe requests.
```

### Punto final de métricas y paneles

Configure `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (o cualquier par host/puerto) antes de
inicie el proxy para exponer un punto final de métricas con formato Prometheus. la ruta
por defecto es `/metrics` pero puede cambiarse con `TRYIT_PROXY_METRICS_PATH=/custom`. Cada
scrape devuelve contadores de totales por método, rechazos por límite de tasa, errores/timeouts
upstream, resultados del proxy y resúmenes de latencia:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Apunta tus colectores Prometheus/OTLP al endpoint de métricas y reutiliza los paneles existentes
en `dashboards/grafana/docs_portal.json` para que SRE observe latencias de cola y picos de
rechazo sin analizar registros. El proxy publica automáticamente `tryit_proxy_start_timestamp_ms`
para ayudar a los operadores a detectar reinicios.

### Automatización de reversión

Utilice el asistente de gestión para actualizar o restaurar la URL objetivo de Torii. El guión
guarda la configuración previa en `.env.tryit-proxy.bak` para que los rollbacks sean un
comando en solitario.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrescribe la ruta del archivo env con `--env` o `TRYIT_PROXY_ENV` si tu despliegue
guarde la configuración en otro lugar.