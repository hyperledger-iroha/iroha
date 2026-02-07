---
lang: pt
direction: ltr
source: docs/portal/docs/devportal/torii-rpc-overview.ru.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

# Обзор Norito-RPC

Norito-RPC - Transporte binário para API Torii. Ao usar o arquivo HTTP, este e `/v1/pipeline`, não use Norito-фрейминингом com esquemas e soma de verificação. Use-o para determinar e testar a configuração do JSON ou a configuração do pipeline JSON местом.

## O que você está procurando?
- Determinar a configuração do CRC64 e verificar os padrões de decodificação.
- Use o auxiliar Norito - o SDK do seu dispositivo permite que você verifique os tipos de modelo desejados.
- Torii permite que o Norito tenha sessões de telefonia, para que o operador possa desativar a propriedade через доступные дашборды.

## Desbloquear a senha

```bash
curl       -H 'Content-Type: application/x-norito'       -H 'Accept: application/x-norito'       -H "Authorization: Bearer ${TOKEN}"       --data-binary @signed_transaction.norito       https://torii.devnet.sora.example/v1/transactions/submit
```

1. A carga útil é definida pelo codec Norito (`iroha_client`, SDK auxiliar ou `norito::to_bytes`).
2. Abra a chave com `Content-Type: application/x-norito`.
3. Verifique o Norito com `Accept: application/x-norito`.
4. Декодируйте ответ соответствующим SDK helper-ом.

Recomendações do SDK:
- **Ferrugem**: `iroha_client::Client` автоматически договаривается о Norito, когда задан заголовок `Accept`.
- **Python**: use `NoritoRpcClient` de `iroha_python.norito_rpc`.
- **Android**: use `NoritoRpcClient` e `NoritoRpcRequestOptions` no Android SDK.
- **JavaScript/Swift**: helper-ы отслеживаются em `docs/source/torii/norito_rpc_tracker.md` e появятся em рамках NRPC-3.

## Пример консоли Experimente

Портал разработчика поставляет Try It прокси, чтобы рецензенты могли воспроизводить Norito payload-ы написания отдельных скриптов.

1. [Adicione a senha](./try-it.md#start-the-proxy-locally) e feche `TRYIT_PROXY_PUBLIC_URL`, você verá o sinal automaticamente, para que o tráfego seja aberto.
2. Abra o cartão **Experimente** nesta página ou painel `/reference/torii-swagger` e verifique o endpoint, por exemplo `POST /v1/pipeline/submit`.
3. Selecione **Content-Type** em `application/x-norito`, selecione o editor **Binary** e execute `fixtures/norito_rpc/transfer_asset.norito` (ou mais carga útil de `fixtures/norito_rpc/transaction_fixtures.manifest.json`).
4. Selecione o token de portador para o widget de código do dispositivo OAuth ou não (o procedimento substitui `X-TryIt-Auth`, como o `TRYIT_PROXY_ALLOW_CLIENT_AUTH=1`).
5. Abra e instale o Torii com o `schema_hash`, conectado ao `fixtures/norito_rpc/schema_hashes.json`. Para obter mais informações, verifique se o Norito é instalado no navegador/produto.

Para obter o roteiro, verifique a tela Experimente com `scripts/run_norito_rpc_fixtures.sh --note "<ticket>"`. Escreva o script `cargo xtask norito-rpc-verify`, insira JSON em `artifacts/norito_rpc/<timestamp>/` e configure seus fixtures, como использовал портал.

## Устранение неполадок

| Sintoma | Где проявляется | Вероятная причина | Expansão |
| --- | --- | --- | --- |
| `415 Unsupported Media Type` | Porta Torii | Отсутствует или неверный `Content-Type` | Use `Content-Type: application/x-norito` para liberar a carga útil. |
| `X-Iroha-Error-Code: schema_mismatch` (HTTP400) | Tela/capa de proteção Torii | Os dispositivos elétricos não são compatíveis com o сборкой Torii | Coloque os fixtures em `cargo xtask norito-rpc-fixtures` e instale-os em `fixtures/norito_rpc/schema_hashes.json`; usa substituto JSON, exceto o endpoint que não possui Norito. |
| `{"error":"origin_forbidden"}` (HTTP403) | Veja o site Try It | O código de origem é encontrado na rede `TRYIT_PROXY_ALLOWED_ORIGINS` | Insira o portal de origem (por exemplo, `https://docs.devnet.sora.example`) na abertura e na solicitação de configuração. |
| `{"error":"rate_limited"}` (HTTP429) | Veja o site Try It | A chave para IP é baseada em `TRYIT_PROXY_RATE_LIMIT`/`TRYIT_PROXY_RATE_WINDOW_MS` | Use um limite para testes de segurança ou crie uma janela de trabalho (configure `retryAfterMs` em JSON). |
| `{"error":"upstream_timeout"}` (HTTP 504) ou `{"error":"upstream_error"}` (HTTP 502) | Veja o site Try It | Torii não é fornecido para back-end | Verifique o fornecimento `TRYIT_PROXY_TARGET`, instale Torii ou instale-o com o maior `TRYIT_PROXY_TIMEOUT_MS`. |

Mais testes Try It e soluções para OAuth são encontrados em [`devportal/try-it.md`](./try-it.md#norito-rpc-samples).

## Recurso de download
- Transporte RFC: `docs/source/torii/norito_rpc.md`
- Configuração definida: `docs/source/torii/norito_rpc_brief.md`
- Código de rastreamento: `docs/source/torii/norito_rpc_tracker.md`
- Guia Try-It de instruções: `docs/portal/docs/devportal/try-it.md`