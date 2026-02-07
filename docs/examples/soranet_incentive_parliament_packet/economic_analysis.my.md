---
lang: my
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-12-29T18:16:35.087502+00:00"
translation_last_reviewed: 2026-02-07
---

# Economic Analysis - 2025-10 -> 2025-11 Shadow Run

Source artefact: `docs/examples/soranet_incentive_shadow_run.json` (signature +
public key in the same directory). The simulation replayed 60 epochs per relay
with the reward engine pinned to `RewardConfig` recorded in
`reward_config.json`.

## Distribution Summary

- **Total payouts:** 5,160 XOR over 360 rewarded epochs.
- **Fairness envelope:** Gini coefficient 0.121; top relay share 23.26%
  (well below the 30% governance guardrail).
- **Availability:** fleet average 96.97%, all relays remained above 94%.
- **Bandwidth:** fleet average 91.20%, with the lowest performer at 87.23%
  during planned maintenance; penalties were applied automatically.
- **Compliance noise:** 9 warning epochs and 3 suspensions were observed and
  translated into payout reductions; no relay exceeded the 12-warning cap.
- **Operational hygiene:** no metrics snapshots were skipped due to missing
  config, bonds, or duplicates; no calculator errors were emitted.

## Observations

- Suspensions correspond to epochs where relays entered maintenance mode. The
  payout engine emitted zero payouts for those epochs while preserving the
  audit trail in the shadow-run JSON.
- Warning penalties shaved 2% off the affected payouts; the resulting
  distribution still converges thanks to the uptime/bandwidth weights (650/350
  per mille).
- Bandwidth variance tracks the anonymised guard heatmap. The lowest performer
  (`6666...6666`) retained 620 XOR across the window, above the 0.6x floor.
- Latency-sensitive alerts (`SoranetRelayLatencySpike`) remained below warning
  thresholds throughout the window; correlated dashboards are captured under
  `dashboards/grafana/soranet_incentives.json`.

## Recommended Actions Before GA

1. Keep running monthly shadow replays and update the artefact set and this
   analysis if the fleet composition changes.
2. Gate automatic payouts on the Grafana alert suite referenced in the roadmap
   (`dashboards/alerts/soranet_incentives_rules.yml`); copy screenshots into the
   governance minutes when seeking renewal.
3. Re-run the economic stress test if base reward, uptime/bandwidth weights, or
   the compliance penalty changes by >=10%.
