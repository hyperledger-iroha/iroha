---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.es.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-settlement-faq
titre : FAQ de Règlement
description : Réponses aux opérateurs qui souhaitent obtenir le règlement, la conversion en XOR, la télémétrie et les preuves auditives.
---

Cette page reflète la FAQ interne du règlement (`docs/source/nexus_settlement_faq.md`) pour que les lecteurs du portail révisent la même guide sans se précipiter dans le mono-repo. Explique comment le processus Settlement Router est effectué, qui surveille les mesures et comment le SDK doit intégrer les charges utiles de Norito.

## Descados1. **Mapeo de lanes** - chaque espace de données déclare un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consultez le catalogue des voies les plus récentes en `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversion définitive** - le routeur convertit tous les règlements en XOR à travers les sources de liquidation approuvées par l'administration. Las voies privées tampons préfinanciers XOR ; Les coupes de cheveux s'appliquent uniquement lorsque les tampons sont des plus utiles à la politique.
3. **Telemetria** - vigila `nexus_settlement_latency_seconds`, contadores de conversion et medidors de coupe de cheveux. Les tableaux de bord sont présents en `dashboards/grafana/nexus_settlement.json` et les alertes en `dashboards/alerts/nexus_audit_rules.yml`.
4. **Preuves** - configurations d'archives, journaux du routeur, exportations de télémétrie et informations de conciliation pour les auditoires.
5. **Responsabilités du SDK** - Chaque SDK doit exponer les aides au règlement, les identifiants de voie et les codificateurs de charges utiles Norito pour maintenir la parité avec le routeur.

## Exemples de exemples| Type de voie | Preuve à capturer | Je vous montre |
|-----------|----------|----------------|
| Privée `xor_hosted_custody` | Journal du routeur + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Les tampons CBDC débitent XOR déterministe et les coupes de cheveux sont maintenues dans la politique. |
| Publique `xor_global` | Journal du routeur + référence DEX/TWAP + mesures de latence/conversion | La route de liquidation partage le prix du transfert au TWAP publié avec une coupe de cheveux zéro. |
| Hibrida `xor_dual_fund` | Journal du routeur qui montre la division publique vs blindé + contadores de télémétrie | La mezcla blindée/publique respecte les ratios de gouvernement et enregistre la coupe de cheveux appliquée à chaque trame. |

## Vous avez besoin de plus de détails ?

- FAQ complète : `docs/source/nexus_settlement_faq.md`
- Spécification du routeur de règlement : `docs/source/settlement_router.md`
- Playbook de politique CBDC : `docs/source/cbdc_lane_playbook.md`
- Runbook des opérations : [Opérations de Nexus](./nexus-operations)