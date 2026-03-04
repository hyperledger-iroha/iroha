---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Visão geral do Norito-RPC

Norito-RPC e o transporte binário para as APIs do Torii. Ele reutiliza os mesmos caminhos HTTP de `/v1/pipeline`, mas troca payloads emoldurados por Norito que incluem hashes de esquema e checksums. Use quando precisar de respostas determinísticas e validadas ou quando as respostas JSON do pipeline virarem um gargalo.

## Por que mudar?
- Enquadramento determinístico com CRC64 e hashes de esquema reduz erros de decodificação.
- Helpers Norito compartilhados entre SDKs permitem reutilizar tipos existentes do modelo de dados.
- Torii ja marca sessões Norito na telemetria, então os operadores podem acompanhar a adocao com os dashboards fornecidos.

## Fazendo uma requisição

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Serialize seu payload com o codec Norito (`iroha_client`, helpers do SDK ou `norito::to_bytes`).
2. Envie uma requisição com `Content-Type: application/x-norito`.
3. Solicite uma resposta Norito usando `Accept: application/x-norito`.
4. Decodifique a resposta com o helper do SDK correspondente.

Guia do SDK:
- **Rust**: `iroha_client::Client` negocia Norito automaticamente quando você define o cabeçalho `Accept`.
- **Python**: use `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: use `NoritoRpcClient` e `NoritoRpcRequestOptions` no SDK Android.
- **JavaScript/Swift**: os helpers estão rastreados em `docs/source/torii/norito_rpc_tracker.md` e chegamao como parte do NRPC-3.

## Exemplo de console Try It

O portal do desenvolvedor inclui um proxy Try It para que os revisores possam reproduzir payloads Norito sem escrever scripts sob medida.

1. [Inicie o proxy](./try-it.md#start-the-proxy-locally) e defina `TRYIT_PROXY_PUBLIC_URL` para que os widgets saibam para onde enviar o tráfego.
2. Abra o cartão **Try it** nesta página ou no painel `/reference/torii-swagger` e selecione um endpoint como `POST /v1/pipeline/submit`.
3. Mude o **Content-Type** para `application/x-norito`, escolha o editor **Binary** e envie `fixtures/norito_rpc/transfer_asset.norito` (ou qualquer payload listado em `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Forneça um bearer token via o widget OAuth device-code ou o campo manual (o proxy aceita overrides `X-TryIt-Auth` quando configurado com `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envie uma requisição e verifique se o Torii ecoa ou `schema_hash` listado em `fixtures/norito_rpc/schema_hashes.json`. Hashes iguais confirmam que o cabecalho Norito sobreviveu ao salto navegador/proxy.

Para evidenciar o roadmap, combine a captura de tela do Try It com uma execução de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. O script envolve `cargo xtask norito-rpc-verify`, escreve o resumo JSON em `artifacts/norito_rpc/<timestamp>/` e captura os mesmos fixtures consumidos pelo portal.

## Solução de problemas

| Sintoma | Onde aparece | Causa provavel | Correção |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Resposta do Torii | Cabeçalho `Content-Type` ausente ou incorreto | Defina `Content-Type: application/x-norito` antes de enviar o payload. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Corpo/headers de resposta do Torii | Hash de esquema dos fixtures diferentes do build do Torii | Regenere fixtures com `cargo xtask norito-rpc-fixtures` e confirme o hash em `fixtures/norito_rpc/schema_hashes.json`; use fallback JSON se o endpoint ainda não habilitou Norito. |
| `{"error":"origin_forbidden"}` (HTTP403) | Resposta do proxy Experimente | A requisição veio de uma origem não listada em `TRYIT_PROXY_ALLOWED_ORIGINS` | Adicione a origem do portal (por exemplo, `https://docs.devnet.sora.example`) na variável de ambiente e reinicie o proxy. |
| `{"error":"rate_limited"}` (HTTP429) | Resposta do proxy Experimente | A cota por IP excedeu o orçamento `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Aumente o limite para testes internos de carga ou espere uma janela reiniciar (veja `retryAfterMs` na resposta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Resposta do proxy Experimente | Torii expirou ou o proxy não conseguiu alcancar o backend configurado | Verifique se `TRYIT_PROXY_TARGET` está acessível, verifique a saúde do Torii ou tente novamente com um `TRYIT_PROXY_TIMEOUT_MS` maior. |

Mais diagnósticos do Try It e dicas de OAuth ficam em [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recursos adicionais
- RFC de transporte: `docs/source/torii/norito_rpc.md`
- Resumo executivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de ações: `docs/source/torii/norito_rpc_tracker.md`
- Instruções do proxy Try-It: `docs/portal/docs/devportal/try-it.md`