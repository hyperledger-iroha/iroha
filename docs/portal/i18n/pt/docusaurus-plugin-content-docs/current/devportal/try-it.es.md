---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/try-it.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Sandbox de Experimente

O portal de desenvolvimento inclui um console opcional "Try it" para que você possa chamar endpoints de Torii sem sair da documentação. O console retransmite solicitações através do proxy incluído para que os navegadores possam evitar limites CORS enquanto se aplicam limites de tasa e autenticação.

## Pré-requisitos

- Node.js 18.18 ou mais novo (coincide com os requisitos de construção do portal)
- Acesso vermelho a um ambiente de teste Torii
- Um token de portador que pode chamar as rotas Torii que os aviões ejetam

Toda a configuração do proxy é realizada por meio de variáveis de ambiente. A tabela a seguir lista os botões mais importantes:

| Variável | Proposta | Padrão |
| --- | --- | --- |
| `TRYIT_PROXY_TARGET` | URL base de Torii para o proxy que reenvia solicitações | **Obrigatório** |
| `TRYIT_PROXY_LISTEN` | Direção de escudo para desenvolvimento local (formato `host:port` ou `[ipv6]:port`) | `127.0.0.1:8787` |
| `TRYIT_PROXY_ALLOWED_ORIGINS` | Lista separada por vírgulas de origem que pode ser chamada de proxy | `http://localhost:3000` |
| `TRYIT_PROXY_CLIENT_ID` | Identificador colocado em `X-TryIt-Client` para cada solicitação upstream | `docs-portal` |
| `TRYIT_PROXY_BEARER` | Bearer token por defeito reenviado para Torii | _vazio_ |
| `TRYIT_PROXY_ALLOW_CLIENT_AUTH` | Permitir que os usuários forneçam seu próprio token via `X-TryIt-Auth` | `0` |
| `TRYIT_PROXY_MAX_BODY` | Tamanho máximo do corpo de solicitação (bytes) | `1048576` |
| `TRYIT_PROXY_TIMEOUT_MS` | Timeout upstream em milissegundos | `10000` |
| `TRYIT_PROXY_RATE_LIMIT` | Solicitações permitidas por janela de tarefa por IP de cliente | `60` |
| `TRYIT_PROXY_RATE_WINDOW_MS` | Ventana deslizante para limitação de taxa (ms) | `60000` |
| `TRYIT_PROXY_METRICS_LISTEN` | Direção de escuta opcional para o endpoint de métricas estilo Prometheus (`host:port` ou `[ipv6]:port`) | _vazio (desativado)_ |
| `TRYIT_PROXY_METRICS_PATH` | Rota HTTP servida pelo endpoint de métricas | `/metrics` |

O proxy também expõe `GET /healthz`, gera erros JSON estruturados e oculta tokens de portador da saída de logs.

Ative `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1` para expor o proxy a usuários de documentos para que os painéis Swagger e RapiDoc possam reenviar tokens ao portador fornecidos pelo usuário. O proxy aplica-se a limites de tasa, oculta credenciais e registra-se mediante solicitação de uso do token por defeito ou anulação por solicitação. Configure `TRYIT_PROXY_CLIENT_ID` com a etiqueta que deseja enviar como `X-TryIt-Client`
(por defeito `docs-portal`). O proxy registra e valida os valores `X-TryIt-Client` transmitidos pelo cliente, retornando a este padrão para que os gateways de teste possam auditar a procedência sem metadados correlacionados do navegador.

## Inicia o proxy localmente

Instale as dependências da primeira vez que configura o portal:

```bash
cd docs/portal
npm install
```

Execute o proxy e conecte-o à sua instância Torii:

```bash
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
# Optional: preset a bearer token for the Swagger / RapiDoc panels
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
npm run tryit-proxy
```

O script registra a direção enlazada e reenvia solicitações de `/proxy/*` para a origem Torii configurada.

Antes de inserir o soquete, o script valida que
`static/openapi/torii.json` coincide com o resumo registrado em
`static/openapi/manifest.json`. Se os arquivos forem desincronizados, o comando termina com um
erro e indica executar `npm run sync-openapi -- --latest`. Exportar
`TRYIT_PROXY_ALLOW_STALE_SPEC=1` apenas para substituições de emergência; o registrador de proxy é um
advertência e continuação para que você possa se recuperar durante as janelas de manutenção.

