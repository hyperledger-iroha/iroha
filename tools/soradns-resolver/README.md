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

### Input and memory corridors

All first-release input byte sizes are admitted before owned decoding. Local
inputs are read from one identity-stable regular-file descriptor through a
`limit + 1` probe. HTTP inputs check `Content-Length` when present and enforce
the same limit while streaming when the length is absent or inaccurate. Norito
JSON is lexically preflighted before materialisation, and binary payloads use
explicit Norito decode limits that reject oversized fields and collections
before allocating them.

- Resolver config: 1 MiB; at most 256 bundle sources, 256 RAD sources, 4,096
  total bundle object references, 64 headers per source, and 64 addresses per
  listener list. A general string/header value is at most 16 KiB and a short
  identifier is at most 4 KiB.
- Static config: at most 4,096 zones and 16,384 records in total, with no more
  than 256 TXT chunks or freeze notes per owning record/zone. Static zones may
  retain at most 16 MiB.
- Proof bundle: 1 MiB encoded; each KSK, ZSK, and delegation collection has at
  most 256 entries. One source's decoded bundle batch retains at most 16 MiB;
  it and the daemon sync map are charged for actual vector/string capacities.
- RAD snapshot: 16 MiB encoded and at most 16,384 entries, with explicit
  32 MiB Norito allocation and 262,144 cumulative-element ceilings.
- Directory CLI: 256 KiB for `record.json`, 16 MiB for `directory.json`, and
  at most 16,384 RAD leaves/files. Merkle levels reuse the admitted leaf vector
  and reserve every successor level fallibly.
- DoT material: 1 MiB for the certificate input and 256 KiB for the private
  key input.
- Live resolver state: at most 16,384 proof bundles, 16,384 RAD adverts, and
  64 MiB of aggregate accounted heap across bundles, RADs, map buckets, and
  static zones. Duplicate-key replacement subtracts the prior retained charge
  before admitting the replacement.

Crossing a corridor rejects that complete input (the existing multi-source
failover may still use other configured sources); it never disables a
transport or silently truncates a valid response.

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
