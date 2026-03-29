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

The bundled MCP preset targets:

- `https://taira.sora.org/v1/mcp`

That endpoint must be enabled by the deployed validator config before the
plugin can be used. If it returns `404`, the network has not been redeployed
with native Torii MCP yet.

For public write readiness, MCP discovery is not enough. A healthy rollout also
needs a signed canary write to succeed; otherwise public reads can work while
transactions still fail with `route_unavailable`.

## Install from this repo

Use the repo-local marketplace entry in `.agents/plugins/marketplace.json` and
install the `iroha` plugin through Codex.

The plugin assumes the repo is the source of truth and keeps only the Taira
preset committed.

## Standalone Codex skill

This repo also ships a standalone skill at `skills/sora-taira-testnet/` for the
Codex Skills surface.

To install that skill from a GitHub checkout of this repo, use the built-in
installer script from your local Codex environment:

```bash
python3 "${CODEX_HOME:-$HOME/.codex}"/skills/.system/skill-installer/scripts/install-skill-from-github.py \
  --repo <owner>/<repo> \
  --path skills/sora-taira-testnet
```

Restart Codex after installation so the skill appears in the Skills tab.

## Add a custom Torii endpoint

Custom Nexus/Torii networks are intentionally user-local rather than committed
to the repo. Add them with a local MCP entry, for example:

```bash
codex mcp add iroha-custom --url https://<torii>/v1/mcp
```

Keep any network-specific auth headers, bearer tokens, or endpoint overrides in
your local Codex config.

## Public writer-profile expectations

The deployed public profile this plugin expects is:

- `torii.mcp.enabled = true`
- `torii.mcp.profile = "writer"`
- `torii.mcp.expose_operator_routes = false`
- `torii.mcp.allow_tool_prefixes = ["iroha."]`

This makes Codex see the curated `iroha.*` aliases instead of the broader raw
`torii.*` OpenAPI-derived surface.

## Runtime-only key material

Some `iroha.*` write-oriented tools accept JSON request bodies that include
fields such as `authority` and `private_key`, or they submit pre-signed
transaction envelopes.

Those values are runtime-only inputs:

- do not store them in repo config
- do not commit them into plugin manifests or docs
- do not write them to files unless the user explicitly asks for that
- prefer read-only queries until the user clearly asks to mutate live state

When live Taira writes fail with `route_unavailable`, treat that as an ingress
or authoritative-peer deployment issue and rerun
`configs/soranexus/taira/check_mcp_rollout.sh --write-config <runtime-only client.toml>`
rather than debugging the plugin surface first. If the deploy still relies on
hand-edited validator configs, rebuild them from
`configs/soranexus/taira/validator_roster.example.toml` with
`python3 scripts/render_taira_validator_bundle.py --roster ... --output-dir ...`
before cutting traffic again.
