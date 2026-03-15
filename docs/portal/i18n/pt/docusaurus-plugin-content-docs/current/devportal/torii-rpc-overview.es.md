---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Resumo de Norito-RPC

Norito-RPC é o transporte binário para as APIs de Torii. Reutiliza as mesmas rotas HTTP que `/v2/pipeline`, mas alterna cargas marcadas por Norito que incluem hashes de esquema e somas de verificação. Usado quando você precisa de respostas deterministas e validadas ou quando as respostas JSON do pipeline são retornadas para um copo de garrafa.

## Por que mudar?
- O determinista marcado com CRC64 e hashes de esquema reduzem erros de decodificação.
- Os auxiliares Norito compartilhados entre SDKs permitem reutilizar tipos existentes no modelo de dados.
- Torii e marque as sessões Norito em telemetria, para que os operadores possam monitorar a adoção com os painéis fornecidos.

## Como fazer uma solicitação

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v2/transactions/submit
```

1. Serialize sua carga útil com o codec Norito (`iroha_client`, ajudantes do SDK ou `norito::to_bytes`).
2. Envie a solicitação com `Content-Type: application/x-norito`.
3. Solicite uma resposta Norito usando `Accept: application/x-norito`.
4. Decodificar a resposta com o auxiliar do SDK correspondente.

Guia do SDK:
- **Rust**: `iroha_client::Client` negocia Norito automaticamente quando estabelece o cabeçalho `Accept`.
- **Python**: usa `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: usa `NoritoRpcClient` e `NoritoRpcRequestOptions` no SDK do Android.
- **JavaScript/Swift**: os auxiliares são rastreados em `docs/source/torii/norito_rpc_tracker.md` e transferidos como parte do NRPC-3.

## Exemplo de console Try It

O portal de desenvolvimento inclui um proxy Try It para que os revisores possam reproduzir cargas úteis Norito sem escrever scripts conforme necessário.

1. [Inicie o proxy](./try-it.md#start-the-proxy-locally) e defina `TRYIT_PROXY_PUBLIC_URL` para que os widgets se espalhem para onde enviar o tráfego.
2. Abra a tarjeta **Try it** nesta página ou no painel `/reference/torii-swagger` e selecione um endpoint como `POST /v2/pipeline/submit`.
3. Mude o **Content-Type** para `application/x-norito`, escolha o editor **Binary** e sube `fixtures/norito_rpc/transfer_asset.norito` (ou qualquer carga útil listada em `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Proporcione um token de portador por meio do widget OAuth device-code ou do campo de token manual (o proxy aceita substitui `X-TryIt-Auth` quando configurado com `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envie a solicitação e verifique se Torii reflete o `schema_hash` listado em `fixtures/norito_rpc/schema_hashes.json`. Os hashes coincidentes confirmam que o encabezado Norito sobrevive ao salto navegador/proxy.

Para evidenciar o roteiro, combine a captura de tela de Try It com uma execução de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. O script envolve `cargo xtask norito-rpc-verify`, escreve o currículo JSON em `artifacts/norito_rpc/<timestamp>/` e captura os mesmos fixtures que consomem o portal.

## Resolução de problemas

| Sintoma | Donde aparece | Causa provável | Solução |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Resposta de Torii | Falta ou está incorreto o cabeçalho `Content-Type` | Defina `Content-Type: application/x-norito` antes de enviar a carga útil. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Cuerpo/headers de resposta de Torii | O hash do esquema de luminárias difere da construção de Torii | Regenera fixtures com `cargo xtask norito-rpc-fixtures` e confirma o hash em `fixtures/norito_rpc/schema_hashes.json`; usa JSON substituto se o endpoint não estiver habilitado para Norito. |
| `{"error":"origin_forbidden"}` (HTTP403) | Resposta do proxy Try It | A solicitação de uma origem que não esteja listada em `TRYIT_PROXY_ALLOWED_ORIGINS` | Agregue a origem do portal (por exemplo, `https://docs.devnet.sora.example`) à variável de ambiente e reinicie o proxy. |
| `{"error":"rate_limited"}` (HTTP429) | Resposta do proxy Try It | A cota por IP excede o pressuposto de `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Aumente o limite para testes internos de carga ou espere até que a janela seja reiniciada (consulte `retryAfterMs` na resposta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Resposta do proxy Try It | Torii aguardo o tempo ou o proxy não posso alcançar o backend configurado | Verifique se `TRYIT_PROXY_TARGET` é acessível ao mar, revise a saúde de Torii ou tente novamente com um `TRYIT_PROXY_TIMEOUT_MS` maior. |

Mas diagnósticos de Try It e conselhos de OAuth vivem em [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recursos adicionais
- RFC de transporte: `docs/source/torii/norito_rpc.md`
- Currículo executivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de ações: `docs/source/torii/norito_rpc_tracker.md`
- Instruções do proxy Try-It: `docs/portal/docs/devportal/try-it.md`