# Torii MCP API

Torii exposes a native Model Context Protocol bridge at `/v1/mcp`.
It lets AI agents call Torii and Connect endpoints through JSON-RPC.

## Endpoints

- `GET /v1/mcp` returns MCP capabilities and server metadata.
- `POST /v1/mcp` accepts JSON-RPC 2.0 requests (`initialize`, `tools/list`, `tools/call`).

## JSON-RPC Methods

- `initialize`: return server capabilities and protocol metadata.
- `tools/list`: page through available tools.
- `tools/call`: execute one tool by name.

## Common Flow

1. Call `initialize`.
2. Call `tools/list` (with optional `cursor`) until `nextCursor` is `null`.
3. Call `tools/call` for the target tool.

## Tool Naming

- OpenAPI-derived tools use `torii.<operationId-or-fallback>`.
- Connect helper tools include:
  - `connect.ws.ticket`
  - `connect.session.create`
  - `connect.session.create_and_ticket`
  - `connect.session.delete`
  - `connect.status`
- Agent-friendly account/transaction aliases include:
  - `iroha.connect.ws.ticket`
  - `iroha.connect.session.create`
  - `iroha.connect.session.create_and_ticket`
  - `iroha.connect.session.delete`
  - `iroha.connect.status`
  - `iroha.health`
  - `iroha.status`
  - `iroha.parameters.get`
  - `iroha.node.capabilities`
  - `iroha.contracts.code.register`
  - `iroha.contracts.code.get`
  - `iroha.contracts.deploy`
  - `iroha.contracts.instance.create`
  - `iroha.contracts.instance.activate`
  - `iroha.contracts.call`
  - `iroha.contracts.call_and_wait`
  - `iroha.contracts.state.get`
  - `iroha.accounts.list`
  - `iroha.accounts.get`
  - `iroha.accounts.qr`
  - `iroha.accounts.query`
  - `iroha.accounts.resolve`
  - `iroha.accounts.onboard`
  - `iroha.accounts.transactions`
  - `iroha.accounts.transactions.query`
  - `iroha.accounts.assets`
  - `iroha.accounts.assets.query`
  - `iroha.accounts.permissions`
  - `iroha.accounts.portfolio`
  - `iroha.domains.list`
  - `iroha.domains.get`
  - `iroha.domains.query`
  - `iroha.subscriptions.plans.list`
  - `iroha.subscriptions.plans.create`
  - `iroha.subscriptions.list`
  - `iroha.subscriptions.create`
  - `iroha.subscriptions.get`
  - `iroha.subscriptions.cancel`
  - `iroha.subscriptions.pause`
  - `iroha.subscriptions.resume`
  - `iroha.subscriptions.keep`
  - `iroha.subscriptions.usage`
  - `iroha.subscriptions.charge_now`
  - `iroha.assets.definitions`
  - `iroha.assets.definitions.get`
  - `iroha.assets.definitions.query`
  - `iroha.assets.holders`
  - `iroha.assets.holders.query`
  - `iroha.assets.list`
  - `iroha.assets.get`
  - `iroha.nfts.list`
  - `iroha.nfts.get`
  - `iroha.nfts.query`
  - `iroha.transactions.list`
  - `iroha.transactions.get`
  - `iroha.instructions.list`
  - `iroha.instructions.get`
  - `iroha.blocks.list`
  - `iroha.blocks.get`
  - `iroha.transactions.submit`
  - `iroha.transactions.submit_and_wait`
  - `iroha.transactions.wait`
  - `iroha.transactions.status`
- `iroha.*` aliases accept flat convenience fields in addition to nested
  `path`/`query`/`body` payloads (for example `account_id`, `hash`, `literal`,
  `index`, `instruction_index`, `identifier`, `block_height`, `code_hash`, `signed_tx_base64`, `signed_tx_hex`, `uaid`, `definition_id`, `domain_id`, `subscription_id`, `asset_id`, `nft_id`, and
  query-envelope shortcuts like `filter`, `sort`, `limit`, `offset`).

## Account Tool Example

List accounts using an OpenAPI-derived tool:

```json
{
  "jsonrpc": "2.0",
  "id": "accounts-1",
  "method": "tools/call",
  "params": {
    "name": "torii.get_v1_accounts"
  }
}
```

Query account transactions with path and query arguments:

```json
{
  "jsonrpc": "2.0",
  "id": "acct-tx-1",
  "method": "tools/call",
  "params": {
    "name": "torii.get_v1_accounts_account_id_transactions",
    "arguments": {
      "path": {
        "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
      },
      "query": {
        "limit": 20,
        "offset": 0
      }
    }
  }
}
```

