---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/try-it.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Bac a sable Experimente

O portal de desenvolvimento fornece uma opção de console "Try it" para chamar os endpoints Torii sem sair da documentação. O console transmite as solicitações por meio do proxy de embarque para que os navegadores contornem os limites do CORS, aplicando a limitação de taxa e a autenticação.

## Pré-requisito

- Node.js 18.18 ou mais recente (corresponde às exigências de construção do portal)
- Acesse o recurso em um ambiente de teste Torii
- Um token de portador capaz de chamar as rotas Torii que você deseja testar

Execute a configuração do proxy através das variáveis de ambiente. O quadro acima lista os botões mais importantes:

| Variável | Objetivo | Padrão |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL Torii de base para o proxy que transmite as solicitações | **Obrigatório** |
| `TRYIT_PROXY_LISTEN` | Endereço de eco para desenvolvimento local (formato `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada pelas virgules de origem autorizadas para chamar o proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador local em `X-TryIt-Client` para cada recepção upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Token de portador par padrão relaie vers Torii | _vazio_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permitir que usuários financiem o fornecimento de seu próprio token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamanho máximo do corpo de recebimento (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Tempo limite upstream em milissegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Requetes autorizadas por criptografia de dados por cliente IP | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Feixe brilhante para limitação de taxa (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Endereço de ecoute opcional para o ponto final de métricas estilo Prometheus (`host:port` ou `[ipv6]:port`) | _vazio (desativado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Caminho HTTP serviço por endpoint de métricas | `/metrics` |

O proxy expõe também `GET /healthz`, envia estruturas JSON erradas e mascara tokens de portador nos logs.

Activez `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` quando você expõe o proxy aos documentos dos usuários para que os painéis Swagger e RapiDoc possam retransmitir tokens de portador fornecidos pelo usuário. O proxy aplica-se sempre aos limites de taxas, mascara os credenciais e registra-se em caso de solicitação para utilizar o token por padrão ou uma sobretaxa por solicitação. Configure `TRYIT_PROXY_CLIENT_ID` com a mensagem que você deseja enviar como `X-TryIt-Client`
(par padrão `docs-portal`). O proxy troque e valide os valores `X-TryIt-Client` quatros pelo recorrente, e então retome o padrão para que os gateways de teste possam auditar a proveniência sem correlacionar os metadonnees do navegador.

## Demarrer o proxy no local

Instale as dependências da configuração inicial do portal:

```bash
cd docs/portal
npm install
```

Lance o proxy e aponte para sua instância Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

O script faz login no endereço e retransmite as solicitações de `/proxy/*` para a origem Torii configurada.

Antes de vincular o soquete, o script é válido
`static/openapi/torii.json` corresponde ao resumo registrado em
`static/openapi/manifest.json`. Se os arquivos divergem, o comando ecoa com um
erro e você exigiu o executor `npm run sync-openapi -- --latest`. Exportar
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` exclusivo para substituições de emergência; o log do proxy
um aviso e continue para que você possa recuperar as janelas de manutenção.

## Cablagem de widgets do portal

Quando você construir ou servir o portal de desenvolvimento, defina o URL que os widgets fornecem
usuário para o proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Os seguintes componentes licenciam estes valores a partir de `docusaurus.config.js`:

- **Swagger UI** - gerado por `/reference/torii-swagger`; pré-autorizar o portador do esquema
  Quando um token estiver presente, marque as solicitações com `X-TryIt-Client`, injete
  `X-TryIt-Auth`, e reecreva os apelos via proxy quando
  `TRYIT_PROXY_PUBLIC_URL` é definido.
- **RapiDoc** - obtido por `/reference/torii-rapidoc`; reflete o token do campeão,
  reutilizar os cabeçalhos dos memes que aparecem no Swagger e cibelizá-los automaticamente
  o proxy quando o URL é configurado.
- **Experimente console** - integrado na página de visão geral da API; permitir o envio de
  receba solicitações personalizadas, veja os cabeçalhos e inspecione o corpo de resposta.Os dois painéis exibem um **seletor de instantâneos** que acende
`docs/portal/static/openapi/versions.json`. Resolva este índice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` após os revisores
pode passar entre as especificações históricas, ver o resumo SHA-256 registrado e confirmado
Se um instantâneo de lançamento embarcar em um manifesto antes de usar os widgets interativos.

Altere o token em um widget e não toque na sessão do navegador atual; o proxy não
persiste jamais et ne logue jamais le token fourni.

## Tokens OAuth cortesia

Para evitar distribuir tokens Torii durante muito tempo com revisores, confie no console Experimente um
seu servidor OAuth. Quando as variáveis ambientais ci-dessous são apresentadas, o portal
renderiza um widget de código de dispositivo de login, emite tokens de portador com cortesia e os injeta
automaticamente no formulário do console.

| Variável | Objetivo | Padrão |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint de dispositivo de autorização OAuth (`/oauth/device/code`) | _vazio (desativado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Endpoint token que aceita `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vazio_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth registrado para pré-visualização de documentos | _vazio_ |
| `DOCS_OAUTH_SCOPE` | Escopos delimitados pelos espaços exigidos para login | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Opções de API de público para obter o token | _vazio_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo mínimo de votação pendente da atenção de aprovação (ms) | `5000` (valores < 5000 ms rejeitados) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Código do dispositivo de expiração (segundos) | `600` (doit rester entre 300 s e 900 s) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Token de acesso Duree de vie (segundos) | `900` (doit rester entre 300 s e 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Mettre `1` para pré-visualizações de locais que executam a aplicação intencional do OAuth | _desativar_ |

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

Quando você lança `npm run start` ou `npm run build`, o portal integra esses valores
em `docusaurus.config.js`. Na pré-visualização do local da carta Experimente exibir um botão
"Entrar com o código do dispositivo". Os usuários enviam o código para sua página OAuth; Uma vez que o fluxo do dispositivo reutilizou o widget:

- injete o token do portador emis no console do campeão Experimente,
- marque as solicitações com os cabeçalhos existentes `X-TryIt-Client` e `X-TryIt-Auth`,
- exibir o tempo restante, et
- apagar automaticamente o token quando ele expirar.

A entrada Manuelle Bearer está disponível; remova as variáveis ​​OAuth quando você quiser
forçar os revisores a coletar um token temporário eux-memes, ou exportá-los
`DOCS_OAUTH_ALLOW_INSECURE=1` para visualizações de locais isolados ou acesso anônimo est
aceitável. As compilações sem OAuth configuram a manutenção rápida para satisfação
para satisfazer o portão do roteiro DOCS-1b.

Nota: Consulte a [Lista de verificação de fortalecimento de segurança e pen-test](./security-hardening.md)
antes de expor o portal fora do laboratório; ela documentou o modelo de ameaça,
o perfil CSP/Trusted Types e as etapas de pen-test que bloqueiam a manutenção do DOCS-1b.

## Exemplos Norito-RPC

As solicitações Norito-RPC compartilham o meme proxy e o encanamento OAuth nas rotas JSON;
Eles apresentam simplesmente `Content-Type: application/x-norito` e enviam a carga útil Norito
pré-codificar decrit na especificação NRPC
(`docs/source/torii/nrpc_spec.md`).
O depósito fornece cargas úteis canônicas sob `fixtures/norito_rpc/` para que os autores du
portail, proprietários SDK e revisores podem recuperar os bytes exatos utilizados por CI.

### Envie uma carga útil Norito a partir do console Try It

1. Escolha um acessório como `fixtures/norito_rpc/transfer_asset.norito`. Ces
   os arquivos são envelopes Norito brutos; **ne** a codificação em base64 não é válida.
2. No Swagger ou RapiDoc, localize o endpoint NRPC (por exemplo
   `POST /v2/pipeline/submit`) e pressione o seletor **Content-Type** em
   `application/x-norito`.
3. Selecione o editor de corpo em **binário** (modo "Arquivo" de Swagger ou
   selecione "Binary/File" do RapiDoc) e carregue o arquivo `.norito`. O widget
   transmite os bytes através do proxy sem alteração.
4. Adicione a receita. Si Torii reenviado `X-Iroha-Error-Code: schema_mismatch`,
   verifique se você está solicitando um endpoint que aceita cargas binárias e confirme
   que o hash do esquema foi registrado em `fixtures/norito_rpc/schema_hashes.json`
   corresponde ao cible au build Torii.O console mantém o arquivo mais recente na memória para que você possa reenviá-lo
meme payload contém todos os diferentes tokens de autorização ou hosts Torii. Adicionar
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` para seu fluxo de trabalho produz o pacote
de referências preliminares no plano de adoção NRPC-4 (log + currículo JSON), o que vai bem
com a captura da tela da resposta Try It nas avaliações.

### Exemplo de CLI (curl)

Les memes fixtures podem ser felizes fora do portal via `curl`, o que é útil
Ao validar o proxy ou desfazer as respostas do gateway:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v2/pipeline/submit"
```

Substitua o aparelho por qualquer item que esteja listado em `transaction_fixtures.manifest.json`
ou codifique sua própria carga útil com `cargo xtask norito-rpc-fixtures`. Quando Torii está em
modo canário você pode apontar o ponteiro `curl` para o proxy try-it
(`https://docs.sora.example/proxy/v2/pipeline/submit`) para exercer a infraestrutura do meme
que os widgets do portal são usados.

## Observabilidade e operações

Cada solicitação foi registrada uma vez com método, caminho, origem, status upstream e fonte
de autenticação (`override`, `default` ou `client`). Les tokens ne sont jamais stockes: les
headers bearer e os valores `X-TryIt-Auth` são alterados antes do log, para que você possa puissiez
retransmite stdout para um coletor central sem risco de vazamentos.

### Sondas de saúde e alertas

Lance a sonda incluindo pendentes de implantação ou em um cronograma:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v2/status" npm run probe:tryit-proxy
```

Botões de ambiente:

- `TRYIT_PROXY_SAMPLE_PATH` - rota Torii opcional (sem `/proxy`) a exercício.
- `TRYIT_PROXY_SAMPLE_METHOD` - por padrão `GET`; defina `POST` para as rotas de escrita.
- `TRYIT_PROXY_PROBE_TOKEN` - injeta um token de portador temporário para chamada de exemplo.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - apaga o tempo limite por padrão de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - texto de destino Prometheus opção para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por virgules aux metrices (par padrão `job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional do endpoint de métricas (por exemplo, `http://localhost:9798/metrics`) que deve responder com sucesso quando `TRYIT_PROXY_METRICS_LISTEN` estiver ativo.

Injeta os resultados em um coletor de arquivo de texto apontando para o teste em um caminho gravável
(por exemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) e um complemento de rótulos
personaliza:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

O script recrita o arquivo de métricas de facão atômico para que seu colecionador lise
sempre com carga útil completa.

Quando `TRYIT_PROXY_METRICS_LISTEN` está configurado, definido
`TRYIT_PROXY_PROBE_METRICS_URL` no ponto final de métricas para que a sonda ecoe rapidamente si
a superfície de raspagem é disparatada (por exemplo, entrada mal configurada ou regras de firewall inadequadas).
Um padrão de produção típico é
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Para alertas mais recentes, conecte a sonda à sua pilha de monitoramento. Exemplo Prometheus
aqui página apres deux echecs consecutifs:

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

Defina `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou todo host/porta) antes de
lance o proxy para expor um endpoint de métricas no formato Prometheus. O caminho
par padrão é `/metrics`, mas pode ser substituído por `TRYIT_PROXY_METRICS_PATH=/custom`. Chaque
raspar renvoie des compteurs des totaux par methode, des rejets rate limit, des erreurs/timeouts
upstream, o proxy de resultados e os currículos de latência:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Aponte seus coletores Prometheus/OTLP para o ponto final das métricas e reutilize os painéis
existentes em `dashboards/grafana/docs_portal.json` para que o SRE observe as latências da fila
e as fotos rejeitadas sem analisar os logs. Le proxy public automatiquement
`tryit_proxy_start_timestamp_ms` para auxiliar os operadores e detectar redecasamentos.

### Automatização da reversão

Use o ajudante de gerenciamento para mantê-lo atualizado ou restaure o URL Torii. O roteiro
armazene a configuração anterior em `.env.tryit-proxy.bak` para que as reversões ocorram
um único comando.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Sobrecarregue o caminho do arquivo ambiental com `--env` ou `TRYIT_PROXY_ENV` se sua implantação
guarde a configuração aqui.