## Conecte os widgets do portal

Ao construir ou servir o portal de desenvolvimento, defina a URL dos widgets
deben usar para o proxy:

```bash
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
export TRYIT_PROXY_DEFAULT_BEARER="Bearer eyJhbGciOi..." # Optional
npm run start
```

Os componentes seguintes leem esses valores de `docusaurus.config.js`:

- **Swagger UI** - renderizado em `/reference/torii-swagger`; pré-autoriza o esquema
  bearer quando há um token, etiqueta las solicitudes con `X-TryIt-Client`,
  inyecta `X-TryIt-Auth`, e reescrever as chamadas através do proxy quando
  `TRYIT_PROXY_PUBLIC_URL` está definido.
- **RapiDoc** - renderizado em `/reference/torii-rapidoc`; reflita o campo de token,
  reutilize os mesmos cabeçalhos do painel Swagger e coloque-o no proxy
  automaticamente quando o URL é configurado.
- **Try it console** - incorporado na página de visão geral da API; permite enviar
  solicitações personalizadas, ver cabeçalhos e inspecionar cuerpos de resposta.Ambos os painéis exibem um **seletor de instantâneos** que lee
`docs/portal/static/openapi/versions.json`. Preencha esse índice com
`npm run sync-openapi -- --version=<label> --mirror=current --latest` para os revisores
você pode pular entre as especificações históricas, ver o resumo SHA-256 registrado e confirmar se um
snapshot de release traz um manifesto firmado antes de usar os widgets interativos.

Alterar o token em qualquer widget só afeta a sessão atual do navegador; o proxy nunca
persista e registre o token fornecido.

## Tokens OAuth de corte de vida

Para evitar a distribuição de tokens Torii de longa duração para os revisores, conecte o console Try it a
seu servidor OAuth. Quando as variáveis do ambiente abaixo estão presentes, o portal renderiza uma
widget de login com código do dispositivo, gera tokens de portador de corte de vida e os inicia automaticamente
no formulário da consola.

| Variável | Proposta | Padrão |
| --- | --- | --- |
| `DOCS_OAUTH_DEVICE_CODE_URL` | Endpoint de autorização do dispositivo OAuth (`/oauth/device/code`) | _vazio (desativado)_ |
| `DOCS_OAUTH_TOKEN_URL` | Endpoint do token que aceita `grant_type=urn:ietf:params:oauth:grant-type:device_code` | _vazio_ |
| `DOCS_OAUTH_CLIENT_ID` | Identificador de cliente OAuth registrado para visualização de documentos | _vazio_ |
| `DOCS_OAUTH_SCOPE` | Escopos separados por espaços solicitados durante o login | `openid profile offline_access` |
| `DOCS_OAUTH_AUDIENCE` | Audiência de API opcional para vincular o token | _vazio_ |
| `DOCS_OAUTH_POLL_INTERVAL_MS` | Intervalo mínimo de votação enquanto espera aprovação (ms) | `5000` (valores < 5000 ms se rechazan) |
| `DOCS_OAUTH_DEVICE_CODE_TTL_SECONDS` | Ventana de expiração do código do dispositivo (segundos) | `600` (deve ser mantido entre 300 e 900 anos) |
| `DOCS_OAUTH_TOKEN_TTL_SECONDS` | Duração do token de acesso (segundos) | `900` (deve ser mantido entre 300 e 900 s) |
| `DOCS_OAUTH_ALLOW_INSECURE` | Pon `1` para visualizações de locais que omitem a aplicação do OAuth intencionalmente | _desativar_ |

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

Quando executados `npm run start` ou `npm run build`, o portal incrusta esses valores
em `docusaurus.config.js`. Durante uma visualização local da tarjeta Try it mostra um
botão "Entrar com código do dispositivo". Os usuários inserem o código exibido em sua página de verificação OAuth; uma vez que o fluxo do dispositivo saiu do widget:

- inyecta o token do portador emitido no campo da consola Experimente,
- etiquetar as solicitações com os cabeçalhos existentes `X-TryIt-Client` e `X-TryIt-Auth`,
- mostra o tempo restante da vida, y
- borra automaticamente o token quando expirar.

