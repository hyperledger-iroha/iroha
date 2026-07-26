## Taikai Cache Governance Profiles

This directory stores the canonical JSON definitions for SNNet-14 cache
profiles. Each document mirrors the structure consumed by the orchestrator:

```json
{
  "profile_id": "balanced-canary",
  "rollout_stage": "canary",
  "description": "Balanced cache profile for SNNet-14 heap canaries",
  "annotations": {
    "ticket": "SNNet-14D",
    "approved_by": "Ops Guild"
  },
  "taikai_cache": {
    "hot_capacity_bytes": 8388608,
    "hot_retention_secs": 45,
    "warm_capacity_bytes": 33554432,
    "warm_retention_secs": 180,
    "cold_capacity_bytes": 268435456,
    "cold_retention_secs": 3600,
    "qos": {
      "priority_rate_bps": 83886080,
      "standard_rate_bps": 41943040,
      "bulk_rate_bps": 12582912,
      "burst_multiplier": 4
    }
  }
}
```

Run `cargo xtask sorafs-taikai-cache-bundle` to convert the profiles under
`profiles/` into governance-ready bundles (JSON + Norito + manifest) under
`artifacts/taikai_cache`. See `docs/source/taikai_cache_hierarchy.md` for the
rollout playbook.
