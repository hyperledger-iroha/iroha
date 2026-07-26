# SoraDNS Resolver Prototype

`soradns-resolver` provides the DG-2 resolver prototype described in the
roadmap. It ingests resolver attestation documents (RAD snapshots) and proof
bundles, tracks bundle/adverts in memory, emits events, and exposes stub
DoH/DoT/DoQ listeners for integration testing.

## Configuration

The daemon loads a Norito JSON document. A minimal example looks like:

```json
{
  "resolver_id": "resolver.sora.test",
  "region": "global",
  "bundle_sources": [{"kind": "file", "value": {"path": "bundles/proof.norito"}}],
  "rad_sources": [{"kind": "torii", "value": {"base_url": "https://torii.dev"}}],
  "doh_listen": ["127.0.0.1:8443"],
  "dot_listen": ["127.0.0.1:853"],
  "doq_listen": ["127.0.0.1:8853"],
  "event_listen": "127.0.0.1:9100",
  "sync_interval_secs": 30
}
```

- `bundle_sources` / `rad_sources` accept `file`, `torii`, or `sorafs` variants
  and may include `headers` blocks for auth tokens.
- `sync_interval_secs` controls how often the daemon re-fetches bundles and RAD
  snapshots once it has started. If omitted it defaults to 30 seconds; values
  below 1 second are rejected.

After editing the config run:

```bash
cargo run -p soradns-resolver -- --config ops/soradns/resolver.json
```

The daemon performs an initial sync and then refreshes state on the configured
interval. Each refresh updates the `/metrics` and `/healthz` endpoints and
emits bundle/resolver events via the SSE listener.

Pass `--sync-interval-secs <seconds>` to the `serve` command to temporarily
override the cadence without editing the configuration file. This is useful for
canary tests that need faster or slower refreshes than the production profile.

## Eventing & validity gates

- Proof bundles and RAD entries are pruned once their validity windows expire
  (or when RAD entries are not yet valid). These removals emit
  `bundle.expired` and `resolver.invalidate` events through the SSE stream and,
  when configured, the on-disk event log.
- If no authoritative static zones are configured the DNS listeners return a
  deterministic `SERVFAIL`, keeping stub deployments predictable while registry
  data is being fetched.
