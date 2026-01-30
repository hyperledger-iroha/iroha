---
lang: ur
direction: rtl
source: docs/source/soranet/gar_cdn_policy_bus.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b230f98f025e9d55d6eefa2b7b44348ceef3f20505e723fc703151497aeef777
source_last_modified: "2026-01-03T18:08:02.008828+00:00"
translation_last_reviewed: 2026-01-30
---

# GAR CDN Policy Bus

The SNNet-15G enforcement surface now lets operators publish GAR CDN policy
payloads (TTL overrides, purge tags, moderation slugs, rate ceilings, geofence
rules, and legal holds) through a file-backed bus so PoPs receive reproducible
artifacts alongside their receipt bundles.

## Publishing
- Run `cargo xtask soranet-gar-bus --policy <path> [--pop <label>] [--out-dir <dir>]` to read a `GarCdnPolicyV1`
  JSON payload and emit `gar_cdn_policy_event.{json,md}` under the target
  directory (defaults to `artifacts/soranet/gateway/<pop>/gar_bus/`).
- The JSON bundle records the source path, publication timestamp, optional PoP
  label, and the full CDN policy, aligning with PoP evidence packets.

## Gateway consumption
- Gateways load `sorafs.gateway.cdn_policy_path` and apply the same enforcement
  contract (TTL override, purge tags, moderation slugs, rate ceilings,
  geofence/deny lists, legal hold) surfaced in `GatewayPolicy` GAR violations
  and CLI receipt action variants (`ttl_override`, `moderation`).
- Updates to GAR violation events carry the new policy labels, observed TTL,
  region, and rate ceiling hints for dashboards/alerting.
