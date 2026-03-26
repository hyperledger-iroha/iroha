---
lang: pt
direction: ltr
source: docs/portal/docs/norito/try-it-console.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: Console Try-It de Norito
description: Use o proxy do portal de desenvolvedores e os widgets Swagger e RapiDoc para enviar requisicoes reais de Torii / Norito-RPC diretamente do site de documentacao.
---

O portal reune tres superficies interativas que repassam trafego para Torii:

- **Swagger UI** em `/reference/torii-swagger` renderiza a especificacao OpenAPI assinada e reescreve automaticamente as requisicoes pelo proxy quando `TRYIT_PROXY_PUBLIC_URL` esta configurado.
- **RapiDoc** em `/reference/torii-rapidoc` exibe o mesmo esquema com uploads de arquivos e seletores de tipo de conteudo que funcionam bem para `application/x-norito`.
- **Try it sandbox** na pagina de visao geral do Norito fornece um formulario leve para requisicoes REST ad hoc e logins OAuth por dispositivo.

Os tres widgets enviam requisicoes ao **proxy Try-It** local (`docs/portal/scripts/tryit-proxy.mjs`). O proxy verifica se `static/openapi/torii.json` coincide com o digest assinado em `static/openapi/manifest.json`, aplica um limitador de taxa, redige os cabecalhos `X-TryIt-Auth` nos logs e marca cada chamada upstream com `X-TryIt-Client` para que operadores de Torii possam auditar as fontes de trafego.

## Inicie o proxy

```bash
cd docs/portal
npm install
export TRYIT_PROXY_TARGET="https://torii.devnet.sora.example"
export TRYIT_PROXY_ALLOWED_ORIGINS="http://localhost:3000"
export TRYIT_PROXY_PUBLIC_URL="http://localhost:8787"
# Optional, use short-lived tokens only:
export TRYIT_PROXY_BEARER="Bearer eyJhbGciOi..."
export TRYIT_PROXY_CLIENT_ID="docs-portal"
export DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1
npm run tryit-proxy
```

- `TRYIT_PROXY_TARGET` e a URL base de Torii que voce quer exercitar.
- `TRYIT_PROXY_ALLOWED_ORIGINS` deve incluir toda origem do portal (servidor local, hostname de producao, URL de preview) que deve embutir a console.
- `TRYIT_PROXY_PUBLIC_URL` e consumido em `docusaurus.config.js` e injetado nos widgets via `customFields.tryIt`.
- `TRYIT_PROXY_BEARER` so carrega quando `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`; caso contrario os usuarios devem fornecer o proprio token via console ou fluxo de dispositivo OAuth.
- `TRYIT_PROXY_CLIENT_ID` define a tag `X-TryIt-Client` carregada em cada requisicao.
  Fornecer `X-TryIt-Client` pelo navegador e permitido, mas os valores sao truncados
  e rejeitados se contiverem caracteres de controle.

Na inicializacao o proxy executa `verifySpecDigest` e encerra com uma dica de remediacao se o manifesto estiver desatualizado. Execute `npm run sync-openapi -- --latest` para baixar a especificacao Torii mais nova ou passe `TRYIT_PROXY_ALLOW_STALE_SPEC=1` para overrides de emergencia.

Para atualizar ou reverter o destino do proxy sem editar arquivos de ambiente a mao, use o helper:

```bash
npm run manage:tryit-proxy -- update --target https://new.torii.example
npm run manage:tryit-proxy -- rollback
```

## Conecte os widgets

Sirva o portal depois que o proxy estiver ouvindo:

```bash
cd docs/portal
TRYIT_PROXY_PUBLIC_URL="http://localhost:8787" npm run start
```

`docusaurus.config.js` define os seguintes ajustes:

| Variavel | Finalidade |
| --- | --- |
| `TRYIT_PROXY_PUBLIC_URL` | URL injetada no Swagger, RapiDoc e no sandbox Try it. Deixe sem definir para ocultar os widgets durante previews nao autorizados. |
| `TRYIT_PROXY_DEFAULT_BEARER` | Token padrao opcional mantido em memoria. Requer `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1` e a protecao CSP somente HTTPS (DOCS-1b) a menos que voce passe `DOCS_SECURITY_ALLOW_INSECURE=1` localmente. |
| `DOCS_OAUTH_*` | Habilita o fluxo de dispositivo OAuth (`OAuthDeviceLogin` component) para que revisores possam emitir tokens de curta duracao sem sair do portal. |

