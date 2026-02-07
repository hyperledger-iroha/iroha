---
lang: mn
direction: ltr
source: docs/source/soranet/lane_profiles.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 4c54af03ac031f706287033ae7158c6d58c76ef275cf88d4127ed0a3f296ec5a
source_last_modified: "2025-12-29T18:16:36.189286+00:00"
translation_last_reviewed: 2026-02-07
---

# SoraNet lane profiles

`network.lane_profile` controls the default connection caps and low-priority
uplink budgets applied to the peer-to-peer layer. The **core** profile mirrors
the previous behaviour (caps unset unless explicitly configured) and is aimed
at datacentre/validator deployments. The **home** profile applies conservative
limits for constrained links and derives per-peer budgets automatically.

| Profile | Tick (ms) | MTU hint (bytes) | Target uplink (Mbps) | Constant-rate neighbours | Max incoming | Max total | Per-peer low-priority budget | Per-peer low-priority rate |
| --- | --- | --- | --- | --- | --- | --- | --- | --- |
| core | 5 | 1500 | inherit config | inherit config | inherit config | inherit config | inherit config | inherit config |
| home | 10 | 1400 | 120 | 12 | 12 | 32 | 1,250,000 B/s (~10 Mbps) | 947 msgs/s (assumes 1,320 B payloads) |

### Behaviour

- The selected profile is recorded in the config surface (`network.lane_profile`)
  so CLI/SDK callers can report which shaping preset is active.
- Home automatically fills `max_incoming`, `max_total_connections`,
  `low_priority_bytes_per_sec`, and `low_priority_rate_per_sec` when those
  fields are not set. Explicit values still win if you provide them.
- Switching the profile through the config update API reapplies the derived caps
  so constrained links auto-disable extra lanes without bespoke tuning.
- MTU hints are advisory; if you override per-topic frame caps, keep them within
  the stated payload envelope to avoid fragmentation on home/consumer links.

### Example

```toml
[network]
lane_profile = "home"               # apply home shaping defaults
# Optional overrides if you need stricter or looser caps:
# max_total_connections = 40
# low_priority_bytes_per_sec = 900000
```
