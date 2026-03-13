---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/try-it.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox, experimente

O portal de desenvolvedores inclui um console opcional "Try it" para que você possa chamar endpoints do Torii sem sair da documentação. O console retransmite requisições pelo proxy embarcado para que os navegadores contornem limites de CORS enquanto ainda aplicam limites de taxa e autenticação.

## Pré-requisitos

- Node.js 18.18 ou mais novo (combina com os requisitos de build do portal)
- Acesso de rede a um ambiente de staging do Torii
- Um bearer token que pode chamar as rotas do Torii que você pretende exercitar

Toda a configuração do proxy e feita por variáveis de ambiente. A tabela abaixo lista os botões mais importantes:

| Variável | Proposta | Padrão |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL base do Torii para qual o proxy encaminha requisições | **Obrigatório** |
| `TRYIT_PROXY_LISTEN` | Endereco de escuta para desenvolvimento local (formato `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por virgula de origens que podem chamar o proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador colocado em `X-TryIt-Client` para cada requisição upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token padrão direcionado ao Torii | _vazio_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permite que usuários finais forneçam seu próprio token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamanho máximo do corpo da requisição (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout upstream em milissegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Requisições permitidas por janela de impostos por IP do cliente | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Janela deslizante para rate limiting (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Endereco de escuta opcional para o endpoint de métricas estilo Prometheus (`host:port` ou `[ipv6]:port`) | _vazio (desativado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Caminho HTTP servido pelo endpoint de métricas | `/metrics` |

O proxy também expoe `GET /healthz`, retorna erros JSON estruturados e mascara bearer tokens nos logs.

Ative `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` exporta o proxy para usuários de documentos para que os usuários Swagger e RapiDoc possam encaminhar tokens de portador fornecidos pelo usuário. O proxy aplica ainda limites de taxas, mascara credenciais e registra se uma requisição usou o token padrão ou um override por requisicao. Configure `TRYIT_PROXY_CLIENT_ID` com o rotulo que você quer enviar como `X-TryIt-Client`
(padrão `docs-portal`). O proxy corta e valida os valores `X-TryIt-Client` fornecidos pelo cliente, retornando para este padrão para que os gateways de staging possam auditar a procedência sem metadados correlacionados do navegador.

## Iniciar o proxy localmente

Instale dependências na primeira vez que configurar o portal:

```bash
cd docs/portal
npm install
```

Rode o proxy e aponte para sua instância Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

O script registra o endereço conectado e encaminha requisições de `/proxy/*` para a origem Torii configurada.

Antes de fazer bind no socket o script valida que
`static/openapi/torii.json` corresponde ao resumo registrado em
`static/openapi/manifest.json`. Se os arquivos divergirem, o comando encerra com erro e
instrui a executar `npm run sync-openapi -- --latest`. Exportar
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` apenas para cancelamentos de emergência; o proxy registra um aviso
e continua para que você possa se recuperar durante as janelas de manutenção.

## Conecte os widgets do portal

Quando você faz build ou serve o portal de desenvolvedores, defina a URL que os widgets devem
usar para proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Os componentes abaixo leem esses valores de `docusaurus.config.js`:

- **Swagger UI** - renderizado em `/reference/torii-swagger`; pré-autoriza o esquema
  bearer quando há um token, marca requisições com `X-TryIt-Client`,
  injeta `X-TryIt-Auth`, e reescreve chamadas pelo proxy quando
  `TRYIT_PROXY_PUBLIC_URL` está configurado.
- **RapiDoc** - renderizado em `/reference/torii-rapidoc`; espelhar o campo de token,
  reutiliza os mesmos cabeçalhos do painel Swagger e aponta para o proxy
  automaticamente quando o URL é configurado.
- **Try it console** - embutido na página de visão geral da API; permite enviar
  requisições personalizadas, ver cabeçalhos e operar corpos de resposta.

Os dois painéis mostram um **seletor de snapshots** que le
`docs/portal/static/openapi/versions.json`. Preencha esse índice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` para que revisores
pode alternar entre especificações históricas, ver o resumo SHA-256 registrado e confirmar se
um snapshot de release carrega um manifesto informado antes de usar os widgets interativos.Mudar o token em qualquer widget relacionado à sessão atual do navegador; o proxy nunca persiste
nem registre o token fornecido.

## Tokens OAuth de curta duração

Para evitar distribuir tokens Torii de longa duração aos revisores, conecte o console Try it ao
seu servidor OAuth. Quando as variáveis de ambiente abaixo estão apresentadas, o portal renderiza
um widget de login com código do dispositivo, gera bearer tokens de curta duração e os injetados
automaticamente no formulário do console.

| Variável | Proposta | Padrão |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint de autorização do dispositivo OAuth (`/oauth/device/code`) | _vazio (desativado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Endpoint de token que aceita `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vazio_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth registrado para visualização de documentos | _vazio_ |
| `DOCS_OAUTH_SCOPE` | Âmbitos separados por espaço solicitado sem login | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Público de API opcional para vincular o token | _vazio_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo mínimo de votação enquanto aguarda aprovação (ms) | `5000` (valores < 5000 ms são rejeitados) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Janela de expiração do código do dispositivo (segundos) | `600` (deve ficar entre 300 e 900) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duração do token de acesso (segundos) | `900` (deve ficar entre 300 e 900) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Defina `1` para visualizações de locais que pulam aplicação OAuth intencionalmente | _desativar_ |

Exemplo de configuração:

```bash
export DOCS_OAUTH_DEVICE_CODE_URL="https://auth.dev.sora.example/oauth/device/code"
export DOCS_OAUTH_TOKEN_URL="https://auth.dev.sora.example/oauth/token"
export DOCS_OAUTH_CLIENT_ID="docs-preview"
export DOCS_OAUTH_SCOPE="torii openid offline_access"
# Optional audience and polling tweaks
export DOCS_OAUTH_AUDIENCE="https://torii.devnet.sora.example"
export DOCS_OAUTH_POLL_INTERVAL_MS="6000"
```

Quando você roda `npm run start` ou `npm run build`, o portal embute esses valores em
`docusaurus.config.js`. Durante uma prévia local o cartão Try it mostra um botao
"Entrar com o código do dispositivo". Os usuários digitam o código mostrado na sua página OAuth; quando o fluxo do dispositivo tem sucesso no widget:

- injetar o bearer token emitido no campo do console Experimente,
- marca requisições com os cabeçalhos existentes `X-TryIt-Client` e `X-TryIt-Auth`,
- exibe o tempo de vida restante, e
- limpa automaticamente o token quando expirar.

A entrada manual Bearer continua disponível; omita as variáveis OAuth quando quiser
forcar reviewers a colar um token temporário por conta própria, ou exportar
`DOCS_OAUTH_ALLOW_INSECURE=1` para pré-visualizações de locais isolados onde acesso anônimo e
aceitavel. Builds sem OAuth configurados agora falham rapidamente para atender o gate do
roteiro DOCS-1b.

Observação: revise uma [lista de verificação de reforço de segurança e pen-test](./security-hardening.md)
antes de exportar o portal fora do laboratório; ela documenta o modelo de ameaça,
o perfil CSP/Trusted Types e os passos de pen-test que agora bloqueiam DOCS-1b.

## Amostras Norito-RPC

Requisições Norito-RPC juntas o mesmo proxy e plumbing OAuth que as rotas JSON;
eles apenas definem `Content-Type: application/x-norito` e enviam o payload Norito
pré-codificado descrito na especificação NRPC
(`docs/source/torii/nrpc_spec.md`).
O repositório inclui payloads canônicos sob `fixtures/norito_rpc/` para que os autores façam
portal, proprietários de SDK e revisores podem reproduzir os bytes exatos que o CI usa.

### Enviar um payload Norito pelo console Try It

1. Escolha um acessório como `fixtures/norito_rpc/transfer_asset.norito`. Esses
   arquivos são envelopes Norito brutos; **nao** faca base64.
2. Em Swagger ou RapiDoc, localize o endpoint NRPC (por exemplo
   `POST /v2/pipeline/submit`) e altere o seletor **Content-Type** para
   `application/x-norito`.
3. Troque o editor de corpo para **binary** (modo "File" do Swagger ou
   seletor "Binary/File" do RapiDoc) e envie o arquivo `.norito`. Ó widget
   transmite os bytes pelo proxy sem alteração.
4. Envie uma requisição. Se o Torii retornar `X-Iroha-Error-Code: schema_mismatch`,
   Verifique se você está chamando um endpoint que aceita payloads binários e confirme
   que o hash do esquema registrado em `fixtures/norito_rpc/schema_hashes.json`
   corresponde ao build do Torii que você está usando.

O console mantém o arquivo mais recente em memória para que você possa reenviar o mesmo
payload enquanto testa diferentes tokens de autorização ou hosts Torii. Adicionar Adicionar
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` ao seu fluxo de trabalho produz o pacote de
 evidência referenciada no plano de adoção NRPC-4 (log + resumo JSON), que combina bem
com capturar screenshots da resposta Experimente durante as avaliações.

### Exemplo CLI (curl)

Os mesmos fixtures podem ser reproduzidos fora do portal via `curl`, o que ajuda
quando você valida o proxy ou depura respostas do gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```Troque o fixture por qualquer entrada listada em `transaction_fixtures.manifest.json`
ou codifique seu próprio payload com `cargo xtask norito-rpc-fixtures`. Quando Torii está
em modo canário você pode indicar o `curl` para o proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) para treinar a mesma
infraestrutura usada pelos widgets do portal.

## Observabilidade e operações

Cada requisição e registro uma vez com método, caminho, origem, status upstream e a fonte
de autenticação (`override`, `default` ou `client`). Tokens nunca são armazenados: tanto
os headers bearer quanto os valores `X-TryIt-Auth` são redigidos antes do log,
então você pode encaminhar stdout para um coletor central sem se preocupar com vazamentos.

### Sondas de saúde e alertas

Rodou a sonda incluída durante implantações ou em um cronograma:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Botões de ambiente:

- `TRYIT_PROXY_SAMPLE_PATH` - rota Torii opcional (sem `/proxy`) para exercício.
- `TRYIT_PROXY_SAMPLE_METHOD` - padrão `GET`; define `POST` para rotas de escrita.
- `TRYIT_PROXY_PROBE_TOKEN` - injeção temporária de token de portador para chamada de amostra.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - ultrapassa o intervalo de tempo limite de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destino de texto Prometheus opcional para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por virgula anexada às métricas (padrão `job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional do endpoint de métricas (por exemplo, `http://localhost:9798/metrics`) que deve responder com sucesso quando `TRYIT_PROXY_METRICS_LISTEN` estiver habilitado.

Alimente os resultados em um coletor de arquivo de texto apontando a sonda para um caminho gravevel
(por exemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) e adicionando etiquetas
personalizados:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

O script reescreve o arquivo de métricas de forma atômica para que seu coletor sempre leia
uma carga útil completa.

Quando `TRYIT_PROXY_METRICS_LISTEN` estiver configurado, defina
`TRYIT_PROXY_PROBE_METRICS_URL` para o endpoint de métricas para que o probe falhe rapidamente se a
superfície de scrape desaparece (por exemplo, ingresso mal configurado ou regras de firewall ausentes).
Um ajuste típico de produção e
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Para níveis de alerta, conecte a sonda à sua pilha de monitoramento. Exemplo Prometheus que
página após duas falhas consecutivas:

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

### Endpoint de métricas e dashboards

Defina `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou qualquer par host/porta) antes de
inicie o proxy para exportar um endpoint de métricas no formato Prometheus. O caminho
padrão e `/metrics` mas pode ser escrito via
`TRYIT_PROXY_METRICS_PATH=/custom`. Cada scrape retorna contadores de totais por método,
rejeições por limite de taxa, erros/timeouts upstream, resultados do proxy e resumos de latência:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Aponte seus coletores Prometheus/OTLP para o endpoint de métricas e reutilizar os painéis
existentes em `dashboards/grafana/docs_portal.json` para que SRE observe latências de cauda
e picos de rejeição sem analisar logs. O proxy publica automaticamente `tryit_proxy_start_timestamp_ms`
para ajudar os operadores a detectar reinícios.

### Automação de rollback

Use o helper de gerenciamento para atualizar ou restaurar uma URL alvo do Torii. Ó roteiro
armazena a configuração anterior em `.env.tryit-proxy.bak` para que rollbacks sejam um
comando único.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrescreva o caminho do arquivo env com `--env` ou `TRYIT_PROXY_ENV` se sua implantação
armazenar a configuração em outro lugar.