---
lang: he
direction: rtl
source: docs/portal/i18n/pt/docusaurus-plugin-content-docs/current/devportal/torii-rpc-overview.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e8e11288c493b3f37f776ba15867abb7104bfa87ffd379a94ad975570405bb68
source_last_modified: "2025-11-14T04:43:20.179533+00:00"
translation_last_reviewed: 2026-01-30
---

# Visao geral do Norito-RPC

Norito-RPC e o transporte binario para as APIs do Torii. Ele reutiliza os mesmos caminhos HTTP de `/v1/pipeline` mas troca payloads emoldurados por Norito que incluem hashes de schema e checksums. Use quando precisar de respostas deterministicas e validadas ou quando as respostas JSON do pipeline virarem um gargalo.

## Por que mudar?
- Enquadramento deterministico com CRC64 e hashes de schema reduz erros de decodificacao.
- Helpers Norito compartilhados entre SDKs permitem reutilizar tipos existentes do modelo de dados.
- Torii ja marca sessoes Norito na telemetria, entao operadores podem acompanhar a adocao com os dashboards fornecidos.

## Fazendo uma requisicao

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Serialize seu payload com o codec Norito (`iroha_client`, helpers do SDK ou `norito::to_bytes`).
2. Envie a requisicao com `Content-Type: application/x-norito`.
3. Solicite uma resposta Norito usando `Accept: application/x-norito`.
4. Decodifique a resposta com o helper do SDK correspondente.

Guia por SDK:
- **Rust**: `iroha_client::Client` negocia Norito automaticamente quando voce define o header `Accept`.
- **Python**: use `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: use `NoritoRpcClient` e `NoritoRpcRequestOptions` no SDK Android.
- **JavaScript/Swift**: os helpers estao rastreados em `docs/source/torii/norito_rpc_tracker.md` e chegarao como parte do NRPC-3.

## Exemplo de console Try It

O portal do desenvolvedor inclui um proxy Try It para que revisores possam reproduzir payloads Norito sem escrever scripts sob medida.

1. [Inicie o proxy](./try-it.md#start-the-proxy-locally) e defina `TRYIT_PROXY_PUBLIC_URL` para que os widgets saibam para onde enviar o trafego.
2. Abra o card **Try it** nesta pagina ou o painel `/reference/torii-swagger` e selecione um endpoint como `POST /v1/pipeline/submit`.
3. Mude o **Content-Type** para `application/x-norito`, escolha o editor **Binary** e envie `fixtures/norito_rpc/transfer_asset.norito` (ou qualquer payload listado em `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Forneca um bearer token via o widget OAuth device-code ou o campo manual (o proxy aceita overrides `X-TryIt-Auth` quando configurado com `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envie a requisicao e verifique se o Torii ecoa o `schema_hash` listado em `fixtures/norito_rpc/schema_hashes.json`. Hashes iguais confirmam que o cabecalho Norito sobreviveu ao salto navegador/proxy.

Para evidencia do roadmap, combine a captura de tela do Try It com uma execucao de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. O script envolve `cargo xtask norito-rpc-verify`, escreve o resumo JSON em `artifacts/norito_rpc/<timestamp>/` e captura os mesmos fixtures consumidos pelo portal.

## Solucao de problemas

| Sintoma | Onde aparece | Causa provavel | Correcao |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Resposta do Torii | Header `Content-Type` ausente ou incorreto | Defina `Content-Type: application/x-norito` antes de enviar o payload. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP 400) | Corpo/headers de resposta do Torii | Hash de schema dos fixtures difere do build do Torii | Regenere fixtures com `cargo xtask norito-rpc-fixtures` e confirme o hash em `fixtures/norito_rpc/schema_hashes.json`; use fallback JSON se o endpoint ainda nao habilitou Norito. |
| `{"error":"origin_forbidden"}` (HTTP 403) | Resposta do proxy Try It | A requisicao veio de uma origem nao listada em `TRYIT_PROXY_ALLOWED_ORIGINS` | Adicione a origem do portal (por exemplo, `https://docs.devnet.sora.example`) na variavel de ambiente e reinicie o proxy. |
| `{"error":"rate_limited"}` (HTTP 429) | Resposta do proxy Try It | A cota por IP excedeu o budget `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Aumente o limite para testes internos de carga ou espere a janela reiniciar (veja `retryAfterMs` na resposta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Resposta do proxy Try It | Torii expirou ou o proxy nao conseguiu alcancar o backend configurado | Verifique se `TRYIT_PROXY_TARGET` esta acessivel, confira a saude do Torii ou tente novamente com um `TRYIT_PROXY_TIMEOUT_MS` maior. |

Mais diagnosticos do Try It e dicas de OAuth ficam em [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recursos adicionais
- RFC de transporte: `docs/source/torii/norito_rpc.md`
- Resumo executivo: `docs/source/torii/norito_rpc_brief.md`
- Tracker de acoes: `docs/source/torii/norito_rpc_tracker.md`
- Instrucoes do proxy Try-It: `docs/portal/docs/devportal/try-it.md`