Equivalent flat-argument call via the alias tool:

```json
{
  "jsonrpc": "2.0",
  "id": "acct-tx-2",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.transactions",
    "arguments": {
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
      "limit": 20,
      "offset": 0
    }
  }
}
```

Account assets alias (flat `account_id` + query keys):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-assets-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.assets",
    "arguments": {
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
      "limit": 20,
      "offset": 0
    }
  }
}
```

Account permissions alias (flat `account_id`):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-perm-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.permissions",
    "arguments": {
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
    }
  }
}
```

Account detail alias (flat `account_id` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.get",
    "arguments": {
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
    }
  }
}
```

Account QR alias (flat `account_id` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-qr-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.qr",
    "arguments": {
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
    }
  }
}
```

Accounts query alias (flat query-envelope shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-query-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.query",
    "arguments": {
      "limit": 20,
      "offset": 0
    }
  }
}
```

Account transactions query alias (flat `account_id` + query-envelope shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-tx-query-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.transactions.query",
    "arguments": {
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
      "limit": 20
    }
  }
}
```

Account assets query alias (flat `account_id` + query-envelope shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-assets-query-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.assets.query",
    "arguments": {
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland",
      "limit": 20
    }
  }
}
```

Account onboarding alias (flat shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-onboard-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.onboard",
    "arguments": {
      "alias": "agent-alice",
      "account_id": "ed0120CE7FA46C9DCE7EA4B125E2E36BDB63EA33073E7590AC92816AE1E861B7048B03@wonderland"
    }
  }
}
```

Account portfolio alias (flat `uaid` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "acct-portfolio-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.portfolio",
    "arguments": {
      "uaid": "uaid:00112233445566778899aabbccddeeff00112233445566778899aabbccddeeff"
    }
  }
}
```

Asset holders query alias (flat `definition_id` + query-envelope shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "asset-holders-query-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.assets.holders.query",
    "arguments": {
      "definition_id": "rose#wonderland",
      "limit": 20
    }
  }
}
```

Asset definition detail alias (flat `definition_id` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "asset-def-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.assets.definitions.get",
    "arguments": {
      "definition_id": "rose#wonderland"
    }
  }
}
```

Domains query alias (flat query-envelope shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "domains-query-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.domains.query",
    "arguments": {
      "limit": 20
    }
  }
}
```

Domain detail alias (flat `domain_id` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "domains-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.domains.get",
    "arguments": {
      "domain_id": "wonderland"
    }
  }
}
```

Subscription plans list alias (flat query fields):

```json
{
  "jsonrpc": "2.0",
  "id": "subs-plans-list-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.subscriptions.plans.list",
    "arguments": {
      "provider": "<account-id>",
      "limit": 20
    }
  }
}
```

Subscriptions list alias (flat query fields):

```json
{
  "jsonrpc": "2.0",
  "id": "subs-list-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.subscriptions.list",
    "arguments": {
      "owned_by": "<account-id>",
      "status": "active",
      "limit": 20
    }
  }
}
```

Subscription detail alias (flat `subscription_id` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "subs-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.subscriptions.get",
    "arguments": {
      "subscription_id": "<subscription-nft-id>"
    }
  }
}
```

Subscription plan create alias (`body` payload):

```json
{
  "jsonrpc": "2.0",
  "id": "subs-plans-create-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.subscriptions.plans.create",
    "arguments": {
      "body": {
        "provider": "<account-id>",
        "asset_definition_id": "usd#wonderland",
        "amount": "100"
      }
    }
  }
}
```

Subscription create alias (`body` payload):

```json
{
  "jsonrpc": "2.0",
  "id": "subs-create-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.subscriptions.create",
    "arguments": {
      "body": {
        "plan_id": "<plan-id>",
        "subscriber": "<account-id>"
      }
    }
  }
}
```

Subscription cancel alias (flat `subscription_id` + optional `body`):

```json
{
  "jsonrpc": "2.0",
  "id": "subs-cancel-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.subscriptions.cancel",
    "arguments": {
      "subscription_id": "<subscription-nft-id>",
      "body": {
        "at_period_end": true
      }
    }
  }
}
```

Other subscription action aliases use the same `subscription_id` + optional `body`
shape:

