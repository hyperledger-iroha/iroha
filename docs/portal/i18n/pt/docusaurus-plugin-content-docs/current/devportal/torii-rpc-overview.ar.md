---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ar.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# نظرة عامة على Norito-RPC

Norito-RPC é o nome do Torii. Use o protocolo HTTP para `/v1/pipeline` para obter informações sobre o Norito Use hashes para dados e somas de verificação. استخدمه عندما تحتاج الى ردود حتمية ومتحقق منها او عندما تصبح استجابات JSON الخاصة بالـ pipeline é um problema.

## لماذا ننتقل؟
- Use o CRC64 e os hashes para obter o valor desejado.
- Use Norito para usar SDKs sem problemas.
- Torii é um dispositivo Norito que pode ser usado sem problemas Certifique-se de que o dispositivo esteja funcionando corretamente.

## تنفيذ طلب

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. Use o codec Norito (`iroha_client`, SDK e `norito::to_bytes`).
2. Limpe o dispositivo com `Content-Type: application/x-norito`.
3. Coloque o Norito no lugar do `Accept: application/x-norito`.
4. Verifique se o SDK está instalado.

Baixe o SDK do SDK:
- **Ferrugem**: `iroha_client::Client` é usado para Norito para remover a ferrugem do `Accept`.
- **Python**: Altere `NoritoRpcClient` para `iroha_python.norito_rpc`.
- **Android**: Use `NoritoRpcClient` e `NoritoRpcRequestOptions` no Android SDK.
- **JavaScript/Swift**: O código é enviado em `docs/source/torii/norito_rpc_tracker.md` e no NRPC-3.

## مثال وحدة Experimente

Você pode tentar e experimentá-lo no site de teste Norito. Não.

1. [./try-it.md#start-the-proxy-locally] e `TRYIT_PROXY_PUBLIC_URL` são usados ​​para remover o problema.
2. Verifique **Experimente** no site e no `/reference/torii-swagger` e no `POST /v1/pipeline/submit`.
3. Selecione **Content-Type** como `application/x-norito`, usando **Binary**, e `fixtures/norito_rpc/transfer_asset.norito` (ou seja, o valor de **Binary** é `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Crie o token de suporte para o código do dispositivo OAuth e o código do dispositivo OAuth e o código do dispositivo `X-TryIt-Auth`. `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. A configuração do Torii é feita pelo `schema_hash` do `fixtures/norito_rpc/schema_hashes.json`. Verifique se o Norito está localizado no local certo.

لادلة خارطة الطريق, ارفق لقطة شاشة Experimente com `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Você pode usar o `cargo xtask norito-rpc-verify`, e o JSON do `artifacts/norito_rpc/<timestamp>/`, e também os fixtures البوابة.

## استكشاف الاخطاء

| العرض | مكان الظهور | السبب المحتمل | الحل |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Conjunto Torii | Ferramentas `Content-Type` Máquinas e ferramentas | Use `Content-Type: application/x-norito` para remover o problema. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Conjunto/revestimento Torii | hash مخطط fixtures يختلف عن build Torii | Use fixtures em `cargo xtask norito-rpc-fixtures` e hash em `fixtures/norito_rpc/schema_hashes.json`; Use o JSON substituto para o Norito. |
| `{"error":"origin_forbidden"}` (HTTP403) | استجابة وكيل Experimente | Você pode fazer isso em `TRYIT_PROXY_ALLOWED_ORIGINS` | Verifique se o dispositivo (referência `https://docs.devnet.sora.example`) pode ser removido e danificado. |
| `{"error":"rate_limited"}` (HTTP429) | استجابة وكيل Experimente | Definições de IP para dispositivos `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Você pode usar o recurso `retryAfterMs` para JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) e `{"error":"upstream_error"}` (HTTP 502) | استجابة وكيل Experimente | Torii انتهت مهلة الاستجابة او لم يستطع الوكيل الوصول الى الخلفية | Você pode usar o `TRYIT_PROXY_TARGET`, e o Torii, e usar o `TRYIT_PROXY_TIMEOUT_MS`. |

O aplicativo Try It e o OAuth estão disponíveis em [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## موارد اضافية
- Código RFC: `docs/source/torii/norito_rpc.md`
- Nome do usuário: `docs/source/torii/norito_rpc_brief.md`
- Código de identificação: `docs/source/torii/norito_rpc_tracker.md`
- Teste e Try-It: `docs/portal/docs/devportal/try-it.md`