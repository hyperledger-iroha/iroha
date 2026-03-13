---
lang: es
direction: ltr
source: docs/portal/docs/devportal/try-it.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bac a sable Pruébalo

El portal de desarrollo ofrece una opción de consola "Pruébalo" para llamar a los puntos finales Torii sin salir de la documentación. La consola envía las solicitudes a través del proxy para que los navegadores cumplan con los límites de CORS y apliquen la limitación de velocidad y la autenticación.

## Requisitos previos

- Node.js 18.18 o más reciente (corresponde a las exigencias de compilación del portal)
- Acceso a la investigación a un entorno Torii de puesta en escena.
- Un token de portador capaz de llamar las rutas Torii que desea probar

Toda la configuración del proxy pasa por las variables de entorno. Le tableau ci-dessous liste les Knobs les plus importants:| Variables | Objetivo | Predeterminado |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL Torii de base a la que el proxy relaja las solicitudes | **Obligatorio** |
| `TRYIT_PROXY_LISTEN` | Dirección de ecología para el desarrollo local (formato `host:port` o `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por las vírgenes de los orígenes autorizadas a apelar el proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Lugar identificativo en `X-TryIt-Client` para cada solicitud aguas arriba | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Token al portador por defecto relaie versión Torii | _vacío_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permet aux utilisateurs finalux de fournir leur propre token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamaño máximo del cuerpo de solicitud (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Tiempo de espera ascendente en milisegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Solicitudes autorizadas por la ventana de conexión del cliente IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Ventana deslizante para limitación de velocidad (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Dirección de ecología opcional para el punto final de medición estilo Prometheus (`host:port` o `[ipv6]:port`) | _vacío (deshabilitado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Servicio HTTP para el punto final de medición | `/metrics` |

El proxy expone también `GET /healthz`, envía errores de estructura JSON y enmascara los tokens de portador en los registros.Active `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` cuando exponga el proxy a los usuarios de documentos para que los paneles Swagger y RapiDoc puedan transmitir los tokens al portador proporcionados por el usuario. El proxy aplica siempre los límites de impuestos, enmascara las credenciales y registra si una solicitud utiliza el token por defecto o un recargo por solicitud. Configure `TRYIT_PROXY_CLIENT_ID` con la etiqueta que desea enviar como `X-TryIt-Client`
(por defecto `docs-portal`). Le proxy tronque et valide les valeurs `X-TryIt-Client` fournies par l'appelant, puis retombe sur ce defaut afin que les gateways de staging puissent auditer la provenance sans correler les metadonnees du browser.

## Demarrer le proxy en local

Instale las dependencias de la primera configuración del portal:

```bash
cd docs/portal
npm install
```

Lanza el proxy y apunta hacia tu instancia Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

El script inicia sesión en la dirección ubicada y relaja las solicitudes después de `/proxy/*` al origen Torii configurado.

Antes de carpeta le socket le script valide que
`static/openapi/torii.json` corresponden al resumen registrado en ellos
`static/openapi/manifest.json`. Si los archivos divergen, la comando echoue avec une
Error y demanda del ejecutor `npm run sync-openapi -- --latest`. exportez
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` exclusivo para anulaciones de urgencia; el diálogo proxy
Un aviso y una continuación para permitirle recuperar las ventanas de mantenimiento.## Cableado de widgets del portal

Cuando construyas o sirvas el portal de desarrollo, define la URL que los widgets hacen
Utilizador del proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Los componentes siguientes escuchan estos valores después de `docusaurus.config.js`:

- **Swagger UI** - renderiza un `/reference/torii-swagger`; preautorizar el portador del esquema
  Cuando un token está presente, tague les requetes con `X-TryIt-Client`, inyecte.
  `X-TryIt-Auth`, y vuelva a escribir las llamadas a través del proxy cuando
  `TRYIT_PROXY_PUBLIC_URL` está definido.
- **RapiDoc** - genera un `/reference/torii-rapidoc`; refleja la ficha del campeón,
  Reutilizar los encabezados de memes que le panneau Swagger, y poder hacerlo automáticamente.
  El proxy cuando se configura la URL.
- **Pruébelo en la consola** - Integración en la página de descripción general de la API; permiso de envío des
  solicitudes personalizadas, ver los encabezados e inspeccionar el cuerpo de respuesta.

Les dos panneaux affichent un **selecteur de snapshots** qui lit
`docs/portal/static/openapi/versions.json`. Remplissez cet index avec
`npm run sync-openapi -- --version=<label> --mirror=current --latest` según los revisores
Puede pasar entre las especificaciones históricas, registrar el resumen SHA-256 y confirmarlo.
Si lanza una instantánea, embarque un manifiesto firmado antes de utilizar los widgets interactivos.

Cambia el token en un widget sin tocar el navegador de sesión actual; el proxy ne
persiste jamais et ne logue jamais le token fourni.

## Tokens OAuth de corte durePara evitar el distribuidor de tokens Torii de larga duración para los revisores, suelte la consola Pruébelo
su servidor OAuth. Cuando las variables de entorno actuales están presentes, el portal
extraiga un widget de código de dispositivo de inicio de sesión, obtenga tokens de portador de corta duración y los inyecte
automáticamente en el formulario de la consola.

| Variables | Objetivo | Predeterminado |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Dispositivo de terminal de autorización OAuth (`/oauth/device/code`) | _vacío (deshabilitado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Token de punto final que se acepta `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vacío_ |
| `DOCS_OAUTH_CLIENT_ID` | Cliente identificador OAuth registrarse para la vista previa de documentos | _vacío_ |
| `DOCS_OAUTH_SCOPE` | Ámbitos delimitados por los espacios exigidos al iniciar sesión | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Opción API de audiencia para ver el token | _vacío_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo mínimo de votación durante la atención de aprobación (ms) | `5000` (valores < 5000 ms rechazados) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Código del dispositivo de vencimiento (segundos) | `600` (reposar entre 300 s y 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Token de acceso de duración de vida (segundos) | `900` (reposar entre 300 s y 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Mettre `1` para vistas previas locales que eliminan la aplicación de la intención de OAuth | _desarmado_ |

Ejemplo de configuración:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Cuando lanzas `npm run start` o `npm run build`, el portal integra estos valores
en `docusaurus.config.js`. En vista previa locale la carte Pruébelo affiche un botón
"Iniciar sesión con el código del dispositivo". Los usuarios indican el código que aparece en su página OAuth; Una de las veces que el flujo del dispositivo reutiliza el widget:

- inyecta el token de portador emitido en la consola de campeones Pruébalo,
- tague les requetes con los encabezados existentes `X-TryIt-Client` et `X-TryIt-Auth`,
- cartel con la duración restante, et
- Borra automáticamente el token cuando caduque.

L'entree manuelle Bearer reste disponible; Omete las variables OAuth cuando quieras
Obligar a los revisores a recopilar un token temporal de memes o exportarlos.
`DOCS_OAUTH_ALLOW_INSECURE=1` para vistas previas locales aisladas o acceso anónimo
aceptable. Las compilaciones sin OAuth configuran el mantenimiento rápido para satisfacer las necesidades
para satisfacer la puerta de la hoja de ruta DOCS-1b.

Nota: Consulte la [Lista de verificación de pruebas de penetración y refuerzo de seguridad](./security-hardening.md)
avant d'exposer le portail hors du labo; elle documente el modelo de amenaza,
el perfil CSP/Trusted Types y las etapas de pen-test que bloquean el mantenimiento DOCS-1b.

## Ejemplos Norito-RPCLas solicitudes Norito-RPC comparten el meme proxy y la plomería OAuth que las rutas JSON;
Elles posent simplement `Content-Type: application/x-norito` y envían la carga útil Norito
decrit precodificado en la especificación NRPC
(`docs/source/torii/nrpc_spec.md`).
El depósito dispone de cargas útiles canónicas bajo `fixtures/norito_rpc/` para los autores del
El portal, los propietarios del SDK y los revisores pueden mejorar los bytes exactos utilizados por CI.

### Enviar una carga útil Norito después de la consola Pruébelo

1. Elija un dispositivo como `fixtures/norito_rpc/transfer_asset.norito`. ces
   los archivos son sobres Norito brutos; **ne** la codificación base64 no.
2. En Swagger o RapiDoc, localice el punto final NRPC (por ejemplo
   `POST /v2/pipeline/submit`) y seleccione el selector **Content-Type** en
   `application/x-norito`.
3. Seleccione el editor de cuerpo en **binario** (modo "Archivo" de Swagger o
   seleccione "Binary/File" de RapiDoc) y cargue el archivo `.norito`. El widget
   Transmita los bytes a través del proxy sin alteración.
4. Soumettez la requete. Si Torii envía `X-Iroha-Error-Code: schema_mismatch`,
   Verifique que llame a un punto final que acepte las cargas binarias y confirme
   que el hash del esquema se registra en `fixtures/norito_rpc/schema_hashes.json`
   corresponden au build Torii cible.La consola conserva el archivo más reciente en memoria para que pueda recuperarlo.
Meme payload tout y testant diferentes tokens de autorización o hosts Torii. Ajouter
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` su paquete de productos de flujo de trabajo
de preuves reference dans le plan d'adoption NRPC-4 (log + resume JSON), ce qui va bien
con la captura de pantalla de la respuesta Pruébelo después de las reseñas.

### Ejemplo de CLI (curvatura)

Les memes fixtures peuvent etre rejoues hors du portail via `curl`, ce qui est utile
Cuando tenga validez el proxy o la puerta de enlace de respuestas:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Reemplace el dispositivo par n'importe quelle entree listee dans `transaction_fixtures.manifest.json`
o codifica tu propia carga útil con `cargo xtask norito-rpc-fixtures`. Cuando Torii está en
modo canario, puedes usar el puntero `curl` frente al proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) para ejercitar la infraestructura del meme
Que utilizan los widgets del portal.

## Observabilidad y operaciones

Chaque requete est loguee une fois avec método, ruta, origen, estatuto aguas arriba y la fuente
de autenticación (`override`, `default` o `client`). Les tokens ne sont jamais stockes: les
headers portador y valores `X-TryIt-Auth` sont rediges avant log, afin que vous puissiez
Relayer stdout vers un colector central sin craindre des fuites.

### Sondas de salud y alertas

Lancez la sonda incluye durante las implementaciones o en un cronograma:```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Perillas de medio ambiente:

- `TRYIT_PROXY_SAMPLE_PATH` - ruta Torii opcional (sin `/proxy`) un deportista.
- `TRYIT_PROXY_SAMPLE_METHOD` - por defecto `GET`; Defina `POST` para las rutas de escritura.
- `TRYIT_PROXY_PROBE_TOKEN`: inyecta un token temporal al portador para la llamada del ejemplo.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - borra el tiempo de espera predeterminado de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - texto de destino Prometheus opcional para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por des virgules ajoutees aux metriques (por defecto `job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional del punto final de mediciones (por ejemplo, `http://localhost:9798/metrics`) que debe responder con éxito cuando `TRYIT_PROXY_METRICS_LISTEN` esté activo.

Inyecte los resultados en un recopilador de archivos de texto apuntando a la sonda frente a un camino grabable
(por ejemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) y junto con las etiquetas
personaliza:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

El guión reescribe el archivo de medidas de facon atomique para que tu coleccionista lise
Siempre hay una carga útil completa.

Cuando `TRYIT_PROXY_METRICS_LISTEN` está configurado y definido
`TRYIT_PROXY_PROBE_METRICS_URL` en el punto final de mediciones para que la sonda haga eco rápidamente si
La superficie de raspado se dispara (por ejemplo, el ingreso está mal configurado o las reglas del firewall manualmente).
Una producción reglamentaria típica.
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.Para leer las alertas, conecte la sonda a su pila de monitoreo. Ejemplo Prometheus
qui page después de dos echecs consecutivos:

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

Definió `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (o todo el host/puerto) antes de
Lanza el proxy para exponer un punto final de medidas en formato Prometheus. El camino
Por defecto está `/metrics` pero se puede reemplazar por `TRYIT_PROXY_METRICS_PATH=/custom`. Chaqué
Scrape renvoie des compteurs des totaux par método, des rechazos rate limit, des erreurs/timeouts
upstream, des results proxy y des resumes de latence:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Pointez vos Collecteurs Prometheus/OTLP vers l'endpoint de metrics et reutilisez les paneles
existentes en `dashboards/grafana/docs_portal.json` afin que SRE observe las latencias de cola
y las fotos de rechazar sin analizador los registros. El proxy público automático
`tryit_proxy_start_timestamp_ms` para ayudar a los operadores a detectar las reestructuraciones.

### Automatización de la reversión

Utilice el asistente de gestión para cada día o restaure la URL correspondiente Torii. El guión
Guarde la configuración anterior en `.env.tryit-proxy.bak` para que las reversiones ocurran
un solo comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Recargue el camino del archivo env con `--env` o `TRYIT_PROXY_ENV` si su implementación
Guarde la configuración de ailleurs.