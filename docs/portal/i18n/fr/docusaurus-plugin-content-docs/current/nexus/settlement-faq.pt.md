---
lang: fr
direction: ltr
source: docs/portal/docs/nexus/settlement-faq.pt.md
status: complete
generator: docs/portal/scripts/sync-i18n.mjs
translator: machine-google-reviewed
translation_last_reviewed: 2026-02-07
---

---
identifiant : nexus-settlement-faq
titre : FAQ de Règlement
description : Respostas para operadores cobrindo roteamento de règlement, conversation XOR, telemetria e evidencia de auditoria.
---

Cette page contient la FAQ interne du règlement (`docs/source/nexus_settlement_faq.md`) pour que les lecteurs du portail révisent leur orientation sem-vasculaire ou mono-repo. Expliquer comment le Settlement Router traite les paiements, quels paramètres sont surveillés et comment les SDK développent les charges utiles Norito.

## Destaques

1. **Mapeamento de lanes** - chaque espace de données déclare un `settlement_handle` (`xor_global`, `xor_lane_weighted`, `xor_hosted_custody` ou `xor_dual_fund`). Consultez le catalogue des voies le plus récent dans `docs/source/project_tracker/nexus_config_deltas/`.
2. **Conversation déterminée** - Le routeur convertit tous les règlements pour XOR pour les polices de liquidation approuvées par la gouvernance. Voies privées tampons de préfinancement XOR ; les coupes de cheveux sont donc appliquées lorsque les tampons sont supprimés de la politique.
3. **Telemetria** - moniteur `nexus_settlement_latency_seconds`, contadores de conversation et medidors de coupe de cheveux. Les tableaux de bord sont affichés sur `dashboards/grafana/nexus_settlement.json` et alertes sur `dashboards/alerts/nexus_audit_rules.yml`.
4. **Preuve** - archiver les configurations, les journaux du routeur, les exportations de télémétrie et les rapports de réconciliation pour les auditoires.
5. **Responsabilités du SDK** - Chaque SDK développe des aides à l'exportation, des identifiants de voie et des codificateurs de charges utiles Norito pour gérer la parité avec le routeur.## Flux d'exemple

| Type de voie | Preuve à capturer | O que acheter |
|-----------|----------|----------------|
| Privée `xor_hosted_custody` | Journal du routeur + `nexus_settlement_latency_seconds{lane}` + `settlement_router_haircut_total{lane}` | Les tampons CBDC débitent XOR déterministe et les coupes de cheveux sont établies à l'intérieur de la politique. |
| Publique `xor_global` | Journal du routeur + référence DEX/TWAP + mesures de latence/conversation | Le moyen de liquider le partage a fixé le délai de transfert dans TWAP publié avec zéro coupe de cheveux. |
| Hibrida `xor_dual_fund` | Connectez-vous au routeur affiché à la division publique vs blindé + contadores de télémétrie | Une loi blindée/publique respectant les ratios de gouvernance et enregistrant la coupe de cheveux appliquée à chaque personne. |

## Plus de détails précis ?

- FAQ complète : `docs/source/nexus_settlement_faq.md`
- Spécifications du routeur de règlement : `docs/source/settlement_router.md`
- Playbook de politique CBDC : `docs/source/cbdc_lane_playbook.md`
- Runbook des opéras : [Opéras do Nexus](./nexus-operations)