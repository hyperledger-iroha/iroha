---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.fr.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-settlement-faq
titre : Règlement des FAQ
description : Réponses pour les opérateurs comprenant le règlement du routage, la conversion XOR, la télémétrie et les preuves d'audit.
---

Cette page reprend la FAQ interne de règlement (`docs/source/nexus_settlement_faq.md`) pour que les lecteurs du portail puissent consulter les mèmes indications sans fouiller le mono-repo. Elle explique comment le Settlement Router traite les paiements, quelles métriques surveiller et comment les SDK doivent intégrer les payloads Norito.

## Clés de points1. **Mappage des voies** - chaque espace de données déclare un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consultez le dernier catalogue des voies dans `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversion déterministe** - le routeur convertit tous les règlements en XOR via les sources de liquidité approuvées par la gouvernance. Les voies privées préfinancent des tampons XOR ; les coupes de cheveux ne s'appliquent que lorsque les tampons dérivent hors de la politique.
3. **Télémétrie** - surveillez `nexus_settlement_latency_seconds`, les compteurs de conversion et les jauges de coupe de cheveux. Les tableaux de bord se trouvent dans `dashboards/grafana/nexus_settlement.json` et les alertes dans `dashboards/alerts/nexus_audit_rules.yml`.
4. **Preuves** - archivez les configs, logs du routeur, exports de télémétrie et rapports de réconciliation pour les audits.
5. **Responsabilités SDK** - chaque SDK doit exposer les helpers de règlement, les ID de voie et les encodeurs de payloads Norito pour rester alignés avec le routeur.

## Flux d'exemple| Type de voie | Preuves à collectionner | Ce que cela prouve |
|-----------|----------|----------------|
| Privée `xor_hosted_custody` | Log du routeur + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Les tampons CBDC débitent un XOR déterministe et les coupes de cheveux restent dans la politique. |
| Publique `xor_global` | Log du routeur + référence DEX/TWAP + métriques de latence/conversion | Le chemin de liquidité partage a fixe le prix du transfert sur le TWAP publié avec zéro décote. |
| Hybride `xor_dual_fund` | Log du routeur affichant la répartition public vs blindé + compteurs de télémétrie | Le mix blindé/public a respecte les ratios de gouvernance et enregistre la coupe de cheveux appliquée à chaque jambe. |

## Besoin de plus de détails ?

- FAQ complète : `docs/source/nexus_settlement_faq.md`
- Spécification du routeur de règlement : `docs/source/settlement_router.md`
- Playbook de politique CBDC : `docs/source/cbdc_lane_playbook.md`
- Opérations du Runbook : [Opérations Nexus](./nexus-operations)