---
lang: he
direction: rtl
source: docs/portal/i18n/fr/docusaurus-plugin-content-docs/current/nexus/settlement-faq.md
status: complete
generator: scripts/sync_docs_i18n.py
source_hash: f9761159192a85f183278a6915b285b0bbb084d01abdf36bea5e837769754d3d
source_last_modified: "2026-01-03T18:07:59+00:00"
translation_last_reviewed: 2026-01-30
---

---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
id: nexus-settlement-faq
title: FAQ Settlement
description: Reponses pour les operateurs couvrant le routage settlement, la conversion XOR, la telemetrie et les preuves d'audit.
---

Cette page reprend la FAQ interne de settlement (`docs/source/nexus_settlement_faq.md`) pour que les lecteurs du portail puissent consulter les memes indications sans fouiller le mono-repo. Elle explique comment le Settlement Router traite les paiements, quelles metriques surveiller et comment les SDK doivent integrer les payloads Norito.

## Points cles

1. **Mappage des lanes** - chaque dataspace declare un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consultez le dernier catalogue des lanes dans `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversion deterministe** - le router convertit toutes les settlements en XOR via les sources de liquidite approuvees par la gouvernance. Les lanes privees prefinancent des buffers XOR; les haircuts ne s'appliquent que lorsque les buffers derivent hors de la politique.
3. **Telemetrie** - surveillez `nexus_settlement_latency_seconds`, les compteurs de conversion et les jauges de haircut. Les dashboards se trouvent dans `dashboards/grafana/nexus_settlement.json` et les alertes dans `dashboards/alerts/nexus_audit_rules.yml`.
4. **Preuves** - archivez les configs, logs du router, exports de telemetrie et rapports de reconciliation pour les audits.
5. **Responsabilites SDK** - chaque SDK doit exposer des helpers de settlement, des IDs de lane et des encodeurs de payloads Norito pour rester aligne avec le router.

## Flux d'exemple

| Type de lane | Preuves a collecter | Ce que cela prouve |
|-----------|--------------------|----------------|
| Privee `xor_hosted_custody` | Log du router + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Les buffers CBDC debitent un XOR deterministe et les haircuts restent dans la politique. |
| Publique `xor_global` | Log du router + reference DEX/TWAP + metriques de latence/conversion | Le chemin de liquidite partage a fixe le prix du transfert sur le TWAP publie avec zero haircut. |
| Hybride `xor_dual_fund` | Log du router montrant la repartition public vs shielded + compteurs de telemetrie | Le mix shielded/public a respecte les ratios de gouvernance et enregistre le haircut applique a chaque jambe. |

## Besoin de plus de details ?

- FAQ complete: `docs/source/nexus_settlement_faq.md`
- Spec du settlement router: `docs/source/settlement_router.md`
- Playbook de politique CBDC: `docs/source/cbdc_lane_playbook.md`
- Runbook operations: [Operations Nexus](./nexus-operations)
