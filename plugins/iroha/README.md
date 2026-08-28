# Iroha Codex Plugin

This plugin packages the deployed-network Iroha workflow for Codex around
native Torii MCP.

It is intentionally optimized for live SORA/Torii networks such as Taira and
future Nexus deployments, not contributor-local repo workflows.

## Included surfaces

- `.codex-plugin/plugin.json` — plugin metadata for Codex
- `.mcp.json` — built-in Taira MCP preset
- `skills/iroha-live-network/` — guidance for safe live-network usage

## Built-in preset

The bundled MCP preset currently targets the primary public Taira endpoint:

- `https://taira.sora.org/v1/mcp`

If your deployment or operator gives you a different public Torii root for the
environment under test, override it locally instead of editing the committed
repo preset.

That endpoint must be enabled by the deployed validator config before the
plugin can be used. Torii MCP is POST-only, so `GET /v1/mcp` returning `405` is
expected. A valid POST returning `503` with `mcp_disabled` means the node has
not enabled native Torii MCP.

For public write readiness, MCP discovery is not enough. A healthy rollout also
needs a signed canary write to succeed; otherwise public reads can work while
transactions still fail with `route_unavailable`.

## Install from this repo

Use the repo-local marketplace entry in `.agents/plugins/marketplace.json` and
install the `iroha` plugin through Codex.

The plugin assumes the repo is the source of truth and keeps the current
primary public Taira root committed. Alternate public roots should stay
user-local.

## Standalone Codex skill

This repo also ships standalone skills for the Codex Skills surface:

- `skills/sora-taira-testnet/` for Taira testnet workflows
- `skills/sora-minamoto-mainnet/` for Minamoto mainnet workflows

To install a skill from a GitHub checkout of this repo, use the built-in
installer script from your local Codex environment and pass the desired skill
path:

```bash
python3 "${CODEX_HOME:-$HOME/.codex}"/skills/.system/skill-installer/scripts/install-skill-from-github.py \
  --repo <owner>/<repo> \
  --path skills/sora-minamoto-mainnet
```

Restart Codex after installation so the skill appears in the Skills tab.

## Add a custom Torii endpoint

Additional public roots and custom Nexus/Torii networks are intentionally
user-local rather than committed to the repo. Add them with a local MCP entry,
for example:

```bash
codex mcp add iroha-custom --url https://<torii>/v1/mcp
```

Keep any network-specific auth headers, bearer tokens, or endpoint overrides in
your local Codex config.

## Public writer-profile expectations

The deployed public profile this plugin expects is:

- `torii.mcp.enabled = true`
- `torii.mcp.max_inflight_dispatches = 32` (or an operator-selected bounded value)
- `torii.mcp.profile = "writer"`
- `torii.mcp.expose_operator_routes = false`
- `torii.mcp.allow_tool_prefixes = ["iroha."]`

This makes Codex see the curated `iroha.*` aliases instead of the broader raw
`torii.*` OpenAPI-derived surface.

Operator tools require both `torii.mcp.profile = "operator"` and
`torii.mcp.expose_operator_routes = true`. Neither setting enables them alone.

## Protocol and discovery

Codex performs the MCP lifecycle for the bundled server. A manual client must:

- POST exactly one JSON-RPC request or notification per HTTP request; GET and
  outer JSON-RPC arrays are not supported
- initialize with `protocolVersion`, `capabilities`, and `clientInfo`, then send
  the negotiated `MCP-Protocol-Version` header on later requests
- use exact names and schemas returned by `tools/list`
- consult each tool's read-only, destructive, and idempotent annotations; the
  initialization response reports the real Torii package version
- use the advertised `tools/call_batch` extension for batching, understanding
  that every inner call is charged against the rate limit and must acquire one
  of the configured in-flight dispatch slots

Requests without `Origin` are supported for non-browser clients. If a browser
sends `Origin`, the value must exactly match one node-operator CORS allowlist
entry; wildcard, duplicate, and unlisted values are rejected.

## Connect tool names

Use only these canonical Connect tools:

- `iroha.connect.session.create`
- `iroha.connect.ws.ticket`
- `iroha.connect.session.status`
- `iroha.connect.session.delete`

The ticket call requires an explicit trusted `node_url`. Bare `connect.*`
aliases and `iroha.connect.session.create_and_ticket` are retired; create the
session and build its ticket as separate explicit steps.

Faucet prepare and submit are not MCP tools. Keep faucet qualification in the
CLI-owned public-reset workflow until the claim is consumed atomically on-ledger.

## Runtime credentials and signing

Never send a raw private key, seed phrase, or signing key file through MCP.
Build unsigned instructions where supported, sign locally, and submit a
pre-signed transaction envelope with `iroha.transactions.submit_and_wait`.

Bearer tokens and forwarded authentication headers are runtime-only inputs:

- do not store them in repo config
- do not commit them into plugin manifests or docs
- do not write them to files or logs
- prefer read-only queries until the user clearly asks to mutate live state

When live Taira writes fail with `route_unavailable`, treat that as an ingress
or authoritative-peer deployment issue and run the same-revision compiled
`iroha taira doctor` against the exact public Torii root before debugging the
plugin surface. Use `iroha taira write-canary` only when the user has explicitly
authorized that workflow, and keep its signing operation outside MCP.
