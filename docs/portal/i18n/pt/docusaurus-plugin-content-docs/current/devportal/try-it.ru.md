---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/try-it.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Песочница Experimente

Портал разработчика включает опциональную консоль "Try it", чтобы вы могли вызывать endpoints Torii, не покидая documentação. Консоль проксирует запросы через встроенный proxy, чтобы браузеры обходили ограничения CORS, одновременно limites de taxa estabelecidos e autenticação.

## Uso antecipado

- Node.js 18.18 ou novo (é necessário construir o portal)
- Сетевой доступ к staging окружению Torii
- token ao portador, который может вызывать нужные маршруты Torii

Seu proxy proxy será configurado automaticamente. Na tabela abaixo, você pode ajustar os botões:

| Variável | Atualizado | Padrão |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL de URL Torii, onde o proxy está configurado para ser executado | **Obrigatório** |
| `TRYIT_PROXY_LISTEN` | Endereço de proxy para distribuição local (formato `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Список origin-ов, которым разрешено обращаться к proxy (через запятую) | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador, fornecido em `X-TryIt-Client` para proteção upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Token de portador para ser usado, transferido para Torii | _vazio_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Разрешить пользователям передавать собственный token через `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Максимальный размер тела запроса (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Tempo limite de upstream em milhões | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Limitação de taxa aberta para cliente IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Скользящее окно limite de taxa (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Endereço de projeto para a métrica Prometheus (`host:port` ou `[ipv6]:port`) | _vazio (desativado)_ |
| `TRYIT_PROXY_METRICS_PATH` | HTTP para a métrica do endpoint | `/metrics` |

O proxy também expõe `GET /healthz`, cria padrões JSON de estrutura e reedita tokens de portador no log.

Selecione `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`, exceto proxy para usar documentos, usar painéis Swagger e RapiDoc para transferir tokens de portador, введенные пользователем. O proxy fornece limites de taxa, редактирует креденшелы e отмечает, использовала ли заявка token para умолчанию ou override на запрос. Instale `TRYIT_PROXY_CLIENT_ID` com um objeto que pode ser usado como `X-TryIt-Client`
(por exemplo `docs-portal`). O proxy é criado e validado `X-TryIt-Client` do cliente e definido como padrão, esses gateways de teste têm origem auditada sem problemas корреляции с метаданными браузера.

## Procurar proxy local

Verifique a configuração antes de abrir o portal:

```bash
cd docs/portal
npm install
```

Abra o proxy e abra sua instância Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

Скрипт логирует адрес binding e proксирует запросы с `/proxy/*` настроенный Torii origin.

Перед vinculativo сокета скрипт проверяет, что
`static/openapi/torii.json` contém resumo, explicado em
`static/openapi/manifest.json`. Se isso acontecer, o comando desativa o dispositivo e executa o comando `npm run sync-openapi -- --latest`. Esporte
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` porta para dispositivos móveis; O proxy abre a operação e executa o trabalho, abrindo sua janela de manutenção.

## Подключение виджетов портала

Para construir ou servir o portal para criar uma URL, você verá os dados disponibilizados para o proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Os componentes adicionais são instalados em `docusaurus.config.js`:

- **Swagger UI** - criado em `/reference/torii-swagger`; esquema de portador de pré-autorização para token, помечает запросы `X-TryIt-Client`, инжектит `X-TryIt-Auth` e переписывает вызовы через proxy, когда `TRYIT_PROXY_PUBLIC_URL` задан.
- **RapiDoc** - отображается на `/reference/torii-rapidoc`; отражает поле token, переиспользует headers Swagger e автоматически направляет через proxy при заданном URL.
- **Experimente console** - встроен в страницу обзора API; позволяет отправлять кастомные запросы, просматривать cabeçalhos e инспектировать тела ответов.

No painel, selecione **seletor de instantâneo**, который читает
`docs/portal/static/openapi/versions.json`. Заполните индекс командой
`npm run sync-openapi -- --version=<label> --mirror=current --latest`, esses revisores podem precisar de um histórico de especificações, ver o resumo SHA-256 e подтверждать, esse lançamento de snapshot tem um manifesto assinado antes de ser usado para vídeos interativos.

Смена token в любом виджете влияет только на текущую сессию браузера; O proxy não é protegido e não registra o token.

## Cortar tokens OAuthEsses tokens não são compatíveis com os revisores dos tokens Torii, por favor, tente consultá-los com o servidor OAuth. Когда переменные окружения ниже присутствуют, portal отображает widget de código de dispositivo, выпускает короткоживущие tokens de portador e автоматически подставляет их в форму консоли.

| Variável | Atualizado | Padrão |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint de autorização de dispositivo OAuth (`/oauth/device/code`) | _vazio (desativado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Endpoint de token, exemplo `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vazio_ |
| `DOCS_OAUTH_CLIENT_ID` | ID do cliente OAuth, зарегистрированный para visualização de documentos | _vazio_ |
| `DOCS_OAUTH_SCOPE` | Escopos, testes de resolução, ajuste por você | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audiência API opcional para token de uso | _vazio_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo de pesquisa mínimo por período de pesquisa (ms) | `5000` (transmissão < 5000 ms de frequência) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Código do dispositivo TTL (segundos) | `600` (é maior que 300 s e 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Token de acesso TTL (segundos) | `900` (é maior que 300 s e 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Configure `1` para visualização local, cujo nome será criado pelo OAuth | _desativar_ |

Configuração de exemplo:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Se você estiver usando `npm run start` ou `npm run build`, conecte-o a `docusaurus.config.js`. Во время локального pré-visualização карточка Experimente показывает кнопку "Entrar com código do dispositivo". Пользователи вводят показанный код на вашей Verificação OAuth странице; после успешного fluxo do dispositivo exibido:

- вставляет выданный token de portador em поле консоли Experimente,
- cabeçalhos добавляет `X-TryIt-Client` e `X-TryIt-Auth`,
- отображает оставшееся время жизни,
- автоматически очищает token при истечении срока.

Ручной ввод Bearer остается доступным - уберите OAuth переменные, если хотите заставить revisores вставлять временный token вручную, ou экспортируйте `DOCS_OAUTH_ALLOW_INSECURE=1` para visualização local de visualização, где анонимный доступ приемлем. Builds sem o uso do OAuth através do padrão, ele usa o gate DOCS-1b.

Nota: Перед публикацией портала за пределами лаборатории проверьте [Lista de verificação de fortalecimento de segurança e pen-test](./security-hardening.md); em nenhum modelo de ameaça especificado, perfil CSP/Trusted Types e testes de pen-test, use o gate DOCS-1b.

## Exemplos Norito-RPC

Norito-RPC é usado para proxy e encanamento OAuth, rotas JSON e; Este é o `Content-Type: application/x-norito` e a capacidade de carga útil do Norito, conforme especificado na especificação NRPC (`docs/source/torii/nrpc_spec.md`). O repositório de cargas úteis canônicas pode ser `fixtures/norito_rpc/`, portais de autores, proprietários e revisores de SDK podem ser solicitados точные bytes, используемые CI.

### Ativando a carga útil Norito do console Try It

1. Fixação de fixação, exemplo `fixtures/norito_rpc/transfer_asset.norito`. Este arquivo contém envelopes Norito; **não** codifique-o em base64.
2. No Swagger ou RapiDoc, selecione o endpoint NRPC (por exemplo, `POST /v1/pipeline/submit`) e selecione o seletor **Content-Type** em `application/x-norito`.
3. Selecione o editor de tela para **binário** (regimente "Arquivo" no Swagger ou "Binário/Arquivo" no RapiDoc) e selecione `.norito` falo. Verifique se os bytes são proxy sem configuração.
4. Abra a tampa. Ou Torii usa `X-Iroha-Error-Code: schema_mismatch`, убедитесь, что вызываете endpoint, принимающий бинарные payloads, и Portanto, esse hash de esquema em `fixtures/norito_rpc/schema_hashes.json` é fornecido com a compilação Torii.

Ao configurar a configuração do arquivo no pacote, você pode usar o payload com tokens de autorização compartilhados ou Anfitriões Torii. `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` adicionado ao fluxo de trabalho contém pacote de evidências, instalado no plano NRPC-4 (log + resumo JSON), que é a solução para isso скриншотом ответа Try It em comentários recentes.

### Exemplo CLI (curl)

Esses fixtures podem ser instalados no portal `curl`, que é usado para testar proxy ou abrir gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

Ative o fixture na entrada principal de `transaction_fixtures.manifest.json` ou conecte seu comando de carga útil `cargo xtask norito-rpc-fixtures`. Se o Torii funcionar na versão canary, você pode usar o `curl` no proxy try-it (`https://docs.sora.example/proxy/v1/pipeline/submit`), isso é provado por você Esta é a infraestrutura, aqui e você pode ver o portal.

## Observabilidade e operaçõesКаждый запрос логируется один раз с método, caminho, origem, status upstream e histórico de autenticação (`override`, `default` ou `client`). Tokens não são suportados - cabeçalhos de portador e значения `X-TryIt-Auth` редактируются перед логированием, поэтому можно Verifique o stdout no coletor central sem arriscar os segredos.

### Sondas de saúde e alertas

Verifique a sonda de teste antes da implantação ou pela descrição:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Oкружения dos botões:

- `TRYIT_PROXY_SAMPLE_PATH` - опциональный Torii fabricante (ou `/proxy`) para teste.
- `TRYIT_PROXY_SAMPLE_METHOD` - para uso `GET`; задайте `POST` para escrever маршрутов.
- `TRYIT_PROXY_PROBE_TOKEN` - cria o token de portador atual para amostra.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - excede o tempo limite para 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - опциональный Prometheus arquivo de texto para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - пары `key=value`, добавляемые к метрикам (por умолчанию `job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - endpoint de métricas de URL opcional (por exemplo `http://localhost:9798/metrics`), который должен отвечать успешно при включенном `TRYIT_PROXY_METRICS_LISTEN`.

Execute os resultados no coletor de arquivo de texto, teste o caminho gravável (por exemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) e adicione rótulos personalizados:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

Скрипт атомарно переписывает файл метрик, чтобы coletor всегда читал полный payload.

Если `TRYIT_PROXY_METRICS_LISTEN` instalado, fechado
`TRYIT_PROXY_PROBE_METRICS_URL` no endpoint de métricas, ele é testado por um padrão para ser usado para raspar (por exemplo, entrada nova ou regras de firewall desativadas). Tipo de produção:
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Para alertar corretamente, você pode testar o probe em sua pilha de monitoramento. Exemplo Prometheus, você pode usar o seguinte padrão:

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

### Endpoint de métricas e painéis

Definindo `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou outro host/porta) através do proxy, ele gera o endpoint de métricas formatado em Prometheus. Se você usar `/metrics`, não será possível alterar o `TRYIT_PROXY_METRICS_PATH=/custom`. O scrape fornece configurações de método, rejeições de limite de taxa, erros/tempos limite de upstream, resultados de proxy e latência de resumo:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Ao instalar coletores Prometheus/OTLP no endpoint de métricas e usar o painel `dashboards/grafana/docs_portal.json`, o SRE pode exibir latência de cauda e всплески отказов без парсинга логов. O proxy é publicado automaticamente `tryit_proxy_start_timestamp_ms`, para que a operação seja reiniciada.

### Reversão automática

Use o auxiliar de gerenciamento para obter ou usar o URL Torii. O script foi configurado para ser configurado em `.env.tryit-proxy.bak`, então esta reversão é o mesmo comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Coloque o env da configuração `--env` ou `TRYIT_PROXY_ENV`, se sua configuração for válida конфигурацию в другом месте.