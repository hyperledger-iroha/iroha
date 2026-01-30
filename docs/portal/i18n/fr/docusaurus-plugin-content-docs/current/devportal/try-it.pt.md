---
lang: fr
direction: ltr
source: docs/portal/docs/devportal/try-it.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
---

# Sandbox do Try It

O portal de desenvolvedores inclui um console opcional "Try it" para que voce possa chamar endpoints do Torii sem sair da documentacao. O console retransmite requisicoes pelo proxy embarcado para que os navegadores contornem limites de CORS enquanto ainda aplicam rate limits e autenticacao.

## Prerequisitos

- Node.js 18.18 ou mais novo (combina com os requisitos de build do portal)
- Acesso de rede a um ambiente de staging do Torii
- Um bearer token que possa chamar as rotas do Torii que voce pretende exercitar

Toda a configuracao do proxy e feita por variaveis de ambiente. A tabela abaixo lista os knobs mais importantes:

| Variable | Proposito | Default |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL base do Torii para a qual o proxy encaminha requisicoes | **Required** |
| `TRYIT_PROXY_LISTEN` | Endereco de escuta para desenvolvimento local (formato `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por virgula de origens que podem chamar o proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador colocado em `X-TryIt-Client` para cada requisicao upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token padrao encaminhado ao Torii | _empty_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permite que usuarios finais fornecam seu proprio token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamanho maximo do corpo da requisicao (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout upstream em milissegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Requisicoes permitidas por janela de taxa por IP do cliente | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Janela deslizante para rate limiting (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Endereco de escuta opcional para o endpoint de metricas estilo Prometheus (`host:port` ou `[ipv6]:port`) | _empty (disabled)_ |
| `TRYIT_PROXY_METRICS_PATH` | Caminho HTTP servido pelo endpoint de metricas | `/metrics` |

O proxy tambem expoe `GET /healthz`, retorna erros JSON estruturados e mascara bearer tokens nos logs.

Ative `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` ao expor o proxy para usuarios de docs para que os paineis Swagger e RapiDoc possam encaminhar bearer tokens fornecidos pelo usuario. O proxy ainda aplica limites de taxa, mascara credenciais e registra se uma requisicao usou o token padrao ou um override por requisicao. Configure `TRYIT_PROXY_CLIENT_ID` com o rotulo que voce quer enviar como `X-TryIt-Client`
(padrao `docs-portal`). O proxy corta e valida valores `X-TryIt-Client` fornecidos pelo cliente, voltando para este default para que os gateways de staging possam auditar a procedencia sem correlacionar metadados do navegador.

## Inicie o proxy localmente

Instale dependencias na primeira vez que configurar o portal:

```bash
cd docs/portal
npm install
```

Rode o proxy e aponte para sua instancia Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

O script registra o endereco ligado e encaminha requisicoes de `/proxy/*` para a origem Torii configurada.

Antes de fazer bind no socket o script valida que
`static/openapi/torii.json` corresponde ao digest registrado em
`static/openapi/manifest.json`. Se os arquivos divergirem, o comando encerra com erro e
instrui a executar `npm run sync-openapi -- --latest`. Exporte
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` apenas para overrides de emergencia; o proxy registra um aviso
e continua para que voce possa se recuperar durante janelas de manutencao.

## Conecte os widgets do portal

Quando voce faz build ou serve o portal de desenvolvedores, defina a URL que os widgets devem
usar para o proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Os componentes abaixo leem esses valores de `docusaurus.config.js`:

- **Swagger UI** - renderizado em `/reference/torii-swagger`; preautoriza o esquema
  bearer quando ha um token, marca requisicoes com `X-TryIt-Client`,
  injeta `X-TryIt-Auth`, e reescreve chamadas pelo proxy quando
  `TRYIT_PROXY_PUBLIC_URL` esta configurado.
- **RapiDoc** - renderizado em `/reference/torii-rapidoc`; espelha o campo de token,
  reutiliza os mesmos headers do painel Swagger e aponta para o proxy
  automaticamente quando a URL esta configurada.
- **Try it console** - embutido na pagina de overview da API; permite enviar
  requisicoes personalizadas, ver headers e inspecionar corpos de resposta.

Os dois paineis mostram um **seletor de snapshots** que le
`docs/portal/static/openapi/versions.json`. Preencha esse indice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` para que reviewers
possam alternar entre specs historicas, ver o digest SHA-256 registrado e confirmar se
um snapshot de release carrega um manifest assinado antes de usar os widgets interativos.

Mudar o token em qualquer widget so afeta a sessao atual do navegador; o proxy nunca persiste
nem registra o token fornecido.

## Tokens OAuth de curta duracao

Para evitar distribuir tokens Torii de longa duracao aos reviewers, conecte o console Try it ao
seu servidor OAuth. Quando as variaveis de ambiente abaixo estao presentes, o portal renderiza
um widget de login com device code, gera bearer tokens de curta duracao e os injeta
automaticamente no formulario do console.

| Variable | Proposito | Default |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint de autorizacao de dispositivo OAuth (`/oauth/device/code`) | _empty (disabled)_ |
| `DOCS_OAUTH_TOKEN_URL` | Endpoint de token que aceita `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _empty_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth registrado para o preview de docs | _empty_ |
| `DOCS_OAUTH_SCOPE` | Scopes separados por espaco solicitados no login | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audience de API opcional para vincular o token | _empty_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo minimo de polling enquanto aguarda aprovacao (ms) | `5000` (valores < 5000 ms sao rejeitados) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Janela de expiracao do device code (segundos) | `600` (deve ficar entre 300 s e 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duracao do access token (segundos) | `900` (deve ficar entre 300 s e 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Defina `1` para previews locais que pulam enforcement OAuth intencionalmente | _unset_ |

Exemplo de configuracao:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Quando voce roda `npm run start` ou `npm run build`, o portal embute esses valores em
`docusaurus.config.js`. Durante um preview local o card Try it mostra um botao
"Sign in with device code". Os usuarios digitam o codigo mostrado na sua pagina OAuth; quando o device flow tem sucesso o widget:

- injeta o bearer token emitido no campo do console Try it,
- marca requisicoes com os headers existentes `X-TryIt-Client` e `X-TryIt-Auth`,
- exibe o tempo de vida restante, e
- limpa automaticamente o token quando expira.

A entrada manual Bearer continua disponivel; omita as variaveis OAuth quando quiser
forcar reviewers a colar um token temporario por conta propria, ou exporte
`DOCS_OAUTH_ALLOW_INSECURE=1` para previews locais isoladas onde acesso anonimo e
aceitavel. Builds sem OAuth configurado agora falham rapido para atender o gate do
roadmap DOCS-1b.

Nota: Revise a [Security hardening & pen-test checklist](./security-hardening.md)
antes de expor o portal fora do laboratorio; ela documenta o threat model,
o perfil CSP/Trusted Types e os passos de pen-test que agora bloqueiam DOCS-1b.

## Amostras Norito-RPC

Requisicoes Norito-RPC compartilham o mesmo proxy e plumbing OAuth que as rotas JSON;
eles apenas definem `Content-Type: application/x-norito` e enviam o payload Norito
pre-encodado descrito na especificacao NRPC
(`docs/source/torii/nrpc_spec.md`).
O repositorio inclui payloads canonicos sob `fixtures/norito_rpc/` para que autores do
portal, owners de SDK e reviewers possam reproduzir os bytes exatos que o CI usa.

### Enviar um payload Norito pelo console Try It

1. Escolha um fixture como `fixtures/norito_rpc/transfer_asset.norito`. Esses
   arquivos sao envelopes Norito brutos; **nao** faca base64.
2. Em Swagger ou RapiDoc, localize o endpoint NRPC (por exemplo
   `POST /v1/pipeline/submit`) e altere o seletor **Content-Type** para
   `application/x-norito`.
3. Troque o editor de corpo para **binary** (modo "File" do Swagger ou
   seletor "Binary/File" do RapiDoc) e envie o arquivo `.norito`. O widget
   transmite os bytes pelo proxy sem alteracao.
4. Envie a requisicao. Se o Torii retornar `X-Iroha-Error-Code: schema_mismatch`,
   verifique se voce esta chamando um endpoint que aceita payloads binarios e confirme
   que o schema hash registrado em `fixtures/norito_rpc/schema_hashes.json`
   corresponde ao build do Torii que voce esta usando.

O console mantem o arquivo mais recente em memoria para que voce possa reenviar o mesmo
payload enquanto testa diferentes tokens de autorizacao ou hosts Torii. Adicionar
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` ao seu workflow produz o bundle de
 evidencia referenciado no plano de adocao NRPC-4 (log + resumo JSON), que combina bem
com capturar screenshots da resposta Try It durante reviews.

### Exemplo CLI (curl)

Os mesmos fixtures podem ser reproduzidos fora do portal via `curl`, o que ajuda
quando voce valida o proxy ou depura respostas do gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

Troque o fixture por qualquer entrada listada em `transaction_fixtures.manifest.json`
ou codifique seu proprio payload com `cargo xtask norito-rpc-fixtures`. Quando Torii esta
em modo canary voce pode apontar o `curl` para o proxy try-it
(`https://docs.sora.example/proxy/v1/pipeline/submit`) para exercitar a mesma
infraestrutura usada pelos widgets do portal.

## Observabilidade e operacoes

Cada requisicao e registrada uma vez com metodo, path, origem, status upstream e a fonte
de autenticacao (`override`, `default` ou `client`). Tokens nunca sao armazenados: tanto
os headers bearer quanto os valores `X-TryIt-Auth` sao redigidos antes do log,
entao voce pode encaminhar stdout para um collector central sem se preocupar com vazamentos.

### Probes de saude e alertas

Rode o probe incluido durante deploys ou em um schedule:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Knobs de ambiente:

- `TRYIT_PROXY_SAMPLE_PATH` - rota Torii opcional (sem `/proxy`) para exercitar.
- `TRYIT_PROXY_SAMPLE_METHOD` - padrao `GET`; defina `POST` para rotas de escrita.
- `TRYIT_PROXY_PROBE_TOKEN` - injeta um bearer token temporario para a chamada de amostra.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - sobrescreve o timeout padrao de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destino de texto Prometheus opcional para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por virgula anexados as metricas (padrao `job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional do endpoint de metricas (por exemplo, `http://localhost:9798/metrics`) que deve responder com sucesso quando `TRYIT_PROXY_METRICS_LISTEN` esta habilitado.

Alimente os resultados em um textfile collector apontando o probe para um caminho gravavel
(por exemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) e adicionando labels
customizados:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

O script reescreve o arquivo de metricas de forma atomica para que seu collector sempre leia
um payload completo.

Quando `TRYIT_PROXY_METRICS_LISTEN` esta configurado, defina
`TRYIT_PROXY_PROBE_METRICS_URL` para o endpoint de metricas para que o probe falhe rapido se a
superficie de scrape desaparecer (por exemplo, ingress mal configurado ou regras de firewall ausentes).
Um ajuste tipico de production e
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Para alertas leves, conecte o probe ao seu stack de monitoramento. Exemplo Prometheus que
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
```

### Endpoint de metricas e dashboards

Defina `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou qualquer par host/porta) antes de
iniciar o proxy para expor um endpoint de metricas no formato Prometheus. O caminho
padrao e `/metrics` mas pode ser sobrescrito via
`TRYIT_PROXY_METRICS_PATH=/custom`. Cada scrape retorna contadores de totais por metodo,
rejeicoes por rate limit, erros/timeouts upstream, resultados do proxy e resumos de latencia:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Aponte seus collectors Prometheus/OTLP para o endpoint de metricas e reutilize os panels
existentes em `dashboards/grafana/docs_portal.json` para que SRE observe latencias de cauda
e picos de rejeicao sem parsear logs. O proxy publica automaticamente `tryit_proxy_start_timestamp_ms`
para ajudar operadores a detectar reinicios.

### Automacao de rollback

Use o helper de gerenciamento para atualizar ou restaurar a URL alvo do Torii. O script
armazena a configuracao anterior em `.env.tryit-proxy.bak` para que rollbacks sejam um
unico comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrescreva o caminho do arquivo env com `--env` ou `TRYIT_PROXY_ENV` se sua implantacao
armazenar a configuracao em outro lugar.
