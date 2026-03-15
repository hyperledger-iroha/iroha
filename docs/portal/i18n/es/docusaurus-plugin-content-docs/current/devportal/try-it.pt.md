---
lang: es
direction: ltr
source: docs/portal/docs/devportal/try-it.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox, pruébalo

El portal de desarrolladores incluye una consola opcional "Pruébalo" para que puedas seleccionar los puntos finales de Torii sin documentar. La consola retransmite los requisitos del proxy embarcado para que los navegadores controlen los límites de CORS mientras aplica límites de velocidad y autenticación.

##Requisitos previos

- Node.js 18.18 o más nuevo (combina con los requisitos de construcción del portal)
- Acceso a la red en un ambiente de puesta en escena de Torii
- Um portador token que possa chamar as rotas do Torii que voce pretender ejercitar

Toda la configuración del proxy y la configuración varían de ambiente. A continuación se presenta una lista de los mandos más importantes:| Variables | propuesta | Predeterminado |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL base de Torii para cual proxy encamina los requisitos | **Obligatorio** |
| `TRYIT_PROXY_LISTEN` | Endereco de escudo para desarrollo local (formato `host:port` o `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por virgula de origens que podem chamar o proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador colocado en `X-TryIt-Client` para cada requisicao upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Token al portador padrao encaminhado ao Torii | _vacío_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permite que los usuarios finais fornecam su propio token a través de `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamanho maximo do corpo da requisicao (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Tiempo de espera aguas arriba en milissegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Requisitos permitidos por janela de taxa por IP do cliente | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Janela deslizante para limitación de velocidad (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Endereco de escudo opcional para el punto final de métricas estilo Prometheus (`host:port` o `[ipv6]:port`) | _vacío (deshabilitado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Camino HTTP servido por punto final de métricas | `/metrics` |

El proxy también expone `GET /healthz`, retorna errores estructurados JSON y tokens portadores de máscara en registros.Ative `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` ao export o proxy para usuarios de docs para que os Paineis Swagger y RapiDoc possam encaminhar tokens de portador fornecidos por el usuario. El proxy también aplica límites de taxones, las credenciales de máscara y el registro se utilizan para usar un token padrao o una anulación por requisito. Configure `TRYIT_PROXY_CLIENT_ID` con el rotulo que voce querer enviar como `X-TryIt-Client`
(padrón `docs-portal`). O proxy corta y valida valores `X-TryIt-Client` fornecidos pelo cliente, voltando para este default para que os gateways de staging possam auditar a procedencia sem correlacionar metadados do navegador.

## Iniciar proxy localmente

Instale dependencias na primera vez que configure el portal:

```bash
cd docs/portal
npm install
```

Rode o proxy y apote para su instancia Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

El script registra el endereco ligado y encamina los requisitos de `/proxy/*` para un origen Torii configurado.

Antes de hacer bind no socket o script valida que
`static/openapi/torii.json` corresponde al resumen registrado en
`static/openapi/manifest.json`. Se os arquivos divergirem, o comando encerrado com error e
Instrumento ejecutante `npm run sync-openapi -- --latest`. exportar
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` apenas para anulaciones de emergencia; o proxy registra um aviso
y continúa para que voce possa se recupere durante janelas de manutencao.

## Conecte los widgets al portalCuando usted hace la construcción o servicio del portal de desarrolladores, defina una URL que los widgets devem
usar para el proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Os componentes abaixo leem esses valores de `docusaurus.config.js`:

- **Swagger UI** - renderizado en `/reference/torii-swagger`; preautoriza el esquema
  portador cuando ha um token, marca requisicoes com `X-TryIt-Client`,
  Injeta `X-TryIt-Auth`, y reescreve chamadas por proxy quando
  `TRYIT_PROXY_PUBLIC_URL` está configurado.
- **RapiDoc** - renderizado en `/reference/torii-rapidoc`; espelha o campo de token,
  reutilizar los encabezados de mesmos del dolor Swagger y aponta para el proxy
  automáticamente cuando una URL está configurada.
- **Pruébalo en la consola** - incorporado en la página de descripción general de la API; permite enviar
  requisicoes personalizados, ver encabezados e inspeccionar cuerpos de respuesta.

Os dois Paineis mostram um **selector de instantáneas** que le
`docs/portal/static/openapi/versions.json`. Preencha esse índice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` para que revisores
Possam alternar entre especificaciones históricas, ver o digerir SHA-256 registrado y confirmar si
um snapshot de release carrega um manifest assinado antes de usar los widgets interactivos.

Mudar o token em qualquer widget para que afecte la sesión actual del navegador; El proxy nunca persiste.
nem registra o token fornecido.

## Tokens OAuth de corta duraciónPara evitar distribuir tokens Torii de larga duración para los revisores, conecte la consola Pruébelo también
su servidor OAuth. Quando as variaveis de ambiente abaixo this o present, o portal renderiza
un widget de inicio de sesión con código de dispositivo, tokens de portador de gera de curta duracao e os injeta
automáticamente no hay formulario en la consola.

| Variables | propuesta | Predeterminado |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Punto final de autorización de dispositivo OAuth (`/oauth/device/code`) | _vacío (deshabilitado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Punto final de token que aceita `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vacío_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth registrado para o vista previa de documentos | _vacío_ |
| `DOCS_OAUTH_SCOPE` | Ámbitos separados por espacio solicitados sin iniciar sesión | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audiencia de API opcional para vincular o token | _vacío_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo minimo de polling enquanto aguarda aprovacao (ms) | `5000` (valores < 5000 ms sao rejeitados) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Janela de expiracao do código del dispositivo (segundos) | `600` (debe fijar entre 300 s y 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duracao do token de acceso (segundos) | `900` (debe fijar entre 300 s y 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Defina `1` para obtener vistas previas localizadas que aplican OAuth intencionalmente | _desarmado_ |

Ejemplo de configuración:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```Cuando la voz gira `npm run start` o `npm run build`, el portal embute esos valores en
`docusaurus.config.js`. Durante una vista previa de la tarjeta local Pruébalo mostra um botao
"Iniciar sesión con el código del dispositivo". Los usuarios digitan el código mostrado en su página OAuth; Cuando el flujo del dispositivo es exitoso o widget:

- injeta o token al portador emitido en el campo de la consola Pruébalo,
- marca requerida con los encabezados existentes `X-TryIt-Client` e `X-TryIt-Auth`,
- exibe o tempo de vida restante, e
- Limpia automáticamente el token cuando expira.

A entrada manual Portador continua disponivel; omita como variaveis OAuth cuando quiera
forcar reviewers a colar un token temporal por cuenta propia o exportable
`DOCS_OAUTH_ALLOW_INSECURE=1` para vistas previas de lugares aislados y acceso anónimo
aceitavel. Construye sin OAuth configurado ahora falham rapido para atender o gate do
hoja de ruta DOCS-1b.

Nota: Revisar una [Lista de verificación de pruebas de penetración y refuerzo de seguridad](./security-hardening.md)
antes de exportar o portal fora do laboratorio; ela documenta o modelo de amenaza,
El perfil CSP/Trusted Types y los pasos de pen-test que ahora bloquean DOCS-1b.

##Amostras Norito-RPCRequisitos Norito-RPC que comparte el proxy y la plomería OAuth como JSON;
Eles apenas definen `Content-Type: application/x-norito` y envían la carga útil Norito
precodificado descrito en la especificación NRPC
(`docs/source/torii/nrpc_spec.md`).
El repositorio incluye cargas útiles canónicas sob `fixtures/norito_rpc/` para que autores hagan
portal, los propietarios del SDK y los revisores pueden reproducir los bytes exactos que el CI usa.

### Enviar una carga útil Norito a la consola Pruébalo

1. Seleccione un dispositivo como `fixtures/norito_rpc/transfer_asset.norito`. eses
   arquivos sao sobres Norito brutos; **nao** faca base64.
2. En Swagger o RapiDoc, localice el punto final NRPC (por ejemplo
   `POST /v2/pipeline/submit`) y altere el selector **Content-Type** para
   `application/x-norito`.
3. Troque o editor de corpo para **binary** (modo "File" de Swagger o
   selector "Binary/File" de RapiDoc) y envíe el archivo `.norito`. Oh widget
   transmite los bytes por proxy sin alteración.
4. Envidia un requisicao. Se o Torii retornar `X-Iroha-Error-Code: schema_mismatch`,
   verifique se voce esta chamando um endpoint que aceita payloads binarios y confirme
   que hash de esquema registrado en `fixtures/norito_rpc/schema_hashes.json`
   corresponde ao build do Torii que voce esta usando.El mantem de la consola o el archivo más reciente en memoria para que voce possa reenviar o mesmo
payload enquanto testa diferentes tokens de autorización o hosts Torii. Agregar
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` ao su flujo de trabajo produce o paquete de
 evidencia referenciada en el plano de adopción NRPC-4 (log + resumo JSON), que combina bien
com capturar capturas de pantalla da respuesta Pruébelo durante las revisiones.

### Ejemplo CLI (curl)

Os mesmos fixtures podem ser reproducidos fora do portal via `curl`, o que ajuda
Cuando la voz valida el proxy o depura las respuestas de la puerta de enlace:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Troque o accesorio por cualquier entrada listada en `transaction_fixtures.manifest.json`
o codifique su propia carga útil con `cargo xtask norito-rpc-fixtures`. Cuando Torii esta
En modo canary voice puedes poner el `curl` para el proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) para ejercitar a mesma
Infraestrutura usada pelos widgets del portal.

## Observabilidade e operacoes

Cada solicitud y registro una vez con método, ruta, origen, estado aguas arriba y fuente
de autenticacao (`override`, `default` o `client`). Tokens nunca sao armazenados: tanto
os headers portador quanto os valores `X-TryIt-Auth` sao redigidos antes del registro,
Entao voce pode encaminhar stdout para un colector central sin preocuparse por los vazamentos.

### Sondas de saude y alertas

Rode o probe incluido durante los despliegues o en un horario:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Perillas de ambiente:- `TRYIT_PROXY_SAMPLE_PATH` - gira Torii opcional (sem `/proxy`) para ejercitar.
- `TRYIT_PROXY_SAMPLE_METHOD` - padrao `GET`; define `POST` para rotas de escrita.
- `TRYIT_PROXY_PROBE_TOKEN` - injeta un token al portador temporal para a chamada de amostra.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - sobrescreve o timeout padrao de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destino de texto Prometheus opcional para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por virgula anexados as métricas (padrao `job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional del punto final de métricas (por ejemplo, `http://localhost:9798/metrics`) que debe responder con éxito cuando `TRYIT_PROXY_METRICS_LISTEN` está habilitado.

Todos los resultados en un recopilador de archivos de texto colocando la sonda para un camino gravavel
(por ejemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) y agregando etiquetas
personalizados:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

O script reescreve o archivo de métricas de forma atómica para que su coleccionista semper leia
um carga útil completa.

Cuando `TRYIT_PROXY_METRICS_LISTEN` está configurado, define
`TRYIT_PROXY_PROBE_METRICS_URL` para el punto final de métricas para que la sonda falle rápido se a
La superficie de raspado desaparecerá (por ejemplo, ingreso mal configurado o registros de firewall ausentes).
Un ajuste típico de producción e
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Para alertas leves, conecte la sonda a su pila de monitoreo. Ejemplo Prometheus que
pagina apos duas falhas consecutivas:

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
```### Punto final de métricas y paneles

Defina `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou qualquer par host/porta) antes de
Iniciar el proxy para exportar un punto final de métricas sin formato Prometheus. Oh camino
padrao e `/metrics` mas pode ser escrito vía
`TRYIT_PROXY_METRICS_PATH=/custom`. Cada scrape retorna contadores de todos por método,
Rechazados por límite de velocidad, errores/tiempos de espera ascendentes, resultados del proxy y resúmenes de latencia:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Aponte sus colectores Prometheus/OTLP para el punto final de métricas y reutilizar los paneles del sistema operativo
existente en `dashboards/grafana/docs_portal.json` para que SRE observe latencias de cauda
e picos de rejeicao sin analizar registros. El proxy público se publica automáticamente `tryit_proxy_start_timestamp_ms`
para ayudar a los operadores a detectar reinicios.

### Reversión automática

Utilice el asistente de gestión para actualizar o restaurar una URL al igual que Torii. Oh guión
Armazena a configuracao anterior em `.env.tryit-proxy.bak` para que rollbacks sejam um
único comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrescreva o caminho do archivo env com `--env` o `TRYIT_PROXY_ENV` se sua implantacao
Armazenar a configuracao em outro lugar.