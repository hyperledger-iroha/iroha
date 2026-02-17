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
  - `connect.session.delete`
  - `connect.status`
- Agent-friendly account/transaction aliases include:
  - `iroha.connect.ws.ticket`
  - `iroha.connect.session.create`
  - `iroha.connect.session.delete`
  - `iroha.connect.status`
  - `iroha.accounts.list`
  - `iroha.accounts.query`
  - `iroha.accounts.resolve`
  - `iroha.accounts.transactions`
  - `iroha.accounts.transactions.query`
  - `iroha.accounts.assets`
  - `iroha.accounts.assets.query`
  - `iroha.accounts.permissions`
  - `iroha.transactions.submit`
  - `iroha.transactions.status`
- `iroha.*` aliases accept flat convenience fields in addition to nested
  `path`/`query`/`body` payloads (for example `account_id`, `hash`, `literal`,
  `signed_tx_base64`, `signed_tx_hex`, and query-envelope shortcuts like
  `filter`, `sort`, `limit`, `offset`).

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

## Transaction Submission Example

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
