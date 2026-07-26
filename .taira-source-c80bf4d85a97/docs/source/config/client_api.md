## Client API Configuration Reference

This document tracks the Torii client-facing configuration knobs that are
surfaces through `iroha_config::parameters::user::Torii`. The section below
focuses on the Norito-RPC transport controls introduced for NRPC-1; future
client API settings should extend this file.

### `torii.transport.norito_rpc`

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `enabled` | `bool` | `true` | Master switch that enables binary Norito decoding. When `false`, Torii rejects every Norito-RPC request with `403 norito_rpc_disabled`. |
| `stage` | `string` | `"disabled"` | Rollout tier: `disabled`, `canary`, or `ga`. Stages drive admission decisions and `/rpc/capabilities` output. |
| `require_mtls` | `bool` | `false` | Enforces mTLS for Norito-RPC transport: when `true`, Torii rejects Norito-RPC requests that do not carry an mTLS marker header (e.g. `X-Forwarded-Client-Cert`). The flag is surfaced via `/rpc/capabilities` so SDKs can warn on misconfigured environments. |
| `allowed_clients` | `array<string>` | `[]` | Canary allowlist. When `stage = "canary"`, only requests carrying an `X-API-Token` header present in this list are accepted. |

Example configuration:

```toml
[torii.transport.norito_rpc]
enabled = true
require_mtls = true
stage = "canary"
allowed_clients = ["alpha-canary-token", "beta-canary-token"]
```

Stage semantics:

- **disabled** — Norito-RPC is unavailable even if `enabled = true`. Clients
  receive `403 norito_rpc_disabled`.
- **canary** — Requests must include an `X-API-Token` header that matches one
  of the `allowed_clients`. All other requests receive `403
  norito_rpc_canary_denied`.
- **ga** — Norito-RPC is available to every authenticated caller (subject to the
  usual rate and pre-auth limits).

Operators can update these values dynamically through `/v1/config`. Each change
is reflected immediately in `/rpc/capabilities`, allowing SDKs and observability
dashboards to show the live transport posture.
