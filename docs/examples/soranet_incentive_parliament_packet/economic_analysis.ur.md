---
lang: ur
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/economic_analysis.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: 1b453559c05401edc11894e585c8d5ca4b678d4667c1cef0415582e1f7de8246
source_last_modified: "2025-11-05T17:23:10.790688+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_incentive_parliament_packet/economic_analysis.md کا اردو ترجمہ -->

# Economic Analysis - 2025-10 -> 2025-11 Shadow Run

Source artefact: `docs/examples/soranet_incentive_shadow_run.json` (signature + public key اسی ڈائریکٹری میں). simulation نے ہر relay کے لئے 60 epochs replay کئے اور reward engine کو `reward_config.json` میں درج `RewardConfig` پر pin رکھا.

## Distribution Summary

- **Total payouts:** 5,160 XOR (360 rewarded epochs).
- **Fairness envelope:** Gini coefficient 0.121; top relay share 23.26%
  (30% governance guardrail سے بہت کم).
- **Availability:** fleet average 96.97%, تمام relays 94% سے اوپر رہے.
- **Bandwidth:** fleet average 91.20%, lowest performer 87.23%
  planned maintenance کے دوران; penalties خودکار طور پر لگیں.
- **Compliance noise:** 9 warning epochs اور 3 suspensions دیکھے گئے اور
  payouts میں کمی میں تبدیل ہوئے; کوئی relay 12-warning cap سے اوپر نہیں گیا.
- **Operational hygiene:** missing config/bonds/duplicates کی وجہ سے کوئی metrics snapshots
  skip نہیں ہوئے; کوئی calculator errors emit نہیں ہوئے.

## Observations

- Suspensions ان epochs کے ساتھ مربوط ہیں جب relays maintenance mode میں گئے۔ payout engine نے
  ان epochs کے لئے zero payouts emit کئے جبکہ shadow-run JSON میں audit trail محفوظ رکھا.
- Warning penalties نے متاثرہ payouts سے 2% کم کیا؛ distribution اب بھی converge کرتا ہے کیونکہ
  uptime/bandwidth weights (650/350 per mille) موجود ہیں.
- Bandwidth variance anonymised guard heatmap کو track کرتی ہے۔ سب سے کم performer
  (`6666...6666`) نے window میں 620 XOR رکھا، 0.6x floor سے اوپر.
- Latency-sensitive alerts (`SoranetRelayLatencySpike`) پوری window میں warning thresholds
  سے نیچے رہے؛ متعلقہ dashboards `dashboards/grafana/soranet_incentives.json` میں محفوظ ہیں.

## Recommended Actions Before GA

1. ماہانہ shadow replays جاری رکھیں اور اگر fleet composition بدلے تو artefact set اور یہ
   analysis اپڈیٹ کریں.
2. roadmap میں درج Grafana alert suite پر automatic payouts کو gate کریں
   (`dashboards/alerts/soranet_incentives_rules.yml`); renewal کے وقت screenshots کو
   governance minutes میں شامل کریں.
3. اگر base reward، uptime/bandwidth weights، یا compliance penalty >=10% بدلے تو
   economic stress test دوبارہ چلائیں.

</div>
