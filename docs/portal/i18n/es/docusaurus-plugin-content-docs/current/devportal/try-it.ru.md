---
lang: es
direction: ltr
source: docs/portal/docs/devportal/try-it.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Песочница Pruébalo

El portal de acceso rápido incluye la consola opcional "Pruébelo", para saber cómo configurar los puntos finales Torii, no necesita acceso documentos. Консоль проксирует запросы через встроенный proxy, чтобы браузеры обходили ограничения CORS, одновременно соблюдая и аутентификацию.

## Предварительные условия

- Node.js 18.18 o nueva (portal de compilación de tres versiones)
- Сетевой доступ к staging окружению Torii
- token al portador, который может вызывать нужные маршруты Torii

Вся настройка proxy выполняется через переменные окружения. В таблице ниже перечислены основные mandos:| Variables | Назначение | Predeterminado |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL básica Torii, el proxy busca mensajes | **Obligatorio** |
| `TRYIT_PROXY_LISTEN` | Dirección de procesamiento de datos locales (formato `host:port` o `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Список origin-ов, которым разрешено обращаться к proxy (через запятую) | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador incorporado en `X-TryIt-Client` para el registro ascendente | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Token al portador по умолчанию, пересылаемый в Torii | _vacío_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Utilice un token de alta calidad para `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максимальный размер тела запроса (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Tiempo de espera ascendente en miles de millones | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Cómo limitar la velocidad del cliente IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Скользящее окно limitación de velocidad (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Dirección de procesamiento del parámetro Prometheus (`host:port` o `[ipv6]:port`) | _vacío (deshabilitado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Entrada HTTP para la métrica del punto final | `/metrics` |

El panel de proxy expone `GET /healthz`, crea una estructura JSON y redacta tokens de portador en registros.Abra `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`, un archivo proxy para consultar documentos, paneles Swagger y RapiDoc que pueden transferir tokens de portador, incluidos пользователем. El proxy permite establecer límites de velocidad, eliminar credenciales y desactivar el token, implementar o anular los bloqueos. Instale `TRYIT_PROXY_CLIENT_ID` con el método, ya que puede utilizar el `X-TryIt-Client`
(по умолчанию `docs-portal`). Proxy validado y validado por el cliente `X-TryIt-Client` y configurado de forma predeterminada, estas puertas de enlace provisionales pueden auditar su procedencia según las correlaciones с метаданными браузера.

## Запуск proxy local

Установите зависимости при первой настройке портала:

```bash
cd docs/portal
npm install
```

Abra el proxy y utilice la instancia Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

El script de enlace de dirección y el proceso de conexión con `/proxy/*` en el origen interno Torii.

Antes de vincular el script de escritura, что
`static/openapi/torii.json` соответствует digest, записанному в
`static/openapi/manifest.json`. Si el archivo se elimina, el comando завершается ошибкой y предлагает выполнить `npm run sync-openapi -- --latest`. Exportar
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` только для аварийных обходов; El proxy abre la ventana de mantenimiento anterior y del proceso de actualización.

## Подключение виджетов портала

Para compilar o servir un portal de configuración de URL, estos vídeos deben utilizarse para el proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```Los siguientes componentes están relacionados con `docusaurus.config.js`:

- **Swagger UI** - отображается на `/reference/torii-swagger`; preautoriza el esquema de portador para el token, activa el `X-TryIt-Client`, activa el `X-TryIt-Auth` y controla el proxy, como `TRYIT_PROXY_PUBLIC_URL` задан.
- **RapiDoc** - отображается на `/reference/torii-rapidoc`; Utiliza el token, actualiza los encabezados Swagger y configura automáticamente el proxy en la URL actual.
- **Pruébelo en la consola** - встроен в страницу обзора API; Puede controlar los archivos adjuntos, controlar los encabezados e inspeccionar los otros.

Обе панели показывают **selector de instantáneas**, который читает
`docs/portal/static/openapi/versions.json`. Заполните индекс командой
`npm run sync-openapi -- --version=<label> --mirror=current --latest`, muchos revisores pueden mejorar las especificaciones históricas, ver un resumen y resumen completo de SHA-256 y publicar una instantánea имеет manifiesto firmado antes de la utilización de vídeos interactivos.

Смена token в любом виджете влияет только на текущую сессию браузера; proxy никогда не сохраняет не логирует переданный token.

## Короткоживущие tokens de OAuth

No es necesario transmitir los tokens Torii de los revisores, puede probarlo en la consola del servidor OAuth. Como opción permanente para no usar dispositivos, widget de código de dispositivo portátil, tokens de portador y automáticos подставляет их в форму консоли.| Variables | Назначение | Predeterminado |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Punto final de autorización de dispositivo OAuth (`/oauth/device/code`) | _vacío (deshabilitado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Punto final del token, modelo `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vacío_ |
| `DOCS_OAUTH_CLIENT_ID` | ID de cliente de OAuth, descargado en vista previa de documentos | _vacío_ |
| `DOCS_OAUTH_SCOPE` | Ámbitos, pruebas de detección, pruebas de vídeo | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audiencia API opcional para tokens de acceso | _vacío_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo de sondeo mínimo para el intervalo de tiempo (ms) | `5000` (значения < 5000 ms отклоняются) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Фоллбек Código de dispositivo TTL (segundos) | `600` (entre 300 s y 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Фоллбек TTL token de acceso (segundos) | `900` (entre 300 s y 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Instale `1` para una vista previa local, un nombre de usuario OAuth | _desarmado_ |

Configuración de ejemplo:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Cuando conecte `npm run start` o `npm run build`, este portal se encuentra en `docusaurus.config.js`. Во время локального vista previa de la tarjeta Pruébelo presionando el botón "Iniciar sesión con el código del dispositivo". Пользователи вводят показанный код на вашей OAuth странице; после успешного flujo del dispositivo video:- вставляет выданный token de portador en поле консоли Pruébalo,
- добавляет encabezados `X-TryIt-Client` y `X-TryIt-Auth`,
- отображает оставшееся время жизни,
- автоматически очищает token при истечении срока.

Ручной ввод остается доступным - уберите OAuth переменные, если хотите заставить revisores вставлять временный token вручную, или Exporte `DOCS_OAUTH_ALLOW_INSECURE=1` para una vista previa local aislada, un principio de descarga anónimo. Se compila a partir de la base OAuth, que está conectada a la puerta DOCS-1b.

Nota: Перед публикацией портала за пределами лаборатории проверьте [Lista de verificación de prueba de penetración y refuerzo de seguridad] (./security-hardening.md); En el modelo de amenaza no detallado, el perfil CSP/Trusted Types y las pruebas de penetración, la puerta DOCS-1b.

## Primeros Norito-RPC

Norito-RPC utiliza el proxy y la plomería OAuth, así como las rutas JSON; они просто ставят `Content-Type: application/x-norito` and отправляют заранее закодированный Norito payload, описанный в спецификации NRPC (`docs/source/torii/nrpc_spec.md`). El repositorio de cargas útiles del programador incluye `fixtures/norito_rpc/`, el portal de autores, los propietarios y revisores del SDK contienen muchos bytes. используемые CI.

### Carga útil de Norito en la consola Pruébalo1. Coloque el dispositivo, nombre `fixtures/norito_rpc/transfer_asset.norito`. Estos archivos son sobres Norito; **не** codifica en base64.
2. En Swagger o RapiDoc, seleccione el punto final NRPC (por ejemplo, `POST /v2/pipeline/submit`) y seleccione el selector **Tipo de contenido** en `application/x-norito`.
3. Coloque el redactor en **binary** (reduzca "Archivo" en Swagger o "Binary/File" en RapiDoc) y descargue el archivo `.norito`. Utilice bytes para que el proxy no esté configurado.
4. Отправьте запрос. Si Torii incluye `X-Iroha-Error-Code: schema_mismatch`, tendrá en cuenta el punto final, las cargas útiles binarias iniciales y el esquema hash en `fixtures/norito_rpc/schema_hashes.json` se actualiza con la compilación Torii.

Si utiliza un archivo de configuración de archivos, puede permitir que la carga útil de los tokens de autorización de la red y los hosts Torii. Incorporado `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` en el paquete de evidencia del flujo de trabajo, incluido en el plan de registro NRPC-4 (registro + resumen JSON), conjunto completo de datos скриншотом ответа Pruébalo con otras reseñas.

### CLI пример (curl)

Estos dispositivos pueden programarse en el portal `curl`, qué polos para el proxy u otras puertas de enlace:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```Coloque el dispositivo en la entrada del disco en `transaction_fixtures.manifest.json` o guarde su carga útil en el comando `cargo xtask norito-rpc-fixtures`. Si Torii funciona con la versión canary, puede utilizar `curl` en try-it proxy (`https://docs.sora.example/proxy/v2/pipeline/submit`), para comprobarlo инфраструктуру, что и виджеты портала.

## Observabilidad y operaciones.

La configuración de registro se basa en el método, la ruta, el origen, el estado ascendente y las autentificaciones históricas (`override`, `default` o `client`). Tokens никогда не сохраняются - encabezados de portador y значения `X-TryIt-Auth` redactar antes de iniciar sesión, puede permitir la salida estándar центральный colector без риска утечки секretов.

### Sondas de salud y alertas

Utilice la sonda actual para implementar o realizar correctamente:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Mandos de operación:- `TRYIT_PROXY_SAMPLE_PATH` - опциональный Torii маршрут (без `/proxy`) для проверки.
- `TRYIT_PROXY_SAMPLE_METHOD` - по умолчанию `GET`; задайте `POST` para escribir маршрутов.
- `TRYIT_PROXY_PROBE_TOKEN` - добавляет временный token al portador к muestra вызову.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - Tiempo de espera prolongado de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - archivo de texto opcional Prometheus para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - partes `key=value`, добавляемые к метрикам (по умолчанию `job=tryit-proxy` и `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL`: punto final de métricas de URL opcional (por ejemplo, `http://localhost:9798/metrics`), que se utiliza exclusivamente para `TRYIT_PROXY_METRICS_LISTEN`.

Busque resultados en el recopilador de archivos de texto, introduzca la sonda en la ruta de escritura (por ejemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) y agregue etiquetas automáticas:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

El script atómico contiene un archivo métrico, un recopilador de datos y una carga útil de una determinada cantidad.

Если `TRYIT_PROXY_METRICS_LISTEN` настроен, задайте
`TRYIT_PROXY_PROBE_METRICS_URL` en el punto final de métricas, la sonda se activa mediante la función de raspado (por ejemplo, nunca ingreso o reglas de firewall externas). Типичная producción настройка:
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

La alerta eléctrica permite conectar la sonda a la pila de monitoreo de la batería. Por ejemplo, Prometheus, este mensaje aparece después de dos pasos posteriores:

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

### Punto final de métricas y panelesInstale `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (o el mismo host/puerto) antes de descargar el proxy, para crear el punto final de métricas con formato Prometheus. Ponga en contacto con `/metrics`, pero no puede predeterminar el número de `TRYIT_PROXY_METRICS_PATH=/custom`. Los métodos de scrape más rápidos son: rechazos de límite de velocidad, errores/tiempos de espera ascendentes, resultados de proxy y latencia resumida:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Coloque los recopiladores Prometheus/OTLP en el punto final de métricas y actualice los paneles en `dashboards/grafana/docs_portal.json`, donde SRE puede aumentar la latencia de cola y los componentes. отказов без парсинга логов. El proxy automático publicado `tryit_proxy_start_timestamp_ms`, puede permitir que el operador reinicie.

### Reversión automática

Utilice el asistente de administración para la URL de actualización o instalación de la URL Torii. El script está configurado previamente a la configuración en `.env.tryit-proxy.bak` y luego retrocede, este es el comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Asegúrese de colocar el archivo env con `--env` o `TRYIT_PROXY_ENV`, o si desea configurar una configuración diferente en el sitio web. месте.