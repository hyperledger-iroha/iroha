---
id: developer-sdk-index
lang: dz
direction: ltr
source: docs/portal/docs/sorafs/developer-sdk-index.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
title: SoraFS SDK Guides
sidebar_label: SDK Guides
description: Language-specific snippets for integrating SoraFS artefacts.
---

:::note Canonical Source
:::

Use this hub to track the per-language helpers that ship with the SoraFS toolchain.
For Rust-specific snippets jump to [Rust SDK snippets](./developer-sdk-rust.md).

## Language helpers

- **Python** — `sorafs_multi_fetch_local` (local orchestrator smoke tests) and
  `sorafs_gateway_fetch` (gateway E2E exercises) now accept an optional
  `telemetry_region` plus a `transport_policy` override
  (`"soranet-first"`, `"soranet-strict"`, or `"direct-only"`), mirroring the CLI
  rollout knobs. When a local QUIC proxy spins up,
  `sorafs_gateway_fetch` returns the browser manifest under
  `local_proxy_manifest` so tests can hand the trust bundle to browser adapters.
- **JavaScript** — `sorafsMultiFetchLocal` mirrors the Python helper, returning
  payload bytes and receipt summaries, while `sorafsGatewayFetch` exercises
  Torii gateways, threads local proxy manifests, and exposes the same
  telemetry/transport overrides as the CLI.
- **Rust** — services can embed the scheduler directly via
  `sorafs_car::multi_fetch`; see the [Rust SDK snippets](./developer-sdk-rust.md)
  reference for proof-stream helpers and orchestrator integration.
- **Android** — `HttpClientTransport.sorafsGatewayFetch(…)` reuses the Torii HTTP
  executor and honours `GatewayFetchOptions`. Combine it with
  `ClientConfig.Builder#setSorafsGatewayUri` and the PQ upload hint
  (`setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)`) when uploads must stick to
  PQ-only paths.

## Scoreboard and policy knobs

Both the Python (`sorafs_multi_fetch_local`) and JavaScript
(`sorafsMultiFetchLocal`) helpers expose the telemetry-aware scheduler scoreboard
used by the CLI:

- Production binaries enable the scoreboard by default; set `use_scoreboard=True`
  (or provide `telemetry` entries) when replaying fixtures so the helper derives
  weighted provider ordering from advert metadata and recent telemetry snapshots.
- Set `return_scoreboard=True` to receive the computed weights alongside chunk
  receipts so CI logs can capture diagnostics.
- Use `deny_providers` or `boost_providers` arrays to reject peers or add a
  `priority_delta` when the scheduler selects providers.
- Keep the default `"soranet-first"` posture unless staging a downgrade; supply
  `"direct-only"` only when a compliance region must avoid relays or when
  rehearsing the SNNet-5a fallback, and reserve `"soranet-strict"` for PQ-only
  pilots with governance approval.
- Gateway helpers also expose `scoreboardOutPath` and `scoreboardNowUnixSecs`.
  Set `scoreboardOutPath` to persist the computed scoreboard (mirrors the CLI
  `--scoreboard-out` flag) so `cargo xtask sorafs-adoption-check` can validate
  SDK artefacts, and use `scoreboardNowUnixSecs` when fixtures need a stable
  `assume_now` value for reproducible metadata. In the JavaScript helper you
  can additionally set `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata`;
  when the label is omitted it derives `region:<telemetryRegion>` (falling back
  to `sdk:js`). The Python helper automatically emits `telemetry_source="sdk:python"`
  whenever it persists a scoreboard and keeps implicit metadata disabled.

```python
result = sorafs_multi_fetch_local(
    plan_json,
    providers,
    options={
        "use_scoreboard": True,
        "telemetry": [
            {"provider_id": "alpha-id", "qos_score": 98, "last_updated_unix": 4_100_000_000},
            {"provider_id": "beta-id", "penalty": True},
        ],
        "return_scoreboard": True,
        "deny_providers": ["beta"],
        "boost_providers": [{"provider": "alpha", "delta": 25}],
    },
)
for row in result["scoreboard"]:
    print(row["provider_id"], row["eligibility"], row["normalized_weight"])
```