- `iroha.subscriptions.pause`
- `iroha.subscriptions.resume`
- `iroha.subscriptions.keep`
- `iroha.subscriptions.usage`
- `iroha.subscriptions.charge_now`

## Connect Session Lifecycle Example

Create a Connect session:

```json
{
  "jsonrpc": "2.0",
  "id": "connect-create-1",
  "method": "tools/call",
  "params": {
    "name": "connect.session.create",
    "arguments": {
      "sid": "<base64url-32-byte-session-id>"
    }
  }
}
```

`connect.session.create` also accepts the raw request body as `arguments.body`;
if both are provided, `body` takes precedence.

Create session + ticket metadata in one call:

```json
{
  "jsonrpc": "2.0",
  "id": "connect-create-ticket-1",
  "method": "tools/call",
  "params": {
    "name": "connect.session.create_and_ticket",
    "arguments": {
      "sid": "<base64url-32-byte-session-id>",
      "role": "app",
      "node_url": "https://node.example"
    }
  }
}
```

`connect.session.create_and_ticket` returns both `create` (HTTP dispatch output from
`connect.session.create`) and `ticket` (same shape as `connect.ws.ticket`).

Fetch Connect status:

```json
{
  "jsonrpc": "2.0",
  "id": "connect-status-1",
  "method": "tools/call",
  "params": {
    "name": "connect.status"
  }
}
```

Delete a Connect session:

```json
{
  "jsonrpc": "2.0",
  "id": "connect-delete-1",
  "method": "tools/call",
  "params": {
    "name": "connect.session.delete",
    "arguments": {
      "sid": "<base64url-32-byte-session-id>"
    }
  }
}
```

Build WebSocket ticket metadata directly from a create-session response token:

```json
{
  "jsonrpc": "2.0",
  "id": "connect-ticket-1",
  "method": "tools/call",
  "params": {
    "name": "connect.ws.ticket",
    "arguments": {
      "sid": "<session-id>",
      "role": "app",
      "token_app": "<token_app-from-connect.session.create>",
      "node_url": "https://node.example"
    }
  }
}
```

`connect.ws.ticket` accepts either `token` or role-matched aliases
(`token_app` when `role=app`, `token_wallet` when `role=wallet`).

## Node Ops Example

Health/status/parameter aliases for preflight checks:

```json
{
  "jsonrpc": "2.0",
  "id": "node-health-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.health"
  }
}
```

```json
{
  "jsonrpc": "2.0",
  "id": "node-caps-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.node.capabilities"
  }
}
```

## Contract Tool Example

Contract call alias (raw `body` payload):

```json
{
  "jsonrpc": "2.0",
  "id": "contract-call-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.contracts.call",
    "arguments": {
      "body": {
        "authority": "<account-id>",
        "private_key": "<multihash-private-key>",
        "namespace": "nexus",
        "contract_id": "vault",
        "gas_limit": 100000
      }
    }
  }
}
```

Contract state alias (flat query shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "contract-state-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.contracts.state.get",
    "arguments": {
      "prefix": "balance",
      "limit": 20
    }
  }
}
```

Contract call-and-wait alias (one-shot submit + terminal status wait):

```json
{
  "jsonrpc": "2.0",
  "id": "contract-call-wait-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.contracts.call_and_wait",
    "arguments": {
      "body": {
        "authority": "<account-id>",
        "private_key": "<multihash-private-key>",
        "namespace": "nexus",
        "contract_id": "vault",
        "gas_limit": 100000
      },
      "timeout_ms": 30000,
      "poll_interval_ms": 500
    }
  }
}
```

## Block, Transaction, and Instruction Explorer Example

Asset explorer list alias (flat query fields):

```json
{
  "jsonrpc": "2.0",
  "id": "asset-list-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.assets.list",
    "arguments": {
      "page": 1,
      "owned_by": "<account-id>"
    }
  }
}
```

Asset explorer detail alias (flat `asset_id` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "asset-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.assets.get",
    "arguments": {
      "asset_id": "<asset-id>"
    }
  }
}
```

NFT explorer list alias (flat query fields):

```json
{
  "jsonrpc": "2.0",
  "id": "nft-list-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.nfts.list",
    "arguments": {
      "page": 1,
      "owned_by": "<account-id>"
    }
  }
}
```

NFT explorer detail alias (flat `nft_id` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "nft-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.nfts.get",
    "arguments": {
      "nft_id": "<nft-id>"
    }
  }
}
```

NFT query alias (flat query-envelope shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "nft-query-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.nfts.query",
    "arguments": {
      "limit": 20
    }
  }
}
```

