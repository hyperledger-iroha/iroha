## Client API Configuration Reference

This document tracks the Torii client-facing configuration knobs that are
surfaces through `iroha_config::parameters::user::Torii`. The section below
focuses on the Norito-RPC transport controls introduced for NRPC-1; future
client API settings should extend this file.

Client signer configurations pair `account.public_key` with exactly one of
`account.private_key` or `account.private_key_file`. Production profiles use
the file form; it is read once through a 4 KiB-bounded descriptor, must contain
one canonical private key, and on Unix must grant no group/other access. The
client rejects missing or duplicate signer sources before constructing the
key pair.

Clients likewise configure exactly one exact-network source: inline
`network_id` or a canonical `network_id_file`. Production templates
point `network_id_file` at the same `/run/iroha/genesis.expected_hash` artifact
used by validator `genesis.expected_hash_file`; missing, duplicate, multiline,
or non-canonical values fail before a request can be signed. The shared file's
exact content is one checked `hash:<64 uppercase hex>#<CRC16>` NetworkId literal
followed by one LF byte; CRLF and unterminated aliases are rejected.

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
