---
lang: pt
direction: ltr
source: docs/source/config/client_api.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: fa548ec31fe928decc5c23719472618ff97f4eb45b084f9f9084df82b96cfac6
source_last_modified: "2026-01-03T18:07:57.683798+00:00"
translation_last_reviewed: 2026-02-07
translator: machine-google-reviewed
---

## Referência de configuração da API do cliente

Este documento rastreia os botões de configuração voltados para o cliente Torii que são
superfícies através de `iroha_config::parameters::user::Torii`. A seção abaixo
concentra-se nos controles de transporte Norito-RPC introduzidos para NRPC-1; futuro
as configurações da API do cliente devem estender este arquivo.

### `torii.transport.norito_rpc`

| Chave | Tipo | Padrão | Descrição |
|-----|------|------------|-------------|
| `enabled` | `bool` | `true` | Chave mestre que permite a decodificação binária Norito. Quando `false`, Torii rejeita todas as solicitações Norito-RPC com `403 norito_rpc_disabled`. |
| `stage` | `string` | `"disabled"` | Camada de implementação: `disabled`, `canary` ou `ga`. Os estágios orientam as decisões de admissão e a saída `/rpc/capabilities`. |
| `require_mtls` | `bool` | `false` | Aplica mTLS para transporte Norito-RPC: quando `true`, Torii rejeita solicitações Norito-RPC que não carregam um cabeçalho de marcador mTLS (por exemplo, `X-Forwarded-Client-Cert`). O sinalizador é exibido por meio de `/rpc/capabilities` para que os SDKs possam alertar sobre ambientes mal configurados. |
| `allowed_clients` | `array<string>` | `[]` | Lista de permissões canário. Quando `stage = "canary"`, apenas serão aceitas solicitações que contenham o cabeçalho `X-API-Token` presente nesta lista. |

Configuração de exemplo:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Semântica de palco:

- **desativado** — Norito-RPC não está disponível mesmo se `enabled = true`. Clientes
  receber `403 norito_rpc_disabled`.
- **canary** — As solicitações devem incluir um cabeçalho `X-API-Token` que corresponda a um
  do `allowed_clients`. Todas as outras solicitações recebem `403
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC está disponível para todos os chamadores autenticados (sujeito ao
  taxa normal e limites de pré-autorização).

Os operadores podem atualizar esses valores dinamicamente por meio de `/v2/config`. Cada mudança
é refletido imediatamente em `/rpc/capabilities`, permitindo SDKs e observabilidade
painéis para mostrar a postura de transporte ao vivo.