Quando as variaveis OAuth estao presentes, o sandbox renderiza um botao **Sign in with device code** que percorre o servidor de Auth configurado (veja `config/security-helpers.js` para o formato exato). Tokens emitidos pelo fluxo de dispositivo so ficam em cache na sessao do navegador.

## Enviar payloads Norito-RPC

1. Crie um payload `.norito` com o CLI ou com os trechos descritos no [quickstart do Norito](./quickstart.md). O proxy encaminha corpos `application/x-norito` sem alterar, entao voce pode reutilizar o mesmo artefato que enviaria com `curl`.
2. Abra `/reference/torii-rapidoc` (preferido para payloads binarios) ou `/reference/torii-swagger`.
3. Selecione o snapshot de Torii desejado no menu suspenso. Snapshots sao assinados; o painel mostra o digest do manifesto registrado em `static/openapi/manifest.json`.
4. Escolha o tipo de conteudo `application/x-norito` na gaveta "Try it", clique em **Choose File** e selecione seu payload. O proxy reescreve a requisicao para `/proxy/v1/pipeline/submit` e marca com `X-TryIt-Client=docs-portal-rapidoc`.
5. Para baixar respostas Norito, defina `Accept: application/x-norito`. Swagger/RapiDoc exibem o seletor de cabecalho na mesma gaveta e retransmitem o binario de volta pelo proxy.

Para rotas somente JSON, o sandbox Try it embutido costuma ser mais rapido: informe o caminho (por exemplo, `/v1/accounts/<i105-account-id>/assets`), selecione o metodo HTTP, cole um corpo JSON quando necessario e clique em **Send request** para inspecionar cabecalhos, duracao e payloads no proprio painel.

## Solucao de problemas

| Sintoma | Causa provavel | Remediacao |
| --- | --- | --- |
| O console do navegador mostra erros CORS ou o sandbox avisa que o URL do proxy esta ausente. | O proxy nao esta em execucao ou a origem nao esta na lista permitida. | Inicie o proxy, confirme que `TRYIT_PROXY_ALLOWED_ORIGINS` cobre o host do portal e reinicie `npm run start`. |
| `npm run tryit-proxy` encerra com "digest mismatch". | O bundle OpenAPI de Torii mudou no upstream. | Execute `npm run sync-openapi -- --latest` (ou `--version=<tag>`) e tente novamente. |
| Os widgets retornam `401` ou `403`. | Token ausente, expirado ou com escopos insuficientes. | Use o fluxo de dispositivo OAuth ou cole um bearer token valido no sandbox. Para tokens estaticos voce deve exportar `DOCS_TRYIT_ALLOW_DEFAULT_BEARER=1`. |
| `429 Too Many Requests` vindo do proxy. | Limite de taxa por IP excedido. | Aumente `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` para ambientes confiaveis ou reduza scripts de teste. Todas as recusas por rate limit incrementam `tryit_proxy_rate_limited_total`. |

## Observabilidade

- `npm run probe:tryit-proxy` (wrapper em torno de `scripts/tryit-proxy-probe.mjs`) chama `/healthz`, opcionalmente exercita uma rota de exemplo e emite arquivos de texto Prometheus para `probe_success` / `probe_duration_seconds`. Configure `TRYIT_PROXY_PROBE_METRICS_FILE` para integrar com node_exporter.
- Configure `TRYIT_PROXY_METRICS_LISTEN=127.0.0.1:9798` para expor contadores (`tryit_proxy_requests_total`, `tryit_proxy_rate_limited_total`, `tryit_proxy_upstream_failures_total`) e histogramas de latencia. O painel `dashboards/grafana/docs_portal.json` le essas metricas para impor SLOs DOCS-SORA.
- Logs de runtime ficam em stdout. Cada entrada inclui o id da requisicao, o status upstream, a fonte de autenticacao (`default`, `override` ou `client`) e a duracao; segredos sao redigidos antes da emissao.

Se voce precisar validar que payloads `application/x-norito` chegam ao Torii sem alteracao, execute a suite Jest (`npm test -- tryit-proxy`) ou inspecione os fixtures em `docs/portal/scripts/__tests__/tryit-proxy.test.mjs`. Os testes de regressao cobrem binarios Norito comprimidos, manifestos OpenAPI assinados e caminhos de downgrade do proxy para que rollouts NRPC mantenham um rastro de evidencia permanente.
