---
lang: fr
direction: ltr
source: docs/examples/soranet_incentive_parliament_packet/README.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: b46ad81721ede2a5c95fc95a445267c4970b4a6ce669c75caadc65e2542b73d7
source_last_modified: "2025-11-05T17:22:30.409223+00:00"
translation_last_reviewed: 2026-01-01
---

# Paquet du Parlement des incentives relay SoraNet

Ce bundle capture les artefacts requis par le Parlement Sora pour approuver les paiements automatiques de relay (SNNet-7):

- `reward_config.json` - configuration du moteur de recompenses serialisable Norito, prete a etre ingeree par `iroha app sorafs incentives service init`. Le `budget_approval_id` correspond au hash liste dans les minutes de governance.
- `shadow_daemon.json` - mapping des beneficiaires et bonds consomme par le harness de replay (`shadow-run`) et le daemon de production.
- `economic_analysis.md` - resume de fairness pour la simulation shadow 2025-10 -> 2025-11.
- `rollback_plan.md` - playbook operationnel pour desactiver les paiements automatiques.
- Artefacts de support: `docs/examples/soranet_incentive_shadow_run.{json,pub,sig}`,
  `dashboards/grafana/soranet_incentives.json`,
  `dashboards/alerts/soranet_incentives_rules.yml`.

## Verifications d'integrite

```bash
shasum -a 256 docs/examples/soranet_incentive_parliament_packet/*       docs/examples/soranet_incentive_shadow_run.json       docs/examples/soranet_incentive_shadow_run.sig
```

Comparez les digests avec les valeurs consignees dans les minutes du Parlement. Verifiez la signature shadow-run comme decrit dans
`docs/source/soranet/reports/incentive_shadow_run.md`.

## Mise a jour du paquet

1. Rafraichissez `reward_config.json` des que les poids de recompense, le payout de base ou le hash d'approbation changent.
2. Re-executez la simulation shadow de 60 jours, mettez a jour `economic_analysis.md` avec les nouveaux resultats, et commitez le JSON + la signature detachee.
3. Presentez le bundle mis a jour au Parlement avec les exports de dashboards Observatory lors d'une demande de renouvellement.
