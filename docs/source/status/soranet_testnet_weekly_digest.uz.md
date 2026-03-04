---
lang: uz
direction: ltr
source: docs/source/status/soranet_testnet_weekly_digest.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: e87ff17c92df5f22db362da249242d7570e9c6e7e53c6d39452061d846cd1757
source_last_modified: "2025-12-29T18:16:36.213964+00:00"
translation_last_reviewed: 2026-02-07
title: SoraNet Testnet Weekly Digest Template
summary: Structured checklist for weekly SNNet-10 status updates.
---

# SoraNet Testnet Weekly Digest — Week of {{ week_start }}

## Network Snapshot

- Relay count: {{ relay_count }} (target {{ target_relay_count }})
- PQ-capable relays: {{ pq_relays }} ({{ pq_ratio }}%)
- Average guard rotation age: {{ guard_rotation_hours }} hours
- Brownout incidents this week: {{ brownout_count }}

## Key Metrics

| Metric | Value | Target | Notes |
|--------|-------|--------|-------|
| `soranet_privacy_circuit_events_total{kind="downgrade"}` (per min) | {{ downgrade_rate }} | 0 | {{ downgrade_notes }} |
| `sorafs_orchestrator_policy_events_total{outcome="brownout"}` (per 30 min) | {{ brownout_rate }} | <0.05 | {{ policy_notes }} |
| PoW median solve time | {{ pow_median_ms }} ms | ≤300 ms | {{ pow_notes }} |
| Circuit RTT p95 | {{ rtt_p95_ms }} ms | ≤200 ms | {{ rtt_notes }} |

## Compliance & GAR

- Opt-out catalogue version: {{ opt_out_tag }}
- Operator compliance confirmations received this week: {{ compliance_forms }}
- Outstanding actions: {{ compliance_actions }}

## Incidents & Drills

- [ ] Brownout drill executed (date/time, outcome)
- [ ] Downgrade comms template exercised (link to record)
- [ ] Rollback rehearsal performed (summary)

## Upcoming Milestones

- {{ milestone_one }}
- {{ milestone_two }}

## Notes & Decisions

- {{ decision_one }}
- {{ decision_two }}
