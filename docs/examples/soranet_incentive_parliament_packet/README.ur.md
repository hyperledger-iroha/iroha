---
lang: ur
direction: rtl
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

<div dir="rtl">

<!-- docs/examples/soranet_incentive_parliament_packet/README.md کا اردو ترجمہ -->

# SoraNet Relay Incentive Parliament Packet

یہ bundle وہ artefacts محفوظ کرتا ہے جو Sora Parliament کو automatic relay payouts (SNNet-7) منظور کرنے کے لئے درکار ہیں:

- `reward_config.json` - Norito-serialisable reward engine configuration، جو `iroha app sorafs incentives service init` کے ذریعے ingest کے لئے تیار ہے۔ `budget_approval_id` governance minutes میں درج hash سے میچ کرتا ہے.
- `shadow_daemon.json` - beneficiary اور bond mapping جو replay harness (`shadow-run`) اور production daemon میں استعمال ہوتی ہے.
- `economic_analysis.md` - 2025-10 -> 2025-11 shadow simulation کے لئے fairness summary.
- `rollback_plan.md` - automatic payouts کو disable کرنے کے لئے operational playbook.
- Supporting artefacts: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Integrity Checks

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

digests کو Parliament minutes میں درج values کے ساتھ compare کریں۔ shadow-run signature کو
`docs/source/soranet/reports/incentive_shadow_run.md` میں بیان کردہ طریقے سے verify کریں.

## Updating the Packet

1. جب reward weights، base payout، یا approval hash میں تبدیلی ہو تو `reward_config.json` ریفریش کریں.
2. 60 دن کی shadow simulation دوبارہ چلائیں، `economic_analysis.md` کو نئے نتائج سے اپڈیٹ کریں، اور JSON + detached signature pair commit کریں.
3. renewed approval کے لئے Parliament کو updated bundle Observatory dashboards exports کے ساتھ پیش کریں.

</div>