A entrada manual Bearer fica disponível; omitir as variáveis ​​OAuth quando você quiser
forçar os revisores a pegar um token temporal para eles mesmos, ou exportá-los
`DOCS_OAUTH_ALLOW_INSECURE=1` para visualizações locais isolados onde o acesso anônimo é
aceitável. As compilações sem OAuth configuradas agora falham rapidamente para satisfazer o portão
del roteiro DOCS-1b.

Nota: Revisa la [Lista de verificação de fortalecimento de segurança e pen-test](./security-hardening.md)
antes de expor o portal fora do laboratório; documenta o modelo de ameaça,
o perfil CSP/Trusted Types, e as etapas de pen-test que agora bloqueiam o DOCS-1b.

## Mostras Norito-RPC

As solicitações Norito-RPC compartilham o mesmo proxy e encanamento OAuth que as rotas JSON;
Simplesmente configure `Content-Type: application/x-norito` e envie a carga útil Norito
pré-codificado descrito na especificação NRPC
(`docs/source/torii/nrpc_spec.md`).
O repositório inclui cargas úteis canônicas abaixo de `fixtures/norito_rpc/` para os autores do portal,
proprietários de SDK e revisores podem reproduzir os bytes exatos que usam CI.

### Enviar uma carga útil Norito do console Try It

1. Escolha um acessório como `fixtures/norito_rpc/transfer_asset.norito`. Estos
   arquivos são envelopes Norito em bruto; **não** os códigos em base64.
2. No Swagger ou RapiDoc, localize o endpoint NRPC (por exemplo
   `POST /v1/pipeline/submit`) e altere o seletor **Content-Type** a
   `application/x-norito`.
3. Mude o editor do corpo de solicitação para **binary** (modo "Arquivo" de Swagger o
   selecione "Binary/File" do RapiDoc) e carregue o arquivo `.norito`. O widget
   transmite os bytes através do proxy sem alterá-los.
4. Envia la solicitud. Si Torii retorna `X-Iroha-Error-Code: schema_mismatch`,
   verifica que estes estão chamando um endpoint que aceita cargas binárias e confirma
   que o hash do esquema registrado em `fixtures/norito_rpc/schema_hashes.json`
   coincide com a compilação de Torii que você está usando.O consolo mantém o arquivo mais recente na memória para que você possa reenviar o mesmo
payload enquanto testa diferentes tokens de autorização ou hosts de Torii. Agregar
`scripts/run_norito_rpc_fixtures.sh --note "<ticket>"` seu fluxo de trabalho produz o pacote de
evidência referenciada no plano de adoção NRPC-4 (log + currículo JSON), qual combinação
é bom capturar capturas de tela da resposta Try It durante as revisões.

### Exemplo CLI (curl)

Os mesmos fixtures podem ser reproduzidos fora do portal via `curl`, o que é útil
quando o proxy é validado ou as respostas do gateway são:

```bash
TORII="https://torii.devnet.sora.example"
TOKEN="Bearer $(cat ~/.config/torii/devnet.token)"
curl   -H "Content-Type: application/x-norito"   -H "Authorization: ${TOKEN}"   --data-binary @fixtures/norito_rpc/transfer_asset.norito   "${TORII}/v1/pipeline/submit"
```

Altere o fixture por qualquer entrada listada em `transaction_fixtures.manifest.json`
ou codifique sua própria carga útil com `cargo xtask norito-rpc-fixtures`. Quando Torii está em
modo canary pode apuntar `curl` al proxy try-it
(`https://docs.sora.example/proxy/v1/pipeline/submit`) para ejetar o misma
infraestrutura que usa os widgets do portal.

## Observabilidade e operações

Cada solicitação é registrada uma vez com método, caminho, origem, estado upstream e fonte
de autenticação (`override`, `default` ou `client`). Os tokens nunca foram armazenados: tanto
os headers bearer como os valores `X-TryIt-Auth` são editados antes do registrador,
então você pode reenviar stdout para um coletor central sem se preocupar com filtrações.

### Sondas de saúde e alertas

Execute a sonda incluída durante despliegues ou em uma programação:

```bash
# Ensure the proxy responds to /healthz and forwards a sample request.
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_SAMPLE_PATH="/v1/status" npm run probe:tryit-proxy
```

