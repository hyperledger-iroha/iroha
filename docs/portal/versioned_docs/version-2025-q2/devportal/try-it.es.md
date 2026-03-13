---
lang: es
direction: ltr
source: docs/portal/versioned_docs/version-2025-q2/devportal/try-it.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: c389a2121f577bcf8893a0d5c0b898ec2ff5330f2f1727de3387da98f8369915
source_last_modified: "2025-11-15T05:16:44.667295+00:00"
translation_last_reviewed: 2026-01-30
---

# Sandbox de Try It

El portal de desarrolladores incluye una consola opcional "Try it" para que puedas llamar endpoints de Torii sin salir de la documentacion. La consola retransmite solicitudes a traves del proxy incluido para que los navegadores puedan evitar limites CORS mientras se siguen aplicando limites de tasa y autenticacion.

## Prerequisitos

- Node.js 18.18 o mas nuevo (coincide con los requisitos de build del portal)
- Acceso de red a un entorno de staging de Torii
- Un bearer token que pueda llamar las rutas Torii que planeas ejercitar

Toda la configuracion del proxy se realiza mediante variables de entorno. La tabla siguiente lista los knobs mas importantes:

| Variable | Proposito | Default |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL base de Torii a la que el proxy reenvia solicitudes | **Required** |
| `TRYIT_PROXY_LISTEN` | Direccion de escucha para desarrollo local (formato `host:port` o `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por comas de origenes que pueden llamar al proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador colocado en `X-TryIt-Client` para cada solicitud upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token por defecto reenviado a Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permite a los usuarios proporcionar su propio token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamano maximo del cuerpo de solicitud (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout upstream en milisegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Solicitudes permitidas por ventana de tasa por IP de cliente | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Ventana deslizante para rate limiting (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Direccion de escucha opcional para el endpoint de metricas estilo Prometheus (`host:port` o `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | Ruta HTTP servida por el endpoint de metricas | `/metrics` |

El proxy tambien expone `GET /healthz`, devuelve errores JSON estructurados y oculta bearer tokens de la salida de logs.

Activa `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` al exponer el proxy a usuarios de docs para que los paneles Swagger y RapiDoc puedan reenviar bearer tokens proporcionados por el usuario. El proxy aun aplica limites de tasa, oculta credenciales y registra si una solicitud uso el token por defecto o una anulacion por solicitud. Configura `TRYIT_PROXY_CLIENT_ID` con la etiqueta que quieres enviar como `X-TryIt-Client`
(por defecto `docs-portal`). El proxy recorta y valida valores `X-TryIt-Client` aportados por el cliente, volviendo a este default para que los gateways de staging puedan auditar la procedencia sin correlacionar metadatos del navegador.

## Inicia el proxy localmente

Instala dependencias la primera vez que configures el portal:

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

El script registra la direccion enlazada y reenvia solicitudes desde `/proxy/*` al origen Torii configurado.

Antes de enlazar el socket el script valida que
`static/openapi/torii.json` coincide con el digest registrado en
`static/openapi/manifest.json`. Si los archivos se desincronizan, el comando termina con un
error y te indica ejecutar `npm run sync-openapi -- --latest`. Exporta
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` solo para overrides de emergencia; el proxy registrara una
advertencia y continuara para que puedas recuperarte durante ventanas de mantenimiento.

## Conecta los widgets del portal

Cuando haces build o sirves el portal de desarrolladores, define la URL que los widgets
deben usar para el proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Los siguientes componentes leen estos valores desde `docusaurus.config.js`:

- **Swagger UI** - renderizado en `/reference/torii-swagger`; preautoriza el esquema
  bearer cuando hay un token, etiqueta las solicitudes con `X-TryIt-Client`,
  inyecta `X-TryIt-Auth`, y reescribe las llamadas a traves del proxy cuando
  `TRYIT_PROXY_PUBLIC_URL` esta definido.
- **RapiDoc** - renderizado en `/reference/torii-rapidoc`; refleja el campo de token,
  reutiliza los mismos headers que el panel Swagger, y apunta al proxy
  automaticamente cuando la URL esta configurada.
- **Try it console** - embebida en la pagina de overview de la API; permite enviar
  solicitudes personalizadas, ver headers e inspeccionar cuerpos de respuesta.

Ambos paneles muestran un **selector de snapshots** que lee
`docs/portal/static/openapi/versions.json`. Llena ese indice con
`npm run sync-openapi -- --version=<label> --mirror=current --latest` para que los reviewers
puedan saltar entre specs historicas, ver el digest SHA-256 registrado, y confirmar si un
snapshot de release trae un manifest firmado antes de usar los widgets interactivos.

Cambiar el token en cualquier widget solo afecta la sesion actual del navegador; el proxy nunca
persiste ni registra el token proporcionado.

## Tokens OAuth de corta vida

Para evitar distribuir tokens Torii de larga duracion a los reviewers, conecta la consola Try it a
tu servidor OAuth. Cuando las variables de entorno abajo estan presentes el portal renderiza un
widget de login con device code, genera bearer tokens de corta vida, y los inyecta automaticamente
en el formulario de la consola.

| Variable | Proposito | Default |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint de autorizacion de dispositivo OAuth (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | Endpoint de token que acepta `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth registrado para el preview de docs | _empty_ |
| `DOCS_OAUTH_SCOPE` | Scopes separados por espacios solicitados durante el login | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audiencia de API opcional para vincular el token | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo minimo de polling mientras espera aprobacion (ms) | `5000` (valores < 5000 ms se rechazan) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Ventana de expiracion del device code (segundos) | `600` (debe mantenerse entre 300 s y 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duracion del access token (segundos) | `900` (debe mantenerse entre 300 s y 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Pon `1` para previews locales que omiten enforcement OAuth intencionalmente | _unset_ |

Ejemplo de configuracion:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Cuando ejecutas `npm run start` o `npm run build`, el portal incrusta estos valores
en `docusaurus.config.js`. Durante un preview local la tarjeta Try it muestra un
boton "Sign in with device code". Los usuarios ingresan el codigo mostrado en tu pagina de verificacion OAuth; una vez que el device flow tiene exito el widget:

- inyecta el bearer token emitido en el campo de la consola Try it,
- etiqueta las solicitudes con los headers existentes `X-TryIt-Client` y `X-TryIt-Auth`,
- muestra el tiempo de vida restante, y
- borra automaticamente el token cuando expira.

La entrada manual Bearer sigue disponible; omite las variables OAuth cuando quieras
forzar a los reviewers a pegar un token temporal ellos mismos, o exporta
`DOCS_OAUTH_ALLOW_INSECURE=1` para previews locales aisladas donde el acceso anonimo es
aceptable. Los builds sin OAuth configurado ahora fallan rapido para satisfacer el gate
del roadmap DOCS-1b.

Nota: Revisa la [Security hardening & pen-test checklist](./security-hardening.md)
antes de exponer el portal fuera del laboratorio; documenta el threat model,
el perfil CSP/Trusted Types, y los pasos de pen-test que ahora bloquean DOCS-1b.

## Muestras Norito-RPC

Las solicitudes Norito-RPC comparten el mismo proxy y plumbing OAuth que las rutas JSON;
simplemente configuran `Content-Type: application/x-norito` y envian el payload Norito
pre-encodado descrito en la especificacion NRPC
(`docs/source/torii/nrpc_spec.md`).
El repositorio incluye payloads canonicos bajo `fixtures/norito_rpc/` para que autores del portal,
owners de SDK y reviewers puedan reproducir los bytes exactos que usa CI.

### Enviar un payload Norito desde la consola Try It

1. Elige un fixture como `fixtures/norito_rpc/transfer_asset.norito`. Estos
   archivos son envelopes Norito en bruto; **no** los codifiques en base64.
2. En Swagger o RapiDoc, localiza el endpoint NRPC (por ejemplo
   `POST /v2/pipeline/submit`) y cambia el selector **Content-Type** a
   `application/x-norito`.
3. Cambia el editor del cuerpo de solicitud a **binary** (modo "File" de Swagger o
   selector "Binary/File" de RapiDoc) y carga el archivo `.norito`. El widget
   transmite los bytes a traves del proxy sin alterarlos.
4. Envia la solicitud. Si Torii devuelve `X-Iroha-Error-Code: schema_mismatch`,
   verifica que estes llamando un endpoint que acepte payloads binarios y confirma
   que el schema hash registrado en `fixtures/norito_rpc/schema_hashes.json`
   coincide con el build de Torii que estas usando.

La consola mantiene el archivo mas reciente en memoria para que puedas reenviar el mismo
payload mientras pruebas diferentes tokens de autorizacion o hosts de Torii. Agregar
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` a tu workflow produce el bundle de
evidencia referenciado en el plan de adopcion NRPC-4 (log + resumen JSON), lo cual combina
bien con capturar screenshots de la respuesta Try It durante revisiones.

### Ejemplo CLI (curl)

Los mismos fixtures se pueden reproducir fuera del portal via `curl`, lo que es util
cuando validas el proxy o depuras respuestas del gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Cambia el fixture por cualquier entrada listada en `transaction_fixtures.manifest.json`
o codifica tu propio payload con `cargo xtask norito-rpc-fixtures`. Cuando Torii esta en
modo canary puedes apuntar `curl` al proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) para ejercitar la misma
infraestructura que usan los widgets del portal.

## Observabilidad y operaciones

Cada solicitud se registra una vez con metodo, path, origen, estado upstream y la fuente
de autenticacion (`override`, `default` o `client`). Los tokens nunca se almacenan: tanto
los headers bearer como los valores `X-TryIt-Auth` se redaccionan antes de registrar,
asi que puedes reenviar stdout a un collector central sin preocuparte por filtraciones.

### Probes de salud y alertas

Ejecuta el probe incluido durante despliegues o en un schedule:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Knobs de entorno:

- `TRYIT_PROXY_SAMPLE_PATH` - ruta Torii opcional (sin `/proxy`) para ejercitar.
- `TRYIT_PROXY_SAMPLE_METHOD` - por defecto `GET`; define `POST` para rutas de escritura.
- `TRYIT_PROXY_PROBE_TOKEN` - inyecta un bearer token temporal para la llamada de muestra.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - sobrescribe el timeout por defecto de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destino opcional de texto Prometheus para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por comas anexados a las metricas (por defecto `job=tryit-proxy` y `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional del endpoint de metricas (por ejemplo, `http://localhost:9798/metrics`) que debe responder con exito cuando `TRYIT_PROXY_METRICS_LISTEN` esta habilitado.

Alimenta los resultados en un textfile collector apuntando el probe a una ruta escribible
(por ejemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) y agregando etiquetas
personalizadas:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

El script reescribe el archivo de metricas de forma atomica para que tu collector siempre lea
un payload completo.

Cuando `TRYIT_PROXY_METRICS_LISTEN` esta configurado, define
`TRYIT_PROXY_PROBE_METRICS_URL` al endpoint de metricas para que el probe falle rapido si la
superficie de scrape desaparece (por ejemplo, ingress mal configurado o reglas de firewall faltantes).
Un ajuste tipico de production es
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Para alertas livianas, conecta el probe a tu stack de monitoreo. Ejemplo de Prometheus que
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

### Endpoint de metricas y dashboards

Configura `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (o cualquier par host/puerto) antes de
iniciar el proxy para exponer un endpoint de metricas con formato Prometheus. La ruta
por defecto es `/metrics` pero puede cambiarse con `TRYIT_PROXY_METRICS_PATH=/custom`. Cada
scrape devuelve contadores de totales por metodo, rechazos por rate limit, errores/timeouts
upstream, outcomes del proxy y resumenes de latencia:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Apunta tus collectors Prometheus/OTLP al endpoint de metricas y reutiliza los panels existentes
en `dashboards/grafana/docs_portal.json` para que SRE observe latencias de cola y picos de
rechazo sin parsear logs. El proxy publica automaticamente `tryit_proxy_start_timestamp_ms`
para ayudar a los operadores a detectar reinicios.

### Automatizacion de rollback

Usa el helper de gestion para actualizar o restaurar la URL objetivo de Torii. El script
guarda la configuracion previa en `.env.tryit-proxy.bak` para que los rollbacks sean un
solo comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrescribe la ruta del archivo env con `--env` o `TRYIT_PROXY_ENV` si tu despliegue
guarda la configuracion en otro lugar.
