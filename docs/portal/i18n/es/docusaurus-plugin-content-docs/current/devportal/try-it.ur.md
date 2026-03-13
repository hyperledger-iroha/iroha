---
lang: es
direction: ltr
source: docs/portal/docs/devportal/try-it.ur.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Pruébalo سینڈ باکس

ڈویلپر پورٹل ایک اختیاری "Pruébalo" کنسول فراہم کرتا ہے تاکہ آپ دستاویزات چھوڑے بغیر Puntos finales Torii کال کر سکیں۔ کنسول بنڈل پروکسی کے ذریعے درخواستیں ری لے کرتا ہے تاکہ براؤزر CORS حدود بائی پاس کر سکیں جبکہ ریٹ لمٹس اور آتھنٹیکیشن نافذ رہیں۔

## شرائط

- Node.js 18.18 یا نیا (compilación completa ضروریات کے مطابق)
- Puesta en escena Torii ماحول تک نیٹ ورک رسائی
- Token de portador جو آپ جن Torii rutas کو ٹیسٹ کرنا چاہتے ہیں انہیں کال کر سکے

پروکسی کی تمام کنفیگریشن variables de entorno سے ہوتی ہے۔ نیچے اہم perillas کی فہرست ہے:| Variables | مقصد | Predeterminado |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | Torii URL base جس پر پروکسی درخواستیں فارورڈ کرتا ہے | **Obligatorio** |
| `TRYIT_PROXY_LISTEN` | لوکل ڈیولپمنٹ کے لئے dirección de escucha (`host:port` یا `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | orígenes کی separados por comas فہرست جو پروکسی کو کال کر سکتے ہیں | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | ہر solicitud ascendente میں `X-TryIt-Client` کے طور پر لگایا گیا identificador | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Token al portador ڈیفالٹ جو Torii کو فارورڈ ہوتا ہے | _vacío_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | صارفین کو `X-TryIt-Auth` کے ذریعے اپنا token دینے کی اجازت | `0` |
| `TRYIT_PROXY_MAX_BODY` | cuerpo de la solicitud کا زیادہ سے زیادہ سائز (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | tiempo de espera ascendente (milisegundos) | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | ہر IP del cliente کے لئے فی ventana اجازت شدہ solicitudes | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | limitación de velocidad کے لئے ventana corredera (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Prometheus Punto final de métricas کے لئے اختیاری dirección de escucha (`host:port` یا `[ipv6]:port`) | _vacío (deshabilitado)_ |
| `TRYIT_PROXY_METRICS_PATH` | punto final de métricas y ruta HTTP | `/metrics` |

Error `GET /healthz` Errores JSON estructurados, tokens de portador y salida de registro, redacción de datos`TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` Los usuarios de documentos exponen los tokens de portador proporcionados por el usuario de los paneles Swagger y RapiDoc. سکیں۔ پروکسی اب بھی límites de tasa نافذ کرتا ہے، credenciales کو redactar کرتا ہے، اور ریکارڈ کرتا ہے کہ solicitud نے token predeterminado استعمال کیا یا anulación por solicitud۔ `TRYIT_PROXY_CLIENT_ID` کو اس etiqueta پر سیٹ کریں جو آپ `X-TryIt-Client` کے طور پر بھیجنا چاہتے ہیں
(ڈیفالٹ `docs-portal`)۔ Valores `X-TryIt-Client` proporcionados por la persona que llama Recortar Validar Puertas de enlace provisionales Metadatos del navegador Correlacionar بغیر auditoría de procedencia کر سکیں۔

## لوکل طور پر پروکسی چلائیں

پورٹل سیٹ اپ کرنے کی پہلی بار dependencias انسٹال کریں:

```bash
cd docs/portal
npm install
```

پروکسی چلائیں طرف پوائنٹ کریں:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

اسکرپٹ dirección enlazada لاگ کرتا ہے اور `/proxy/*` سے solicitudes کو configurado Torii origen کی طرف reenviar کرتا ہے۔

ساکٹ enlazar کرنے سے پہلے اسکرپٹ verificar کرتا ہے کہ
`static/openapi/torii.json` کا resumen `static/openapi/manifest.json` میں ریکارڈ شدہ resumen سے میچ کرے۔ اگر فائلز drift ہو جائیں تو کمانڈ error کے ساتھ exit کر دیتا ہے اور `npm run sync-openapi -- --latest` چلانے کی ہدایت دیتا ہے۔ `TRYIT_PROXY_ALLOW_STALE_SPEC=1` صرف ہنگامی anular کے لئے استعمال کریں؛ پروکسی advertencia لاگ کرے گا اور جاری رہے گا تاکہ mantenimiento ventanas میں ریکوری ہو سکے۔

## پورٹل widgets کو جوڑیںEste es el portal para desarrolladores, la compilación y el servicio, las URL y los widgets que están disponibles:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

یہ componentes `docusaurus.config.js` سے valores پڑھتے ہیں:

- **Swagger UI** - `/reference/torii-swagger` پر render ہوتا ہے؛ token موجود ہونے پر esquema de portador کو preautorización کرتا ہے، solicitudes کو `X-TryIt-Client` سے etiqueta کرتا ہے، `X-TryIt-Auth` inyectar کرتا ہے، اور `TRYIT_PROXY_PUBLIC_URL` سیٹ ہونے پر llamadas کو پروکسی کے ذریعے reescribir کرتا ہے۔
- **RapiDoc** - `/reference/torii-rapidoc` پر render ہوتا ہے؛ token فیلڈ کو mirror کرتا ہے، Swagger panel جیسے headers reuse کرتا ہے، اور URL configure ہونے پر خودکار طور پر proxy کو target کرتا ہے۔
- **Pruébelo en la consola** - Página de descripción general de API میں incrustado ہے؛ solicitudes personalizadas بھیجنے، encabezados دیکھنے، اور cuerpos de respuesta inspeccionar کرنے دیتا ہے۔

دونوں paneles ایک **selector de instantáneas** دکھاتے ہیں جو
`docs/portal/static/openapi/versions.json` پڑھتا ہے۔ اس índice کو
`npm run sync-openapi -- --version=<label> --mirror=current --latest` سے بھریں تاکہ especificaciones históricas de los revisores میں جا سکیں، ریکارڈ شدہ SHA-256 digest دیکھ سکیں، اور widgets interactivos استعمال کرنے سے پہلے تصدیق کر سکیں کہ instantánea de lanzamiento manifiesto firmado لے کر آیا ہے۔

کسی بھی widget میں token بدلنا صرف موجودہ sesión del navegador پر اثر ڈالتا ہے؛ proxy کبھی بھی فراہم کردہ token کو persist یا log نہیں کرتا۔

## مختصر مدت والے Tokens de OAuthطویل مدت والے Torii revisores de tokens کو دینے سے بچنے کے لئے Pruébelo consola کو اپنے Servidor OAuth سے جوڑیں۔ جب نیچے دیے گئے variables de entorno موجود ہوں تو پورٹل widget de inicio de sesión de código de dispositivo دکھاتا ہے، مختصر مدت والے tokens de portador بناتا ہے، اور انہیں forma de consola میں خودکار طور پر inyectar کرتا ہے۔

| Variables | مقصد | Predeterminado |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Punto final de autorización de dispositivo OAuth (`/oauth/device/code`) | _vacío (deshabilitado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Punto final del token جو `grant_type=urn:ietf:params:oauth:grant-type:device_code` قبول کرتا ہے | _vacío_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth جو docs vista previa کے لئے رجسٹر ہے | _vacío_ |
| `DOCS_OAUTH_SCOPE` | iniciar sesión کے دوران مانگے گئے ámbitos delimitados por espacios | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | token کو باندھنے کے لئے اختیاری Audiencia API | _vacío_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | منظوری کے انتظار میں کم سے کم intervalo de sondeo (ms) | `5000` (قدریں < 5000 ms رد ہوتی ہیں) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | código de dispositivo کی ventana de vencimiento alternativa (segundos) | `600` (300 s اور 900 s کے درمیان رہنی چاہئے) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | token de acceso کی vida útil de respaldo (segundos) | `900` (300 s اور 900 s کے درمیان رہنی چاہئے) |
| `DOCS_OAUTH_ALLOW_INSECURE` | لوکل vistas previas کے لئے `1` جو Aplicación de OAuth جان بوجھ کر چھوڑتے ہیں | _desarmado_ |

Configuración de ejemplo:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```جب آپ `npm run start` یا `npm run build` چلاتے ہیں تو پورٹل یہ valores `docusaurus.config.js` میں incrustar کرتا ہے۔ لوکل vista previa کے دوران Pruébelo tarjeta "Iniciar sesión con el código del dispositivo" بٹن دکھاتا ہے۔ صارفین دکھایا گیا código اپنی Página de verificación de OAuth پر درج کرتے ہیں؛ flujo de dispositivo کامیاب ہونے کے بعد widget:

- جاری شدہ token de portador کو Pruébalo consola فیلڈ میں inyectar کرتا ہے،
- موجودہ `X-TryIt-Client` اور `X-TryIt-Auth` encabezados کے ساتھ solicita etiqueta کرتا ہے،
- باقی ماندہ مدت دکھاتا ہے، اور
- token ختم ہونے پر خودکار طور پر صاف کر دیتا ہے۔

manual Entrada de portador دستیاب رہتا ہے - Variables de OAuth کو چھوڑ دیں جب آپ revisores کو عارضی token خود pegar کرنے پر مجبور کرنا چاہتے ہیں، یا `DOCS_OAUTH_ALLOW_INSECURE=1` exportar کریں تاکہ vistas previas aisladas میں acceso anónimo قابل قبول ہو۔ OAuth کے بغیر construye una puerta de hoja de ruta DOCS-1b پورا کرنے کے لئے فوراً falla ہو جاتے ہیں۔

Nota: پورٹل کو لیب سے باہر exponen کرنے سے پہلے [Lista de verificación de prueba de penetración y refuerzo de seguridad](./security-hardening.md) دیکھیں؛ اس میں modelo de amenaza, CSP/Tipos de confianza پروفائل، اور pasos de prueba de penetración شامل ہیں جو اب DOCS-1b کو gate کرتے ہیں۔

## Norito-RPC EspañaNorito-RPC solicita proxy, plomería OAuth y rutas JSON. یہ صرف `Content-Type: application/x-norito` سیٹ کرتی ہیں اور NRPC especificación میں بیان کردہ carga útil Norito precodificada بھیجتی ہیں
(`docs/source/torii/nrpc_spec.md`) ۔ ریپو میں `fixtures/norito_rpc/` کے تحت cargas útiles canónicas موجود ہیں تاکہ autores del portal, propietarios de SDK, اور revisores y bytes de reproducción کر سکیں جو CI استعمال کرتا ہے۔

### Pruébelo consola سے Carga útil Norito بھیجیں

1. `fixtures/norito_rpc/transfer_asset.norito` Accesorio negro منتخب کریں۔ یہ فائلیں sobres Norito sin procesar ہیں؛ انہیں **base64 codificar نہ کریں**۔
2. Swagger یا RapiDoc میں NRPC endpoint تلاش کریں (مثلاً `POST /v2/pipeline/submit`) اور **Content-Type** selector کو `application/x-norito` پر سوئچ کریں۔
3. editor de cuerpo de solicitud کو **binario** پر ٹوگل کریں (Swagger کا "Archivo" موڈ یا RapiDoc کا "Binario/Archivo" selector) اور `.norito` فائل اپ لوڈ کریں۔ bytes de widget کو proxy کے ذریعے بغیر تبدیلی کے stream کرتا ہے۔
4. solicitar بھیجیں۔ اگر Torii `X-Iroha-Error-Code: schema_mismatch` y el punto final کال کر رہے ہیں جو cargas binarias قبول کرتا ہے اور `fixtures/norito_rpc/schema_hashes.json` میں ریکارڈ شدہ esquema hash آپ کے Torii build سے میچ کرتا ہے۔Uso de tokens de autorización y hosts Torii carga útil دوبارہ بھیج سکیں۔ `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` کو flujo de trabajo میں شامل کرنے سے NRPC-4 plan de adopción میں حوالہ دیا گیا paquete de evidencia (registro + resumen JSON) تیار ہوتا ہے، جو reviews کے دوران Pruébelo respuesta کے captura de pantalla کے ساتھ اچھی طرح جاتا ہے۔

### CLI مثال (rizo)

وہی accesorios `curl` کے ذریعے پورٹل سے باہر بھی repetición کیے جا سکتے ہیں، جو proxy validar کرنے یا respuestas de puerta de enlace depurar کرنے میں مددگار ہے:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

`transaction_fixtures.manifest.json` میں موجود کسی بھی entrada کے ساتھ accesorio بدلیں یا `cargo xtask norito-rpc-fixtures` سے اپنا codificación de carga útil کریں۔ جب Torii modo canario میں ہو تو آپ `curl` کو try-it proxy (`https://docs.sora.example/proxy/v2/pipeline/submit`) پر پوائنٹ کر سکتے ہیں تاکہ وہی prueba de infraestructura ہو جو widgets del portal استعمال کرتے ہیں۔

## Observabilidad de operaciones

ہر solicitud ایک بار método, ruta, origen, estado ascendente, اور fuente de autenticación (`override`, `default`, یا `client`) کے ساتھ registro ہوتی ہے۔ tokens کبھی store نہیں ہوتے - encabezados de portador اور `X-TryIt-Auth` registro de valores سے پہلے redact ہو جاتی ہیں، اس لئے آپ stdout کو recopilador central میں forward کر سکتے ہیں بغیر secretos filtrados ہونے کے خدشے کے۔

### Sondas de salud y alertas

implementaciones کے دوران یا programación پر sonda incluida چلائیں:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Perillas ambientales:- `TRYIT_PROXY_SAMPLE_PATH` - اختیاری Torii ruta (بغیر `/proxy`) جسے چیک کرنا ہو۔
- `TRYIT_PROXY_SAMPLE_METHOD` - ڈیفالٹ `GET`؛ escribir rutas کے لئے `POST` سیٹ کریں۔
- `TRYIT_PROXY_PROBE_TOKEN` - llamada de muestra کے لئے عارضی token de portador inyectar کرتا ہے۔
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - ڈیفالٹ Tiempo de espera de 5 s کو anulación کرتا ہے۔
- `TRYIT_PROXY_PROBE_METRICS_FILE` - `probe_success`/`probe_duration_seconds` کے لئے اختیاری Prometheus destino del archivo de texto۔
- `TRYIT_PROXY_PROBE_LABELS` - `key=value` جوڑے métricas separadas por comas میں شامل ہوتے ہیں (ڈیفالٹ `job=tryit-proxy` اور `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL del punto final de métricas (desde `http://localhost:9798/metrics`) a `TRYIT_PROXY_METRICS_LISTEN`

نتائج کو recopilador de archivos de texto میں فیڈ کریں، sonda کو ruta de escritura پر پوائنٹ کر کے
(مثلاً `/var/lib/node_exporter/textfile_collector/tryit.prom`) Estas etiquetas personalizadas son:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

اسکرپٹ métricas فائل کو reescritura atómica کرتا ہے تاکہ recopilador ہمیشہ مکمل carga útil پڑھے۔

جب `TRYIT_PROXY_METRICS_LISTEN` configurar ہو تو `TRYIT_PROXY_PROBE_METRICS_URL` کو punto final de métricas پر سیٹ کریں تاکہ sonda تیزی سے falla ہو اگر raspado de superficie غائب ہو جائے (مثلاً ingreso mal configurado یا faltan reglas de firewall) ۔ ایک configuración de producción típica ہے
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`۔

لائٹ ویٹ alerta کے لئے sonda کو اپنے pila de monitoreo میں جوڑیں۔ Prometheus Errores en la página کرتی ہے:

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

### Punto final de métricas y paneles`TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (host/puerto) سیٹ کریں اور پھر inicio de proxy کریں تاکہ Prometheus exposición del punto final de métricas formateadas ہو۔ ruta ڈیفالٹ `/metrics` ہے لیکن `TRYIT_PROXY_METRICS_PATH=/custom` سے بدل سکتے ہیں۔ ہر totales de método de raspado, rechazos de límite de velocidad, errores/tiempos de espera ascendentes, resultados de proxy, resúmenes de latencia y کرتا ہے:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Recopiladores Prometheus/OTLP, punto final de métricas, reutilización, latencias de cola SRE y picos de rechazo. análisis de registros کیے بغیر دیکھ سکے۔ proxy خودکار طور پر `tryit_proxy_start_timestamp_ms` publicar کرتا ہے تاکہ operadores reinicia detectar کر سکیں۔

### Automatización de reversión

asistente de administración استعمال کریں تاکہ Torii URL de destino کو actualizar یا restaurar کیا جا سکے۔ اسکرپٹ پچھلی configuración کو `.env.tryit-proxy.bak` میں محفوظ کرتا ہے تاکہ rollback ایک ہی کمانڈ ہو۔

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Configuración de implementación de configuración de implementación `--env` `TRYIT_PROXY_ENV` env فائل کا راستہ anulación کریں۔