Botões de entorno:

- `TRYIT_PROXY_SAMPLE_PATH` - ruta Torii opcional (sin `/proxy`) para ejercitar.
- `TRYIT_PROXY_SAMPLE_METHOD` - por defeito `GET`; defina `POST` para rotas de escritura.
- `TRYIT_PROXY_PROBE_TOKEN` - insere um token de portador temporal para a chamada de exibição.
- `TRYIT_PROXY_PROBE_TIMEOUT_MS` - sobrescreve o tempo limite por defeito de 5 s.
- `TRYIT_PROXY_PROBE_METRICS_FILE` - destino opcional de texto Prometheus para `probe_success`/`probe_duration_seconds`.
- `TRYIT_PROXY_PROBE_LABELS` - pares `key=value` separados por como anexados às métricas (por defeito `job=tryit-proxy` e `instance=<proxy URL>`).
- `TRYIT_PROXY_PROBE_METRICS_URL` - URL opcional do endpoint de métricas (por exemplo, `http://localhost:9798/metrics`) que deve responder com saída quando `TRYIT_PROXY_METRICS_LISTEN` estiver habilitado.

Alimenta os resultados em um coletor de arquivo de texto apontando a sonda para uma rota escribível
(por exemplo, `/var/lib/node_exporter/textfile_collector/tryit.prom`) e adicionando etiquetas
personalizado:

```bash
TRYIT_PROXY_PUBLIC_URL="https://docs.sora.example/proxy" TRYIT_PROXY_PROBE_METRICS_FILE="/var/lib/node_exporter/textfile_collector/tryit.prom" TRYIT_PROXY_PROBE_LABELS="job=tryit-proxy,cluster=prod" npm run probe:tryit-proxy
```

O script reescreve o arquivo de métricas de forma atômica para que seu coletor sempre leia
uma carga útil completa.

Quando `TRYIT_PROXY_METRICS_LISTEN` está configurado, defina
`TRYIT_PROXY_PROBE_METRICS_URL` no endpoint de métricas para que a sonda falhe rapidamente se ela
a superfície de scrape desaparece (por exemplo, ingresso mal configurado ou regras de firewall ausentes).
Um ajuste típico de produção é
`TRYIT_PROXY_PROBE_METRICS_URL="http://127.0.0.1:9798/metrics"`.

Para alertar Livianas, conecte a sonda à sua pilha de monitor. Exemplo de Prometheus que
página após os erros executados:

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

Configure `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` (ou qualquer par host/porto) antes de
inicie o proxy para expor um endpoint de métricas com formato Prometheus. A rota
por defeito é `/metrics`, mas pode mudar com `TRYIT_PROXY_METRICS_PATH=/custom`. Cada
scrape deve retornar contadores de total por método, rechaços por limite de taxa, erros/tempos limite
upstream, resultados do proxy e resumos de latência:

```bash
export TRYIT_PROXY_METRICS_LISTEN="127.0.0.1:9798"
npm run tryit-proxy &
curl http://127.0.0.1:9798/metrics | head -n 5
# HELP tryit_proxy_requests_total Requests handled by method
tryit_proxy_requests_total{method="GET"} 12
tryit_proxy_rate_limited_total 1
```

Coloque seus coletores Prometheus/OTLP no endpoint de métricas e reutilize os painéis existentes
en `dashboards/grafana/docs_portal.json` para que SRE observe latências de cola e picos de
rechazo sin analisar logs. O proxy publica automaticamente `tryit_proxy_start_timestamp_ms`
para ajudar os operadores a detectar reinícios.

### Automatização de reversão

Use o auxiliar de gerenciamento para atualizar ou restaurar o URL objetivo de Torii. O roteiro
guarde a configuração anterior em `.env.tryit-proxy.bak` para que as reversões sejam un
comando solo.

```bash
# Update TRYIT_PROXY_TARGET and back up the previous config.
npm run manage:tryit-proxy -- update --target https://torii.devnet.sora.example

# Roll back to the previously backed-up target.
npm run manage:tryit-proxy -- rollback
```

Descreva a rota do arquivo env com `--env` ou `TRYIT_PROXY_ENV` se você solicitar
guarde a configuração em outro lugar.