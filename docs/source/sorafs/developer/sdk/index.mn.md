---
lang: mn
direction: ltr
source: docs/source/sorafs/developer/sdk/index.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 199872a42bc82165e97f09025ba4c0b2a7b288ee701ad964a0ec18b8c8103d1c
source_last_modified: "2025-12-29T18:16:36.115150+00:00"
translation_last_reviewed: 2026-02-07
title: SDK Guides
summary: Language-specific snippets for integrating SoraFS artefacts.
---

# SDK Guides

```{toctree}
:maxdepth: 1

rust
```

## Language helpers

- Python bindings expose `sorafs_multi_fetch_local` for local orchestrator
  smoke tests and `sorafs_gateway_fetch` for end-to-end gateway exercises in CI;
  both accept an optional `telemetry_region` to label metrics and a
  `transport_policy` override (`"soranet-first"`, `"soranet-strict"`, or
  `"direct-only"`) so scripts can mirror production rollout policies. When the
  orchestrator spawns a local QUIC proxy, `sorafs_gateway_fetch` now returns the
  browser manifest under `local_proxy_manifest`, matching the CLI payload so
  tests can hand the trust bundle to the browser extension or SDK adapters.
- The JavaScript host publishes `sorafsMultiFetchLocal` with the same
  semantics, returning payload bytes and receipt summaries, and
  `sorafsGatewayFetch` to exercise Torii gateways. The gateway helper
  threads local proxy manifests (when configured) and CAR verification
  metadata, mirroring the CLI output so browser adapters and SDKs can
  reuse the trust bundle.
- Rust projects can continue to embed the scheduler directly via
  `sorafs_car::multi_fetch`.
- Android clients can reuse `HttpClientTransport.sorafsGatewayFetch(…)` once
  `ClientConfig.Builder#setSorafsGatewayUri` points at the gateway. The helper
  shares the Torii HTTP executor and honours `GatewayFetchOptions` (including
  `setWriteModeHint(WriteModeHint.UPLOAD_PQ_ONLY)` when uploads must enforce
  PQ-only paths).

## Scoreboard and policy knobs

The Python (`sorafs_multi_fetch_local`) and JavaScript (`sorafsMultiFetchLocal`)
helpers now expose the same telemetry-aware scoreboard used by the CLI:

- Production binaries enable the scoreboard by default. When using the local
  helper, set `use_scoreboard=True` (or supply `telemetry` records) to derive
  weighted provider ordering from advert metadata and recent telemetry snapshots
  so fixtures mirror the shipping behaviour.
- Set `return_scoreboard=True` to receive the computed scoreboard alongside
  chunk receipts, making it easy to surface scoring diagnostics in CI logs.
- Toggle `deny_providers` or `boost_providers` to reject specific peers or add a
  `priority_delta` when the scheduler selects providers; both bindings accept
  the same option shapes.
- Keep the default `"soranet-first"` posture unless staging a downgrade; supply
  `"direct-only"` in `sorafs_gateway_fetch`/`sorafsMultiFetchLocal` only when a
  compliance region must avoid relays or when rehearsing the SNNet-5a fallback.
  Use `"soranet-strict"` exclusively for PQ-only pilots once governance
  approves the stricter posture.
- Gateway helpers also expose `scoreboardOutPath` and `scoreboardNowUnixSecs`.
  Set `scoreboardOutPath` to persist the computed scoreboard (mirrors the CLI
  `--scoreboard-out` flag) so `cargo xtask sorafs-adoption-check` can validate
  SDK artefacts, and use `scoreboardNowUnixSecs` when fixtures need a stable
  `assume_now` value for reproducible metadata. In the JavaScript helper you
  can also set `scoreboardTelemetryLabel`/`scoreboardAllowImplicitMetadata` to
  mirror the CLI metadata exactly; when the label is omitted the helper derives
  `region:<telemetryRegion>` (falling back to `sdk:js`). The Python helper
  automatically emits `telemetry_source="sdk:python"` whenever a scoreboard is
  persisted and keeps implicit metadata disabled.

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