Block explorer list alias (flat query fields):

```json
{
  "jsonrpc": "2.0",
  "id": "block-list-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.blocks.list",
    "arguments": {
      "page": 1,
      "per_page": 20
    }
  }
}
```

Block explorer detail alias (flat `identifier`/`block_height` shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "block-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.blocks.get",
    "arguments": {
      "block_height": 1
    }
  }
}
```

Transaction explorer list alias (flat query fields):

```json
{
  "jsonrpc": "2.0",
  "id": "tx-list-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.transactions.list",
    "arguments": {
      "limit": 20,
      "status": "committed"
    }
  }
}
```

Transaction explorer detail alias (flat `hash` shortcut):

```json
{
  "jsonrpc": "2.0",
  "id": "tx-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.transactions.get",
    "arguments": {
      "hash": "<transaction-hash-hex>"
    }
  }
}
```

Instruction explorer list alias (flat query fields):

```json
{
  "jsonrpc": "2.0",
  "id": "instruction-list-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.instructions.list",
    "arguments": {
      "page": 1,
      "per_page": 20
    }
  }
}
```

Instruction explorer detail alias (flat `hash` + `index` shortcuts):

```json
{
  "jsonrpc": "2.0",
  "id": "instruction-get-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.instructions.get",
    "arguments": {
      "hash": "<transaction-hash-hex>",
      "index": 0
    }
  }
}
```

`/transaction` expects Norito-encoded signed transaction bytes.
Send binary payloads through `body_base64`:

```json
{
  "jsonrpc": "2.0",
  "id": "tx-submit-1",
  "method": "tools/call",
  "params": {
    "name": "torii.post_transaction",
    "arguments": {
      "body_base64": "<base64-signed-transaction-bytes>"
    }
  }
}
```

Alias shortcuts accepted by `iroha.transactions.submit`:

- `signed_tx_base64` / `tx_base64` (same as `body_base64`)
- `signed_tx_hex` / `tx_hex` / `body_hex` (hex payload converted to bytes)

Pipeline status alias shortcut:

```json
{
  "jsonrpc": "2.0",
  "id": "tx-status-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.transactions.status",
    "arguments": {
      "hash": "<transaction-hash-hex>"
    }
  }
}
```

One-shot submit + wait alias (submits and polls until terminal status):

```json
{
  "jsonrpc": "2.0",
  "id": "tx-submit-wait-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.transactions.submit_and_wait",
    "arguments": {
      "signed_tx_base64": "<base64-signed-transaction-bytes>",
      "timeout_ms": 30000,
      "poll_interval_ms": 500
    }
  }
}
```

Optional `iroha.transactions.submit_and_wait` controls:

- `hash` / `transaction_hash` (override hash extraction from submission receipt)
- `terminal_statuses` (default: `Committed`, `Applied`, `Rejected`, `Expired`)
- `status_accept` (Accept header for status polling calls; defaults to `application/json`)

Wait on an existing transaction hash (without submitting):

```json
{
  "jsonrpc": "2.0",
  "id": "tx-wait-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.transactions.wait",
    "arguments": {
      "hash": "<transaction-hash-hex>",
      "timeout_ms": 30000,
      "poll_interval_ms": 500
    }
  }
}
```

Optional `iroha.transactions.wait` controls:

- `transaction_hash` (alias for `hash`)
- `terminal_statuses` (default: `Committed`, `Applied`, `Rejected`, `Expired`)
- `status_accept` (Accept header for status polling calls; defaults to `application/json`)

Account resolve alias shortcut:

```json
{
  "jsonrpc": "2.0",
  "id": "acct-resolve-1",
  "method": "tools/call",
  "params": {
    "name": "iroha.accounts.resolve",
    "arguments": {
      "literal": "<account-literal>"
    }
  }
}
```

Optional request metadata:

- `arguments.content_type` (defaults to `application/x-norito` for `body_base64`)
- `arguments.accept`
- `arguments.headers` (forwarded to the dispatched Torii route)

## Response Shape

`tools/call` returns MCP tool content plus structured HTTP dispatch output:

```json
{
  "result": {
    "isError": false,
    "structuredContent": {
      "status": 200,
      "headers": {},
      "content_type": "application/json",
      "body": {}
    }
  }
}
```

When route dispatch fails validation or access checks, `structuredContent.status`
contains the HTTP failure status, `isError` is `true` for HTTP `>= 400`, and
`structuredContent.body` carries the route error payload.
