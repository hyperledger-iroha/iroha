<!-- Auto-generated stub for Portuguese (pt) translation. Replace this content with the full translation. -->

---
id: torii-mcp
lang: pt
direction: ltr
source: docs/portal/docs/reference/torii-mcp.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
source_hash: 316a408473f53a9763a18f40d49cfd766b5b93a3611e277e5a761e366e85c082
source_last_modified: "2026-03-15T11:38:44.302824+00:00"
translation_last_reviewed: 2026-04-02
id: torii-mcp
title: API Torii MCP
description: Guia de referência para usar a ponte nativa do Model Context Protocol do Torii.
---

Torii expõe uma ponte nativa do Model Context Protocol (MCP) em `/v1/mcp`.
Este endpoint permite que os agentes descubram ferramentas e invoquem rotas Torii/Connect por meio de JSON-RPC.

## Formato do ponto final

- `GET /v1/mcp` retorna metadados de recursos (não empacotados em JSON-RPC).
- `POST /v1/mcp` aceita solicitações JSON-RPC 2.0.
- Se `torii.mcp.enabled = false`, nenhuma rota será exposta.
- Se `torii.require_api_token` estiver ativado, o token ausente/inválido será rejeitado antes do envio do JSON-RPC.

## Configuração

Habilite o MCP em `torii.mcp`:

```json
{
  "torii": {
    "mcp": {
      "enabled": true,
      "max_request_bytes": 1048576,
      "max_tools_per_list": 500,
      "profile": "read_only",
      "expose_operator_routes": false,
      "allow_tool_prefixes": [],
      "deny_tool_prefixes": [],
      "rate_per_minute": 240,
      "burst": 120,
      "async_job_ttl_secs": 300,
      "async_job_max_entries": 2000
    }
  }
}
```

Comportamento principal:

- `profile` controla a visibilidade da ferramenta (`read_only`, `writer`, `operator`).
- `allow_tool_prefixes`/`deny_tool_prefixes` aplicam política adicional baseada em nome.
- `rate_per_minute`/`burst` aplicam limitação de token-bucket para solicitações MCP.
- O estado do trabalho assíncrono de `tools/call_async` é retido na memória usando `async_job_ttl_secs` e `async_job_max_entries`.

## Fluxo de cliente recomendado

1. Ligue para `initialize`.
2. Chame `tools/list` e armazene em cache `toolsetVersion`.
3. Use `tools/call` para operações normais.
4. Use `tools/call_async` + `tools/jobs/get` para operações mais longas.
5. Execute novamente `tools/list` quando `listChanged` for `true`.

Não codifique o catálogo completo de ferramentas. Descubra em tempo de execução.

## Métodos e semântica

Métodos JSON-RPC suportados:

-`initialize`
-`tools/list`
-`tools/call`
-`tools/call_batch`
-`tools/call_async`
-`tools/jobs/get`

Notas:- `tools/list` aceita `toolset_version` e `toolsetVersion`.
- `tools/jobs/get` aceita `job_id` e `jobId`.
- `tools/list.cursor` é um deslocamento de string numérica; valores inválidos voltam para `0`.
- `tools/call_batch` é o melhor esforço por item (uma chamada com falha não falha nas chamadas entre irmãos).
- `tools/call_async` valida apenas o formato do envelope imediatamente; erros de execução aparecem posteriormente no estado do trabalho.
- `jsonrpc` deve ser `"2.0"`; omitido `jsonrpc` é aceito para compatibilidade.

## Autenticação e encaminhamento

O despacho MCP não ignora a autorização Torii. As chamadas executam manipuladores de rotas normais e verificações de autenticação.

Torii encaminha cabeçalhos de entrada relacionados à autenticação para envio de ferramentas:

-`Authorization`
-`x-api-token`
-`x-iroha-account`
-`x-iroha-signature`
-`x-iroha-api-version`

Os clientes também podem fornecer cabeçalhos adicionais por chamada via `arguments.headers`.
`content-length`, `host` e `connection` de `arguments.headers` são ignorados.

## Modelo de erro

Camada HTTP:

- `400` JSON inválido
- Token de API `403` rejeitado antes do processamento JSON-RPC
- A carga útil `413` excede `max_request_bytes`
- `429` com taxa limitada
- `200` para respostas JSON-RPC (incluindo erros JSON-RPC)

Camada JSON-RPC:- `error.data.error_code` de nível superior é estável (por exemplo `invalid_request`, `invalid_params`, `tool_not_found`, `tool_not_allowed`, `job_not_found`, `rate_limited`).
- Falhas de ferramentas surgem como resultados de ferramentas MCP com `isError = true` e detalhes estruturados.
- Falhas de ferramentas despachadas por rota mapeiam o status HTTP para `structuredContent.error_code` (por exemplo, `forbidden`, `not_found`, `server_error`).

## Nomeação de ferramentas

As ferramentas derivadas de OpenAPI usam nomes estáveis baseados em rotas:

-`torii.<method>_<path...>`
- Exemplo: `torii.get_v1_accounts`

Os aliases selecionados também são expostos em `iroha.*` e `connect.*`.

## Especificação canônica

O contrato completo de nível de fio é mantido em:

-`crates/iroha_torii/docs/mcp_api.md`

Quando o comportamento muda em `crates/iroha_torii/src/mcp.rs` ou `crates/iroha_torii/src/lib.rs`,
atualize essa especificação na mesma alteração e, em seguida, espelhe as orientações de uso da chave aqui.