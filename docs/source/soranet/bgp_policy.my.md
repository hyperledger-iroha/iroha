---
lang: my
direction: ltr
source: docs/source/soranet/bgp_policy.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 7219a11fe701f05bb5c481ca9faf65eba4cf27ad666c8b18ade2c957365430cd
source_last_modified: "2026-01-05T09:28:12.088272+00:00"
translation_last_reviewed: 2026-02-07
---

<!--
  SPDX-License-Identifier: Apache-2.0
-->

# SoraNet BGP Policy Templates (SNNet-15A2)

Roadmap item **SNNet-15A2 – BGP policy & monitoring suite** requires repeatable
FRR templates, BFD thresholds, RPKI enforcement hooks, and dashboards that
surface prefix churn and downgrade risks. This guide introduces the JSON schema
used by the new `cargo xtask soranet-pop-template` helper and documents the
expectations for PoP operators ahead of the SNNet-15A roll-out.

## Descriptor Schema

Each PoP describes its desired FRR configuration as Norito JSON. Descriptors
live alongside operator evidence (e.g., `fixtures/soranet_pop/`) and contain:

| Field | Required | Description |
|-------|----------|-------------|
| `pop_name` | ✅ | Human-friendly POP/region label (used for hostname tags). |
| `asn` | ✅ | Public AS number assigned to the edge router. |
| `router_id` | ✅ | IPv4 router-id supplied to `bgp router-id`. |
| `loopback_ipv4` | ✅ | IPv4 loopback used for `network` announcements. |
| `loopback_ipv6` | optional | IPv6 loopback for dual-stack PoPs. |
| `bfd_profiles[]` | optional | Named BFD profiles (`name`, `transmit_ms`, `receive_ms`, `detect_multiplier`). |
| `rpki_caches[]` | optional | RTR cache endpoints (`address`, `port`, optional `preference`/`description`). |
| `prefixes.ipv4`/`prefixes.ipv6` | optional | Networks to advertise in the respective address families. |
| `neighbors[]` | ✅ | One entry per adjacency. Fields include `name`, `address`, `peer_asn`, optional `bfd_profile`, `local_pref`, `med`, `import_rpki_deny_invalid`, `max_prefix`, `send_community_both`, `password`, timer overrides, and `multihop_ttl`. |

See `fixtures/soranet_pop/sjc01.json` for a fully populated example that
exercises dual-stack networks, BFD, and RPKI enforcement.

## Rendering an FRR Template

Use the xtask helper to turn the JSON descriptor into an FRR configuration:

```bash
cargo xtask soranet-pop-template \
  --input fixtures/soranet_pop/sjc01.json \
  --output artifacts/soranet_pop/sjc01.frr.conf
```

When `--output` is omitted the command writes to
`artifacts/soranet_pop/frr.conf`. The generator performs the following steps:

1. Validates that each neighbor references a known BFD profile (if present) and
   that IPv4/IPv6 addresses are syntactically correct.
2. Emits deterministic header boilerplate (`frr defaults datacenter`,
   hostname/log directives, etc.).
3. Defines every BFD profile and RPKI cache before generating the `router bgp`
   block.
4. Produces route-maps for import/export policy overrides. Local preference
   adjustments attach to `RM_<NEIGH>_IMPORT`, MED adjustments use
   `RM_<NEIGH>_EXPORT`, and `import_rpki_deny_invalid = true` injects a
   `match rpki invalid` deny stanza ahead of the permit rules so invalid routes
   never enter the RIB.
5. Activates address-family sections for IPv4/IPv6 only when prefixes are
   provided, keeping router configs minimal and deterministic.

All rendered configs end with a `! metadata:` line that records the PoP name and
neighbor count so audit tooling can fingerprint artefacts without scraping
comments.

## BFD & RPKI Guardrails

* **BFD:** The Stage A template uses a single `fast-default` profile (300 ms
  transmit/receive, multiplier 3); heavier PoPs can add per-neighbor profiles in
  the descriptor. The CLI enforces that neighbor references point at a defined
  profile, preventing typos from silently disabling liveness checks.
* **RPKI:** The RPKI block lists every RTR cache to be polled. Set
  `import_rpki_deny_invalid: true` on neighbors that require hard enforcement.
  The generator emits a deny clause (`match rpki invalid`) before the permit
  entry, matching the policy mandated by SNNet-15A.
* **Local preference & MED:** Use `local_pref` and `med` fields per neighbor to
  express uplink ordering. The helper attaches well-named route-maps and uses
  deterministic sequence numbers (`deny 5`, `permit 10`) so policy diffs stay
  easy to audit.

## Monitoring & Dashboards

The Grafana board `dashboards/grafana/soranet_bgp_policy.json` surfaces the
following Prometheus metrics:

- `soranet_bgp_announcements_total{neighbor=...}` – per-neighbor announcement
  churn.
- `soranet_bgp_rpki_invalid_total{neighbor=...}` – invalid prefixes blocked by
  the generated route-maps.
- `soranet_bgp_med_observed` / `soranet_bgp_local_pref_observed` – telemetry
  mirroring the exported MED and local-pref decisions so operators can confirm
  parity across PoPs.

Add the dashboard to your region’s Grafana folders and pin it to the SNNet
operations board before Stage A rehearsals. The panel definitions rely on a
templated Prometheus data source (`PROMETHEUS` UID) to stay aligned with the
existing observability stack.

## Additional Resources

- `deploy/soranet/frr/README.md` – summarized operator workflow and file layout.
- `fixtures/soranet_pop/sjc01.json` – canonical descriptor for CI/demos.
- `cargo xtask soranet-pop-template --help` – latest CLI synopsis.
