---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Abra Norito-RPC

Norito-RPC é o transporte binário para API Torii. Ele reutiliza os memes HTTP que `/v1/pipeline` e troca as cobranças inseridas por Norito que incluem hashes de esquema e somas de verificação. Use-o quando você precisar de respostas determinadas e válidas ou quando as respostas JSON do pipeline apresentarem uma falha de estrangulamento.

## Trocador de pourquoi?
- Um encadeamento determinado com CRC64 e hashes de esquema reduz erros de decodificação.
- Os auxiliares Norito compartilham entre SDKs que permitem reutilizar os tipos existentes do modelo de dados.
- Torii deixe as sessões Norito na telemetria, pois os operadores podem seguir a adoção com os painéis fornecidos.

## Faça uma receita

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Serialize sua carga útil com o codec Norito (`iroha_client`, helpers SDK ou `norito::to_bytes`).
2. Envie a solicitação com `Content-Type: application/x-norito`.
3. Exija uma resposta Norito via `Accept: application/x-norito`.
4. Decodifique a resposta com o SDK auxiliar correspondente.

Conselhos por SDK:
- **Rust**: `iroha_client::Client` negócio Norito automaticamente quando você define o `Accept`.
- **Python**: use `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: use `NoritoRpcClient` e `NoritoRpcRequestOptions` no SDK Android.
- **JavaScript/Swift**: os auxiliares são sucessivos em `docs/source/torii/norito_rpc_tracker.md` e chegam em NRPC-3.

## Exemplo de console Try It

O portal de desenvolvimento fornece um proxy Try It para que os receptores possam exibir as cargas úteis Norito sem criar scripts na medida.

1. [Desmarque o proxy](./try-it.md#start-the-proxy-locally) e defina `TRYIT_PROXY_PUBLIC_URL` para que os widgets sejam enviados ou enviados para o tráfego.
2. Abra o cartão **Try it** nesta página ou no painel `/reference/torii-swagger` e selecione um endpoint como `POST /v1/pipeline/submit`.
3. Passe o **Content-Type** para `application/x-norito`, escolha o editor **Binary** e carregue `fixtures/norito_rpc/transfer_asset.norito` (ou toda a lista de carga útil em `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Forneça um token de portador por meio do widget OAuth device-code ou do código manual (o proxy aceita as substituições `X-TryIt-Auth` quando é configurado com `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Envie a solicitação e verifique se Torii envia a lista `schema_hash` em `fixtures/norito_rpc/schema_hashes.json`. Os hashes idênticos confirmam que o Norito está localizado no navegador/proxy.

Para o roteiro de evidências, associe a captura da tela Try It a uma execução de `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. O script encapsula `cargo xtask norito-rpc-verify`, escreve o currículo JSON em `artifacts/norito_rpc/<timestamp>/` e captura os dispositivos de memes que você compartilha.

## Depanagem

| Sintoma | Ou este aparelho | Causa provável | Corretivo |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Resposta Torii | En-tete `Content-Type` manquant ou incorreto | Defina `Content-Type: application/x-norito` antes de enviar a carga útil. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Corpo/têtes de resposta Torii | O hash do esquema de luminárias difere da construção Torii | Regenere os fixtures com `cargo xtask norito-rpc-fixtures` e confirme o hash em `fixtures/norito_rpc/schema_hashes.json`; repasse em JSON se o endpoint não estiver ativo Norito. |
| `{"error":"origin_forbidden"}` (HTTP403) | Resposta do proxy Experimente | A solicitação vem de uma origem não listada em `TRYIT_PROXY_ALLOWED_ORIGINS` | Adicione a origem do portal (por ex. `https://docs.devnet.sora.example`) à variável de ambiente e redefina o proxy. |
| `{"error":"rate_limited"}` (HTTP429) | Resposta do proxy Experimente | A cota por IP para ultrapassar o orçamento `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Aumente o limite para testes de carga internos ou participe da reinicialização da janela (veja `retryAfterMs` na resposta JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Resposta do proxy Experimente | Torii expira ou o proxy não permite a configuração do backend | Verifique se `TRYIT_PROXY_TARGET` está acessível, controle o estado de Torii ou reessai com um `TRYIT_PROXY_TIMEOUT_MS` mais eleve. |

Além disso, o diagnóstico Try It e os conselhos OAuth foram encontrados em [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recursos complementares
- Transporte RFC: `docs/source/torii/norito_rpc.md`
- Currículo executivo: `docs/source/torii/norito_rpc_brief.md`
- Rastreador de ação: `docs/source/torii/norito_rpc_tracker.md`
- Instruções do proxy Try-It: `docs/portal/docs/devportal/try-it.md`